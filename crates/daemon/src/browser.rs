//! Receives the active tab from the zman browser extension.
//!
//! The extension POSTs `{ "url": ..., "title": ... }` to a port on
//! 127.0.0.1 every time the active tab changes. The tracker then attaches
//! the URL to the event when the focused window shows that same tab.
//!
//! The HTTP handling is deliberately tiny: one short request per
//! connection, no keep-alive, nothing but the two routes below.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use zman_core::WindowInfo;
use zman_core::event::clean_url;

const MAX_REQUEST_BYTES: usize = 16 * 1024;

/// The tab the browser last reported as active.
#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub url: String,
    pub title: String,
}

/// Shared between the listener thread and the tracking loop.
#[derive(Debug, Clone, Default)]
pub struct ActiveTab(Arc<Mutex<Option<Tab>>>);

impl ActiveTab {
    fn set(&self, tab: Option<Tab>) {
        *self.0.lock().unwrap() = tab;
    }

    /// Fills in `window.url` if the window is showing the reported tab.
    ///
    /// Browsers title their window after the page ("Page title - Google
    /// Chrome"), so the URL is only trusted while the window title still
    /// starts with the tab's title. A stale report can never label another
    /// window.
    pub fn annotate(&self, window: &mut WindowInfo) {
        let guard = self.0.lock().unwrap();
        if let Some(tab) = guard.as_ref() {
            if !tab.title.is_empty() && window.title.starts_with(&tab.title) {
                window.url = Some(tab.url.clone());
            }
        }
    }
}

/// Starts listening on `127.0.0.1:port` in a background thread.
pub fn listen(port: u16, full_urls: bool) -> std::io::Result<ActiveTab> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    let active = ActiveTab::default();
    let shared = active.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = serve(stream, &shared, full_urls);
        }
    });
    Ok(active)
}

fn serve(mut stream: TcpStream, active: &ActiveTab, full_urls: bool) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    // Read until the headers and the declared body are complete.
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..n]);
        if request.len() > MAX_REQUEST_BYTES || is_complete(&request) {
            break;
        }
    }
    let (status, body) = respond(&request, active, full_urls);
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn split_request(request: &[u8]) -> Option<(&str, &[u8])> {
    let end = request.windows(4).position(|w| w == b"\r\n\r\n")?;
    Some((std::str::from_utf8(&request[..end]).ok()?, &request[end + 4..]))
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then_some(value.trim())
    })
}

fn is_complete(request: &[u8]) -> bool {
    let Some((head, body)) = split_request(request) else {
        return false;
    };
    let length = header(head, "content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    body.len() >= length
}

/// Decides the reply to a raw HTTP request: `(status line, JSON body)`.
fn respond(request: &[u8], active: &ActiveTab, full_urls: bool) -> (&'static str, String) {
    let Some((head, body)) = split_request(request) else {
        return ("400 Bad Request", "{}".into());
    };
    let mut parts = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));

    match (method, path) {
        // Lets the extension's popup show whether zman is running.
        ("GET", "/status") => ("200 OK", format!(r#"{{"app":"zman","version":"{}"}}"#, env!("CARGO_PKG_VERSION"))),
        ("POST", "/tab") => {
            // Only browser extensions may report tabs. A web page cannot
            // forge this header, so sites cannot inject fake activity.
            let origin = header(head, "origin").unwrap_or("");
            if !origin.starts_with("chrome-extension://") && !origin.starts_with("moz-extension://") {
                return ("403 Forbidden", "{}".into());
            }
            let Ok(message) = serde_json::from_slice::<Value>(body) else {
                return ("400 Bad Request", "{}".into());
            };
            let text = |key: &str| message.get(key).and_then(Value::as_str).unwrap_or("").to_string();
            // No URL (or one we do not store) means "no trackable tab".
            let tab = clean_url(&text("url"), full_urls).map(|url| Tab { url, title: text("title") });
            active.set(tab);
            ("200 OK", "{}".into())
        }
        _ => ("404 Not Found", "{}".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn post(origin: &str, body: &str) -> Vec<u8> {
        format!(
            "POST /tab HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn window(title: &str) -> WindowInfo {
        WindowInfo { app: "chrome.exe".into(), title: title.into(), url: None }
    }

    const EXT: &str = "chrome-extension://abcdefghijklmnop";

    #[test]
    fn extension_reports_a_tab_and_matching_window_gets_the_url() {
        let active = ActiveTab::default();
        let body = r#"{"url":"https://github.com/MalkSof/zman?tab=readme","title":"MalkSof/zman"}"#;
        assert_eq!(respond(&post(EXT, body), &active, true).0, "200 OK");

        let mut w = window("MalkSof/zman - Google Chrome");
        active.annotate(&mut w);
        assert_eq!(w.url.as_deref(), Some("https://github.com/MalkSof/zman"));

        // A different window (or a tab the extension has not reported yet).
        let mut other = window("Inbox - Gmail - Google Chrome");
        active.annotate(&mut other);
        assert_eq!(other.url, None);
    }

    #[test]
    fn full_urls_off_keeps_only_the_site() {
        let active = ActiveTab::default();
        respond(&post(EXT, r#"{"url":"https://github.com/a/b","title":"Repo"}"#), &active, false);
        let mut w = window("Repo - Google Chrome");
        active.annotate(&mut w);
        assert_eq!(w.url.as_deref(), Some("https://github.com/"));
    }

    #[test]
    fn web_pages_cannot_report_tabs() {
        let active = ActiveTab::default();
        let body = r#"{"url":"https://evil.example/","title":"x"}"#;
        assert_eq!(respond(&post("https://evil.example", body), &active, true).0, "403 Forbidden");
        assert_eq!(respond(&post("", body), &active, true).0, "403 Forbidden");
        let mut w = window("x");
        active.annotate(&mut w);
        assert_eq!(w.url, None);
    }

    #[test]
    fn non_web_or_missing_urls_clear_the_tab() {
        let active = ActiveTab::default();
        respond(&post(EXT, r#"{"url":"https://a.com/","title":"A"}"#), &active, true);
        respond(&post(EXT, r#"{"url":"chrome://settings","title":"Settings"}"#), &active, true);
        let mut w = window("A - Google Chrome");
        active.annotate(&mut w);
        assert_eq!(w.url, None);

        respond(&post(EXT, r#"{"url":"https://a.com/","title":""}"#), &active, true);
        let mut untitled = window("anything");
        active.annotate(&mut untitled);
        assert_eq!(untitled.url, None, "an empty tab title must not match every window");
    }

    #[test]
    fn other_routes_and_garbage() {
        let active = ActiveTab::default();
        assert_eq!(respond(b"GET /status HTTP/1.1\r\nHost: x\r\n\r\n", &active, true).0, "200 OK");
        assert_eq!(respond(b"GET / HTTP/1.1\r\n\r\n", &active, true).0, "404 Not Found");
        assert_eq!(respond(b"\xff\xfe garbage", &active, true).0, "400 Bad Request");
        assert_eq!(respond(&post(EXT, "{not json"), &active, true).0, "400 Bad Request");
    }

    #[test]
    fn completeness_waits_for_the_body() {
        let full = post(EXT, r#"{"url":"https://a.com/"}"#);
        assert!(is_complete(&full));
        assert!(!is_complete(&full[..full.len() - 3]));
        assert!(!is_complete(b"POST /tab HTTP/1.1\r\nHost: x\r\n"));
    }
}
