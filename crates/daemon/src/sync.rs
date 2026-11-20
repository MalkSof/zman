//! Uploading this device's events to a zman server.
//!
//! The device signs in once and keeps a token. Each sync sends the events
//! created or changed since a cursor, in batches, and moves the cursor only
//! after the server confirms a batch: a failed or interrupted sync simply
//! resumes later. The server ignores anything it already has.

use std::time::Duration;

use jiff::tz::TimeZone;
use serde_json::{Value, json};
use zman_core::{Config, Event, Store, now_ms};

const BATCH: u32 = 500;
const KEY_SERVER: &str = "sync_server";
const KEY_EMAIL: &str = "sync_email";
const KEY_TOKEN: &str = "sync_token";
const KEY_CURSOR_MS: &str = "sync_cursor_ms";
const KEY_CURSOR_ID: &str = "sync_cursor_id";
const KEY_LAST_SYNC: &str = "sync_last_ms";

/// The server this device is signed in to.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub server: String,
    pub email: String,
    token: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub account: Option<Account>,
    /// When the last successful sync finished.
    pub last_sync_ms: Option<i64>,
}

fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        // We read error bodies ourselves to show the server's message.
        .http_status_as_error(false)
        .build()
        .into()
}

/// Cleans up a server address and refuses to send credentials in the clear.
pub fn normalize_server(server: &str) -> Result<String, String> {
    let server = server.trim();
    let (scheme, rest) = server.split_once("://").unwrap_or(("https", server));
    let rest = rest.trim_end_matches('/');
    let server = format!("{scheme}://{rest}");
    let host = rest.split('/').next().unwrap_or("");
    let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
    let local = matches!(host, "localhost" | "127.0.0.1" | "[::1]");
    match scheme {
        _ if host.is_empty() => Err("the server address is not valid".into()),
        "https" => Ok(server),
        "http" if local => Ok(server),
        "http" => Err("the server must use https (plain http is only allowed for localhost)".into()),
        _ => Err("the server address must start with https://".into()),
    }
}

/// Sends a JSON request and returns the JSON reply, turning error statuses
/// into the server's own message.
fn call(method: &str, url: &str, token: Option<&str>, body: &Value) -> Result<Value, String> {
    let agent = agent();
    let authorization = token.map(|t| format!("Bearer {t}"));
    let mut response = match method {
        "PUT" => {
            let mut request = agent.put(url);
            if let Some(value) = &authorization {
                request = request.header("Authorization", value);
            }
            request.send_json(body)
        }
        _ => {
            let mut request = agent.post(url);
            if let Some(value) = &authorization {
                request = request.header("Authorization", value);
            }
            request.send_json(body)
        }
    }
    .map_err(|e| format!("could not reach the server: {e}"))?;

    let status = response.status().as_u16();
    let reply: Value = response.body_mut().read_json().unwrap_or(Value::Null);
    if (200..300).contains(&status) {
        return Ok(reply);
    }
    let message = reply.get("error").and_then(Value::as_str).unwrap_or("unexpected reply");
    Err(format!("{message} (HTTP {status})"))
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "device".to_string())
}

pub fn status(store: &Store) -> Result<Status, String> {
    let get = |key: &str| store.meta_get(key).map_err(text);
    let account = match (get(KEY_SERVER)?, get(KEY_EMAIL)?, get(KEY_TOKEN)?) {
        (Some(server), Some(email), Some(token)) => Some(Account { server, email, token }),
        _ => None,
    };
    let last_sync_ms = get(KEY_LAST_SYNC)?.and_then(|v| v.parse().ok());
    Ok(Status { account, last_sync_ms })
}

/// Signs this device in, creating the account first when `register` is set.
/// The password is only ever sent to the server; it is not stored.
pub fn login(store: &Store, server: &str, email: &str, password: &str, register: bool) -> Result<Account, String> {
    let server = normalize_server(server)?;
    if register {
        let body = json!({ "email": email, "password": password });
        call("POST", &format!("{server}/api/register"), None, &body)?;
    }
    let body = json!({
        "email": email,
        "password": password,
        "device_id": store.device_id().map_err(text)?,
        "device_name": device_name(),
    });
    let reply = call("POST", &format!("{server}/api/login"), None, &body)?;
    let token = reply.get("token").and_then(Value::as_str).ok_or("the server did not return a token")?;
    let email = reply.get("email").and_then(Value::as_str).unwrap_or(email).to_string();

    let previous = store.meta_get(KEY_SERVER).map_err(text)?;
    store.meta_set(KEY_SERVER, &server).map_err(text)?;
    store.meta_set(KEY_EMAIL, &email).map_err(text)?;
    store.meta_set(KEY_TOKEN, token).map_err(text)?;
    // A different server has none of our events yet: start over.
    if previous.as_deref() != Some(server.as_str()) {
        store.meta_delete(KEY_CURSOR_MS).map_err(text)?;
        store.meta_delete(KEY_CURSOR_ID).map_err(text)?;
        store.meta_delete(KEY_LAST_SYNC).map_err(text)?;
    }
    Ok(Account { server, email, token: token.to_string() })
}

/// Forgets the account on this device and tells the server to drop the token.
pub fn logout(store: &Store) -> Result<(), String> {
    if let Some(account) = status(store)?.account {
        // Best effort: being offline must not stop a local sign-out.
        let _ = call("POST", &format!("{}/api/logout", account.server), Some(&account.token), &json!({}));
    }
    for key in [KEY_SERVER, KEY_EMAIL, KEY_TOKEN, KEY_CURSOR_MS, KEY_CURSOR_ID, KEY_LAST_SYNC] {
        store.meta_delete(key).map_err(text)?;
    }
    Ok(())
}

/// What an event looks like on the wire, honoring the privacy settings.
fn outgoing(mut event: Event, config: &Config) -> Event {
    if !config.sync.titles {
        event.title.clear();
    }
    event
}

/// Uploads everything new. Returns how many events were sent, or `None` if
/// this device is not signed in.
pub fn run(store: &Store, config: &Config) -> Result<Option<usize>, String> {
    let Some(account) = status(store)?.account else {
        return Ok(None);
    };
    let token = Some(account.token.as_str());

    // Lets the server answer in the user's time zone with their categories.
    let settings = json!({
        "timezone": TimeZone::system().iana_name(),
        "categories": if config.categories.is_empty() { Value::Null } else { json!(config.categories) },
    });
    call("PUT", &format!("{}/api/settings", account.server), token, &settings)?;

    let mut cursor_ms: i64 = store.meta_get(KEY_CURSOR_MS).map_err(text)?.and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut cursor_id = store.meta_get(KEY_CURSOR_ID).map_err(text)?.unwrap_or_default();
    let mut sent = 0;
    loop {
        let batch = store.events_after(cursor_ms, &cursor_id, BATCH).map_err(text)?;
        let Some(last) = batch.last() else {
            break;
        };
        let (next_ms, next_id) = (last.updated_at_ms, last.id.clone());
        let count = batch.len();
        let events: Vec<Event> = batch.into_iter().map(|e| outgoing(e, config)).collect();
        call("POST", &format!("{}/api/events", account.server), token, &json!({ "events": events }))?;

        store.meta_set(KEY_CURSOR_MS, &next_ms.to_string()).map_err(text)?;
        store.meta_set(KEY_CURSOR_ID, &next_id).map_err(text)?;
        (cursor_ms, cursor_id) = (next_ms, next_id);
        sent += count;
        if count < BATCH as usize {
            break;
        }
    }
    store.meta_set(KEY_LAST_SYNC, &now_ms().to_string()).map_err(text)?;
    Ok(Some(sent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_addresses() {
        assert_eq!(normalize_server("zman.example.com/").unwrap(), "https://zman.example.com");
        assert_eq!(normalize_server(" https://zman.example.com/base/ ").unwrap(), "https://zman.example.com/base");
        assert_eq!(normalize_server("http://localhost:8787").unwrap(), "http://localhost:8787");
        assert_eq!(normalize_server("http://127.0.0.1:8787/").unwrap(), "http://127.0.0.1:8787");
        assert!(normalize_server("http://zman.example.com").is_err());
        assert!(normalize_server("ftp://zman.example.com").is_err());
        assert!(normalize_server("https://").is_err());
        assert!(normalize_server("").is_err());
    }

    #[test]
    fn nothing_happens_when_not_signed_in() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(status(&store).unwrap(), Status { account: None, last_sync_ms: None });
        assert_eq!(run(&store, &Config::default()).unwrap(), None);
        logout(&store).unwrap();
    }

    #[test]
    fn titles_can_be_kept_on_the_device() {
        let event = Event {
            id: "E".into(),
            device_id: "D".into(),
            start_ms: 0,
            duration_ms: 1,
            app: "code.exe".into(),
            title: "secret".into(),
            data: json!({}),
            updated_at_ms: 1,
        };
        assert_eq!(outgoing(event.clone(), &Config::default()).title, "secret");
        let private = Config::parse("[sync]\ntitles = false").unwrap();
        let sent = outgoing(event, &private);
        assert_eq!(sent.title, "");
        assert_eq!(sent.app, "code.exe");
    }
}
