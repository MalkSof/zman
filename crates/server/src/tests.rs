//! End-to-end tests of the HTTP surface, on in-memory storage.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::db::Db;
use crate::{AppState, app, auth};

const BASE: &str = "https://zman.test";
/// 2026-11-20T00:00:00Z.
const DAY: i64 = 1_795_132_800_000;
const MIN: i64 = 60_000;
const CALLBACK: &str = "https://assistant.test/callback";

fn server(allow_registration: bool) -> Router {
    app(AppState { db: Db::memory(), public_url: BASE.into(), allow_registration })
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    text: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|_| panic!("not JSON: {}", self.text))
    }
}

async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = to_bytes(body, 1 << 20).await.unwrap();
    Reply { status: parts.status, headers: parts.headers, text: String::from_utf8_lossy(&bytes).into_owned() }
}

async fn json_request(app: &Router, method: &str, path: &str, token: Option<&str>, body: Value) -> Reply {
    let mut request = Request::builder().method(method).uri(path).header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    send(app, request.body(Body::from(body.to_string())).unwrap()).await
}

async fn form(app: &Router, path: &str, fields: &[(&str, &str)]) -> Reply {
    let encode = |text: &str| -> String {
        text.bytes()
            .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
            .collect()
    };
    let body: Vec<String> = fields.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect();
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.join("&")))
        .unwrap();
    send(app, request).await
}

async fn get(app: &Router, path: &str) -> Reply {
    send(app, Request::builder().uri(path).body(Body::empty()).unwrap()).await
}

/// Registers a user and signs a device in; returns the device token.
async fn device(app: &Router, email: &str, device_id: &str) -> String {
    let credentials = json!({ "email": email, "password": "correct horse" });
    let created = json_request(app, "POST", "/api/register", None, credentials).await;
    assert!(created.status == StatusCode::CREATED || created.status == StatusCode::CONFLICT);
    let login = json!({ "email": email, "password": "correct horse", "device_id": device_id, "device_name": "Laptop" });
    let reply = json_request(app, "POST", "/api/login", None, login).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    reply.json()["token"].as_str().unwrap().to_string()
}

fn event(id: &str, device_id: &str, app_name: &str, start_min: i64, minutes: i64, updated: i64) -> Value {
    json!({
        "id": id, "device_id": device_id, "start_ms": DAY + start_min * MIN, "duration_ms": minutes * MIN,
        "app": app_name, "title": "secret window title", "data": {}, "updated_at_ms": updated,
    })
}

/// Runs the whole OAuth flow as an MCP client would; returns the token reply.
async fn oauth_tokens(app: &Router, email: &str) -> Value {
    let client = json_request(app, "POST", "/oauth/register", None, json!({ "client_name": "Assistant", "redirect_uris": [CALLBACK] })).await;
    assert_eq!(client.status, StatusCode::CREATED);
    let client_id = client.json()["client_id"].as_str().unwrap().to_string();

    let verifier = "a-code-verifier-that-is-at-least-43-characters-long";
    let challenge = auth::pkce_challenge(verifier);
    let fields = [
        ("response_type", "code"), ("client_id", client_id.as_str()), ("redirect_uri", CALLBACK),
        ("code_challenge", challenge.as_str()), ("code_challenge_method", "S256"), ("state", "xyz 1"),
        ("email", email), ("password", "correct horse"),
    ];
    let redirect = form(app, "/oauth/authorize", &fields).await;
    assert_eq!(redirect.status, StatusCode::SEE_OTHER, "{}", redirect.text);
    let location = redirect.headers[header::LOCATION].to_str().unwrap().to_string();
    assert!(location.starts_with(&format!("{CALLBACK}?code=")), "{location}");
    assert!(location.ends_with("&state=xyz%201"), "{location}");
    let code = location.split("code=").nth(1).unwrap().split('&').next().unwrap().to_string();

    let exchange = [
        ("grant_type", "authorization_code"), ("code", code.as_str()), ("redirect_uri", CALLBACK),
        ("client_id", client_id.as_str()), ("code_verifier", verifier),
    ];
    let tokens = form(app, "/oauth/token", &exchange).await;
    assert_eq!(tokens.status, StatusCode::OK, "{}", tokens.text);
    assert_eq!(tokens.headers[header::CACHE_CONTROL], "no-store");

    // The code is single-use.
    assert_eq!(form(app, "/oauth/token", &exchange).await.status, StatusCode::BAD_REQUEST);

    let mut reply = tokens.json();
    reply["client_id"] = json!(client_id);
    reply
}

async fn mcp_call(app: &Router, token: &str, tool: &str, args: Value) -> Value {
    let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } });
    let reply = json_request(app, "POST", "/mcp", Some(token), request).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let result = reply.json()["result"].clone();
    assert_eq!(result["isError"], false, "{result}");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn users_are_listed_newest_first() {
    let db = crate::db::Db::memory();
    for email in ["a@example.com", "b@example.com", "c@example.com"] {
        db.create_user(email, "hash", 1).await.unwrap().unwrap();
        // Ids only sort by time across milliseconds.
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    let (users, total) = db.recent_users(2).await.unwrap();
    assert_eq!(total, 3);
    assert_eq!(users.iter().map(|u| u.email.as_str()).collect::<Vec<_>>(), ["c@example.com", "b@example.com"]);
}

#[tokio::test]
async fn accounts_and_login() {
    let app = server(true);
    let credentials = json!({ "email": " Ana@Example.com ", "password": "correct horse" });
    assert_eq!(json_request(&app, "POST", "/api/register", None, credentials.clone()).await.status, StatusCode::CREATED);
    assert_eq!(json_request(&app, "POST", "/api/register", None, credentials).await.status, StatusCode::CONFLICT);

    for bad in [json!({ "email": "nope", "password": "correct horse" }), json!({ "email": "a@b.co", "password": "short" })] {
        assert_eq!(json_request(&app, "POST", "/api/register", None, bad).await.status, StatusCode::BAD_REQUEST);
    }

    let login = |password: &str, email: &str| json!({ "email": email, "password": password, "device_id": "DEV1", "device_name": "Laptop" });
    assert_eq!(json_request(&app, "POST", "/api/login", None, login("wrong", "ana@example.com")).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(json_request(&app, "POST", "/api/login", None, login("correct horse", "nobody@example.com")).await.status, StatusCode::UNAUTHORIZED);

    let ok = json_request(&app, "POST", "/api/login", None, login("correct horse", "ana@example.com")).await;
    assert_eq!(ok.status, StatusCode::OK);
    let token = ok.json()["token"].as_str().unwrap().to_string();

    let me = json_request(&app, "GET", "/api/me", Some(&token), json!({})).await.json();
    assert_eq!(me["email"], "ana@example.com");
    assert_eq!(me["devices"][0]["name"], "Laptop");

    assert_eq!(json_request(&app, "POST", "/api/logout", Some(&token), json!({})).await.status, StatusCode::NO_CONTENT);
    assert_eq!(json_request(&app, "GET", "/api/me", Some(&token), json!({})).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn registration_can_be_closed() {
    let app = server(false);
    let reply = json_request(&app, "POST", "/api/register", None, json!({ "email": "a@b.co", "password": "correct horse" })).await;
    assert_eq!(reply.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn upload_is_idempotent_and_keeps_the_newest_version() {
    let app = server(true);
    let token = device(&app, "ana@example.com", "DEV1").await;

    let upload = |events: Value| json_request(&app, "POST", "/api/events", Some(&token), json!({ "events": events }));
    assert_eq!(upload(json!([event("E1", "DEV1", "code.exe", 540, 10, 100)])).await.status, StatusCode::OK);
    // The same event grows; then a stale copy arrives late and is ignored.
    assert_eq!(upload(json!([event("E1", "DEV1", "code.exe", 540, 45, 200)])).await.status, StatusCode::OK);
    assert_eq!(upload(json!([event("E1", "DEV1", "code.exe", 540, 10, 100)])).await.status, StatusCode::OK);
    // Both versions in one batch: the newest wins regardless of order.
    let both = json!([event("E2", "DEV1", "code.exe", 600, 30, 300), event("E2", "DEV1", "code.exe", 600, 5, 250)]);
    assert_eq!(upload(both).await.status, StatusCode::OK);

    let access = oauth_tokens(&app, "ana@example.com").await["access_token"].as_str().unwrap().to_string();
    let summary = mcp_call(&app, &access, "daily_summary", json!({ "date": "2026-11-20" })).await;
    assert_eq!(summary["tracked_minutes"], 75);
}

#[tokio::test]
async fn uploads_are_validated() {
    let app = server(true);
    let token = device(&app, "ana@example.com", "DEV1").await;
    let upload = |token: Option<&'static str>, events: Value| {
        let app = app.clone();
        async move { json_request(&app, "POST", "/api/events", token, json!({ "events": events })).await.status }
    };
    let token: &'static str = Box::leak(token.into_boxed_str());

    assert_eq!(upload(None, json!([])).await, StatusCode::UNAUTHORIZED);
    assert_eq!(upload(Some("not-a-token"), json!([])).await, StatusCode::UNAUTHORIZED);
    // Events claiming to come from another device.
    assert_eq!(upload(Some(token), json!([event("E1", "OTHER", "x", 0, 1, 1)])).await, StatusCode::BAD_REQUEST);
    // Negative duration, bad id.
    assert_eq!(upload(Some(token), json!([event("E1", "DEV1", "x", 0, -5, 1)])).await, StatusCode::BAD_REQUEST);
    assert_eq!(upload(Some(token), json!([event("bad id!", "DEV1", "x", 0, 1, 1)])).await, StatusCode::BAD_REQUEST);
    // Too many at once.
    let many: Vec<Value> = (0..crate::api::MAX_BATCH + 1).map(|i| event(&format!("E{i}"), "DEV1", "x", 0, 1, 1)).collect();
    assert_eq!(upload(Some(token), json!(many)).await, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn users_only_see_their_own_data() {
    let app = server(true);
    let ana = device(&app, "ana@example.com", "DEV1").await;
    let bob = device(&app, "bob@example.com", "DEV2").await;
    let upload = |token: String, events: Value| json_request(&app, "POST", "/api/events", Some(Box::leak(token.into_boxed_str())), json!({ "events": events }));
    upload(ana, json!([event("E1", "DEV1", "code.exe", 540, 60, 1)])).await;
    // Bob reuses Ana's event id: it must not overwrite hers.
    upload(bob, json!([event("E1", "DEV2", "game.exe", 540, 20, 5)])).await;

    let ana_access = oauth_tokens(&app, "ana@example.com").await["access_token"].as_str().unwrap().to_string();
    let bob_access = oauth_tokens(&app, "bob@example.com").await["access_token"].as_str().unwrap().to_string();
    let ana_day = mcp_call(&app, &ana_access, "daily_summary", json!({ "date": "2026-11-20" })).await;
    let bob_day = mcp_call(&app, &bob_access, "daily_summary", json!({ "date": "2026-11-20" })).await;
    assert_eq!(ana_day["tracked_minutes"], 60);
    assert_eq!(ana_day["apps"][0]["name"], "code");
    assert_eq!(bob_day["tracked_minutes"], 20);
    assert_eq!(bob_day["apps"][0]["name"], "game");
}

#[tokio::test]
async fn several_devices_add_up_and_settings_apply() {
    let app = server(true);
    let laptop = device(&app, "ana@example.com", "DEV1").await;
    let desktop = device(&app, "ana@example.com", "DEV2").await;
    json_request(&app, "POST", "/api/events", Some(&laptop), json!({ "events": [event("E1", "DEV1", "code.exe", 9 * 60, 30, 1)] })).await;
    json_request(&app, "POST", "/api/events", Some(&desktop), json!({ "events": [event("E2", "DEV2", "mytool.exe", 12 * 60, 15, 1)] })).await;

    let settings = json!({
        "timezone": "America/Argentina/Buenos_Aires",
        "categories": [{ "name": "Deep work", "score": 2, "apps": ["mytool.exe"] }],
    });
    assert_eq!(json_request(&app, "PUT", "/api/settings", Some(&laptop), settings).await.status, StatusCode::NO_CONTENT);
    let bad = json!({ "timezone": "Mars/Olympus", "categories": null });
    assert_eq!(json_request(&app, "PUT", "/api/settings", Some(&laptop), bad).await.status, StatusCode::BAD_REQUEST);

    let access = oauth_tokens(&app, "ana@example.com").await["access_token"].as_str().unwrap().to_string();
    let day = mcp_call(&app, &access, "daily_summary", json!({ "date": "2026-11-20" })).await;
    assert_eq!(day["tracked_minutes"], 45);
    // The user's own rule is applied, and hours are in their time zone (UTC-3).
    let deep = day["categories"].as_array().unwrap().iter().find(|c| c["name"] == "Deep work").unwrap();
    assert_eq!(deep["minutes"], 15);
    assert_eq!(day["hourly_minutes"][6], 30);
    assert_eq!(day["hourly_minutes"][9], 15);
    // Titles never travel through MCP.
    assert!(!day.to_string().contains("secret window title"));
}

#[tokio::test]
async fn mcp_requires_an_oauth_access_token() {
    let app = server(true);
    let device_token = device(&app, "ana@example.com", "DEV1").await;
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    let anonymous = json_request(&app, "POST", "/mcp", None, ping.clone()).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let challenge = anonymous.headers[header::WWW_AUTHENTICATE].to_str().unwrap();
    assert_eq!(challenge, format!("Bearer resource_metadata=\"{BASE}/.well-known/oauth-protected-resource\""));

    // A device token is not an MCP credential.
    assert_eq!(json_request(&app, "POST", "/mcp", Some(&device_token), ping.clone()).await.status, StatusCode::UNAUTHORIZED);

    let tokens = oauth_tokens(&app, "ana@example.com").await;
    let access = tokens["access_token"].as_str().unwrap();
    assert_eq!(tokens["token_type"], "Bearer");
    assert_eq!(tokens["expires_in"], 3600);
    assert_eq!(json_request(&app, "POST", "/mcp", Some(access), ping).await.json()["result"], json!({}));

    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {} } });
    assert_eq!(json_request(&app, "POST", "/mcp", Some(access), init).await.json()["result"]["serverInfo"]["name"], "zman");
    let listed = json_request(&app, "POST", "/mcp", Some(access), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    assert_eq!(listed.json()["result"]["tools"].as_array().unwrap().len(), 6);

    let notification = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    assert_eq!(json_request(&app, "POST", "/mcp", Some(access), notification).await.status, StatusCode::ACCEPTED);
    assert_eq!(get(&app, "/mcp").await.status, StatusCode::METHOD_NOT_ALLOWED);

    // An access token cannot be used on the device API either.
    assert_eq!(json_request(&app, "GET", "/api/me", Some(access), json!({})).await.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn refresh_tokens_rotate() {
    let app = server(true);
    device(&app, "ana@example.com", "DEV1").await;
    let tokens = oauth_tokens(&app, "ana@example.com").await;
    let client_id = tokens["client_id"].as_str().unwrap();
    let refresh = tokens["refresh_token"].as_str().unwrap();

    let fields = [("grant_type", "refresh_token"), ("refresh_token", refresh), ("client_id", client_id)];
    let renewed = form(&app, "/oauth/token", &fields).await;
    assert_eq!(renewed.status, StatusCode::OK, "{}", renewed.text);
    let new_access = renewed.json()["access_token"].as_str().unwrap().to_string();
    assert_ne!(new_access, tokens["access_token"]);
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    assert_eq!(json_request(&app, "POST", "/mcp", Some(&new_access), ping).await.status, StatusCode::OK);

    // The old refresh token is spent; an access token is not a refresh token.
    assert_eq!(form(&app, "/oauth/token", &fields).await.status, StatusCode::BAD_REQUEST);
    let wrong_kind = [("grant_type", "refresh_token"), ("refresh_token", new_access.as_str()), ("client_id", client_id)];
    assert_eq!(form(&app, "/oauth/token", &wrong_kind).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(form(&app, "/oauth/token", &[("grant_type", "password")]).await.json()["error"], "unsupported_grant_type");
}

#[tokio::test]
async fn authorization_rejects_bad_requests() {
    let app = server(true);
    device(&app, "ana@example.com", "DEV1").await;
    let client = json_request(&app, "POST", "/oauth/register", None, json!({ "client_name": "<b>Evil</b>", "redirect_uris": [CALLBACK] })).await;
    let client_id = client.json()["client_id"].as_str().unwrap().to_string();
    let verifier = "a-code-verifier-that-is-at-least-43-characters-long";
    let challenge = auth::pkce_challenge(verifier);
    let query = |client: &str, redirect: &str, method: &str| {
        format!("/oauth/authorize?response_type=code&client_id={client}&redirect_uri={redirect}&code_challenge={challenge}&code_challenge_method={method}&state=s")
    };

    // The sign-in page renders, with the client name escaped.
    let page = get(&app, &query(&client_id, CALLBACK, "S256")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.text.contains("&lt;b&gt;Evil&lt;/b&gt;") && !page.text.contains("<b>Evil</b>"));
    assert!(page.text.contains("assistant.test"));

    // Unknown client, unregistered redirect and missing PKCE never redirect.
    for bad in [query("nope", CALLBACK, "S256"), query(&client_id, "https://evil.test/cb", "S256"), query(&client_id, CALLBACK, "plain")] {
        let reply = get(&app, &bad).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert!(!reply.headers.contains_key(header::LOCATION));
    }

    // Wrong password shows the form again instead of issuing a code.
    let fields = [
        ("response_type", "code"), ("client_id", client_id.as_str()), ("redirect_uri", CALLBACK),
        ("code_challenge", challenge.as_str()), ("code_challenge_method", "S256"),
        ("email", "ana@example.com"), ("password", "wrong"),
    ];
    let denied = form(&app, "/oauth/authorize", &fields).await;
    assert_eq!(denied.status, StatusCode::UNAUTHORIZED);
    assert!(!denied.headers.contains_key(header::LOCATION));

    // A code cannot be redeemed with the wrong verifier.
    let mut good = fields;
    good[6] = ("password", "correct horse");
    let location = form(&app, "/oauth/authorize", &good).await.headers[header::LOCATION].to_str().unwrap().to_string();
    let code = location.split("code=").nth(1).unwrap().to_string();
    let exchange = [
        ("grant_type", "authorization_code"), ("code", code.as_str()), ("redirect_uri", CALLBACK),
        ("client_id", client_id.as_str()), ("code_verifier", "another-verifier-that-is-also-long-enough-to-pass"),
    ];
    assert_eq!(form(&app, "/oauth/token", &exchange).await.json()["error"], "invalid_grant");

    // Clients cannot register dangerous redirect URIs.
    let evil = json_request(&app, "POST", "/oauth/register", None, json!({ "redirect_uris": ["http://evil.test/cb"] })).await;
    assert_eq!(evil.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn discovery_documents() {
    let app = server(true);
    let resource = get(&app, "/.well-known/oauth-protected-resource").await.json();
    assert_eq!(resource["resource"], format!("{BASE}/mcp"));
    assert_eq!(resource["authorization_servers"][0], BASE);
    assert_eq!(get(&app, "/.well-known/oauth-protected-resource/mcp").await.json(), resource);

    let meta = get(&app, "/.well-known/oauth-authorization-server").await.json();
    assert_eq!(meta["issuer"], BASE);
    assert_eq!(meta["token_endpoint"], format!("{BASE}/oauth/token"));
    assert_eq!(meta["code_challenge_methods_supported"], json!(["S256"]));
    assert_eq!(get(&app, "/health").await.text, "ok");
}
