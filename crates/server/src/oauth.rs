//! OAuth 2.1 authorization server for MCP clients (Claude, ChatGPT, ...).
//!
//! The flow a client follows:
//! 1. It calls `/mcp` without a token and gets a 401 pointing at the
//!    protected-resource metadata, which names this server as the issuer.
//! 2. It registers itself (dynamic client registration) and sends the user
//!    to `/oauth/authorize` with a PKCE challenge.
//! 3. The user signs in with their zman account and is sent back with a
//!    single-use code, which the client trades at `/oauth/token` for an
//!    access token (1 hour) and a refresh token (30 days, rotated).
//!
//! Clients are public: there are no client secrets, PKCE (S256) is required.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Form, Json};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::check_credentials;
use crate::db::{OauthClient, OauthCode, Token, TokenKind};
use crate::{ApiError, AppState, auth, now_ms};

const SCOPE: &str = "zman.read";
const CODE_TTL_MS: i64 = 5 * 60_000;
const ACCESS_TTL_MS: i64 = 60 * 60_000;
const REFRESH_TTL_MS: i64 = 30 * 24 * 60 * 60_000;

// ---------- Discovery ----------

pub async fn resource_metadata(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "resource": format!("{}/mcp", state.public_url),
        "authorization_servers": [state.public_url],
        "scopes_supported": [SCOPE],
        "bearer_methods_supported": ["header"],
    }))
}

pub async fn server_metadata(State(state): State<AppState>) -> Json<Value> {
    let url = &state.public_url;
    Json(json!({
        "issuer": url,
        "authorization_endpoint": format!("{url}/oauth/authorize"),
        "token_endpoint": format!("{url}/oauth/token"),
        "registration_endpoint": format!("{url}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": [SCOPE],
    }))
}

// ---------- Dynamic client registration ----------

#[derive(Deserialize)]
pub struct Registration {
    #[serde(default)]
    client_name: String,
    redirect_uris: Vec<String>,
}

/// Where we are willing to send a user back to: https anywhere, plain http
/// only on the local machine, or an app's own scheme. Never a scheme that a
/// browser would execute or read from disk.
fn valid_redirect_uri(uri: &str) -> bool {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    if rest.is_empty() || uri.len() > 2000 || uri.contains(['#', ' ', '\n', '\r', '"', '<', '>']) {
        return false;
    }
    match scheme.as_str() {
        "https" => true,
        "http" => {
            let host = rest.split(['/', '?']).next().unwrap_or("");
            let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
            matches!(host, "localhost" | "127.0.0.1" | "[::1]")
        }
        "javascript" | "data" | "file" | "vbscript" | "blob" | "about" => false,
        _ => scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')),
    }
}

pub async fn register_client(State(state): State<AppState>, Json(body): Json<Registration>) -> Result<Response, ApiError> {
    let invalid = |description: &str| {
        let body = json!({ "error": "invalid_client_metadata", "error_description": description });
        (StatusCode::BAD_REQUEST, Json(body)).into_response()
    };
    if body.redirect_uris.is_empty() || body.redirect_uris.len() > 10 {
        return Ok(invalid("between 1 and 10 redirect_uris are required"));
    }
    if !body.redirect_uris.iter().all(|uri| valid_redirect_uri(uri)) {
        return Ok(invalid("redirect_uris must be https, http on localhost, or an app scheme"));
    }
    let name: String = body.client_name.trim().chars().take(100).collect();
    let client = OauthClient {
        id: auth::random_token()[..32].to_string(),
        name: if name.is_empty() { "MCP client".to_string() } else { name },
        redirect_uris: body.redirect_uris,
    };
    state.db.insert_client(&client, now_ms()).await?;
    let reply = json!({
        "client_id": client.id,
        "client_name": client.name,
        "redirect_uris": client.redirect_uris,
        "token_endpoint_auth_method": "none",
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
    });
    Ok((StatusCode::CREATED, Json(reply)).into_response())
}

// ---------- Authorization (the page the user sees) ----------

#[derive(Deserialize, Clone, Default)]
pub struct AuthorizeParams {
    #[serde(default)]
    response_type: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    code_challenge: String,
    #[serde(default)]
    code_challenge_method: String,
    #[serde(default)]
    state: String,
    // Present only when the sign-in form is submitted.
    #[serde(default)]
    email: String,
    #[serde(default)]
    password: String,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn spanish(headers: &HeaderMap) -> bool {
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim_start().to_lowercase().starts_with("es"))
}

fn page(title: &str, body: &str) -> Html<String> {
    Html(format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · zman</title>
<style>
:root {{ color-scheme: light dark; --page:#f6f6f3; --surface:#fcfcfb; --ink:#0b0b0b; --muted:#6b6a65; --line:rgba(11,11,11,.14); --accent:#2a78d6; --bad:#b92f2f; }}
@media (prefers-color-scheme: dark) {{ :root {{ --page:#0d0d0d; --surface:#1a1a19; --ink:#f0efec; --muted:#a3a29b; --line:rgba(255,255,255,.16); --accent:#3987e5; --bad:#e66767; }} }}
* {{ box-sizing: border-box; }}
body {{ margin:0; min-height:100vh; display:grid; place-items:center; padding:24px; background:var(--page); color:var(--ink); font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif; }}
main {{ width:100%; max-width:380px; background:var(--surface); border:1px solid var(--line); border-radius:16px; padding:28px; }}
.brand {{ font-weight:650; font-size:17px; margin-bottom:18px; }}
h1 {{ font-size:20px; margin:0 0 6px; line-height:1.3; }}
p {{ margin:0 0 18px; color:var(--muted); }}
b {{ color:var(--ink); font-weight:600; }}
label {{ display:block; font-size:13px; color:var(--muted); margin:12px 0 4px; }}
input {{ width:100%; padding:10px 12px; font:inherit; color:var(--ink); background:transparent; border:1px solid var(--line); border-radius:9px; }}
input:focus {{ outline:2px solid var(--accent); outline-offset:1px; }}
button {{ width:100%; margin-top:20px; padding:11px; font:inherit; font-weight:600; color:#fff; background:var(--accent); border:0; border-radius:9px; cursor:pointer; }}
.error {{ color:var(--bad); margin:0 0 4px; }}
small {{ display:block; margin-top:16px; color:var(--muted); font-size:12.5px; }}
</style></head><body><main><div class="brand">zman</div>{body}</main></body></html>"#
    ))
}

/// A problem we cannot report by redirecting, because the client or its
/// redirect URI is the thing we do not trust.
fn error_page(es: bool, message: &str) -> Response {
    let title = if es { "No se pudo continuar" } else { "Could not continue" };
    (StatusCode::BAD_REQUEST, page(title, &format!("<h1>{title}</h1><p>{}</p>", escape(message)))).into_response()
}

fn login_form(es: bool, params: &AuthorizeParams, client: &OauthClient, error: Option<&str>) -> Html<String> {
    let host = params.redirect_uri.split("://").nth(1).unwrap_or("").split(['/', '?']).next().unwrap_or("");
    let (title, lead, email, password, button, note) = if es {
        (
            "Conectar con zman",
            format!("<b>{}</b> quiere leer tus resúmenes de tiempo (totales por categoría, app y sitio). No puede ver títulos de ventanas ni modificar nada.", escape(&client.name)),
            "Email",
            "Contraseña",
            "Iniciar sesión y permitir",
            format!("Vas a volver a {}.", escape(host)),
        )
    } else {
        (
            "Connect to zman",
            format!("<b>{}</b> wants to read your time summaries (totals per category, app and site). It cannot see window titles or change anything.", escape(&client.name)),
            "Email",
            "Password",
            "Sign in and allow",
            format!("You will be sent back to {}.", escape(host)),
        )
    };
    let hidden: String = [
        ("response_type", &params.response_type),
        ("client_id", &params.client_id),
        ("redirect_uri", &params.redirect_uri),
        ("code_challenge", &params.code_challenge),
        ("code_challenge_method", &params.code_challenge_method),
        ("state", &params.state),
    ]
    .iter()
    .map(|(name, value)| format!(r#"<input type="hidden" name="{name}" value="{}">"#, escape(value)))
    .collect();
    let error = error.map_or(String::new(), |e| format!(r#"<p class="error">{}</p>"#, escape(e)));
    page(
        title,
        &format!(
            r#"<h1>{title}</h1><p>{lead}</p>{error}
<form method="post" action="/oauth/authorize">{hidden}
<label for="email">{email}</label><input id="email" name="email" type="email" autocomplete="username" value="{}" required autofocus>
<label for="password">{password}</label><input id="password" name="password" type="password" autocomplete="current-password" required>
<button type="submit">{button}</button></form><small>{note}</small>"#,
            escape(&params.email)
        ),
    )
}

/// Checks the parts of an authorization request that must be right before
/// we show anything or redirect anywhere.
async fn validate_request(state: &AppState, params: &AuthorizeParams, es: bool) -> Result<OauthClient, Response> {
    let fail = |en: &str, es_text: &str| Err(error_page(es, if es { es_text } else { en }));
    let client = match state.db.client(&params.client_id).await {
        Ok(Some(client)) => client,
        Ok(None) => return fail("Unknown application. Try connecting again from your assistant.", "Aplicación desconocida. Probá conectar de nuevo desde tu asistente."),
        Err(e) => return Err(ApiError::from(e).into_response()),
    };
    if !client.redirect_uris.contains(&params.redirect_uri) {
        return fail("The return address does not match the one this application registered.", "La dirección de retorno no coincide con la que registró esta aplicación.");
    }
    if params.response_type != "code" || params.code_challenge_method != "S256" || params.code_challenge.len() < 43 || params.code_challenge.len() > 128 {
        return fail("This application sent an unsupported request (PKCE with S256 is required).", "Esta aplicación envió un pedido no soportado (se requiere PKCE con S256).");
    }
    Ok(client)
}

pub async fn authorize_page(State(state): State<AppState>, headers: HeaderMap, Query(params): Query<AuthorizeParams>) -> Response {
    let es = spanish(&headers);
    match validate_request(&state, &params, es).await {
        Ok(client) => no_store(login_form(es, &params, &client, None).into_response()),
        Err(response) => response,
    }
}

pub async fn authorize_submit(State(state): State<AppState>, headers: HeaderMap, Form(params): Form<AuthorizeParams>) -> Response {
    let es = spanish(&headers);
    let client = match validate_request(&state, &params, es).await {
        Ok(client) => client,
        Err(response) => return response,
    };
    let user = match check_credentials(&state, &params.email, &params.password).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            let message = if es { "Email o contraseña incorrectos." } else { "Wrong email or password." };
            let form = login_form(es, &params, &client, Some(message));
            return no_store((StatusCode::UNAUTHORIZED, form).into_response());
        }
        Err(e) => return e.into_response(),
    };

    let code = auth::random_token();
    let stored = OauthCode {
        client_id: client.id,
        user_id: user.id,
        redirect_uri: params.redirect_uri.clone(),
        code_challenge: params.code_challenge.clone(),
        expires_at_ms: now_ms() + CODE_TTL_MS,
    };
    if let Err(e) = state.db.insert_code(&auth::token_hash(&code), &stored).await {
        return ApiError::from(e).into_response();
    }
    let separator = if params.redirect_uri.contains('?') { '&' } else { '?' };
    let mut target = format!("{}{separator}code={code}", params.redirect_uri);
    if !params.state.is_empty() {
        target.push_str(&format!("&state={}", percent_encode(&params.state)));
    }
    no_store(Redirect::to(&target).into_response())
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

// ---------- Token endpoint ----------

#[derive(Deserialize)]
pub struct TokenRequest {
    #[serde(default)]
    grant_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    code_verifier: String,
    #[serde(default)]
    refresh_token: String,
}

fn token_error(error: &str, description: &str) -> Response {
    let body = json!({ "error": error, "error_description": description });
    no_store((StatusCode::BAD_REQUEST, Json(body)).into_response())
}

async fn issue_tokens(state: &AppState, user_id: &str, client_id: &str) -> Result<Response, ApiError> {
    let now = now_ms();
    let (access, refresh) = (auth::random_token(), auth::random_token());
    for (secret, kind, ttl) in [(&access, TokenKind::Access, ACCESS_TTL_MS), (&refresh, TokenKind::Refresh, REFRESH_TTL_MS)] {
        let token = Token {
            user_id: user_id.to_string(),
            kind,
            device_id: None,
            client_id: Some(client_id.to_string()),
            expires_at_ms: Some(now + ttl),
        };
        state.db.insert_token(&auth::token_hash(secret), &token).await?;
    }
    let body = json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": ACCESS_TTL_MS / 1000,
        "refresh_token": refresh,
        "scope": SCOPE,
    });
    Ok(no_store(Json(body).into_response()))
}

pub async fn token(State(state): State<AppState>, Form(body): Form<TokenRequest>) -> Result<Response, ApiError> {
    match body.grant_type.as_str() {
        "authorization_code" => {
            let Some(code) = state.db.take_code(&auth::token_hash(&body.code)).await? else {
                return Ok(token_error("invalid_grant", "unknown or already used code"));
            };
            let verifier_ok = (43..=128).contains(&body.code_verifier.len())
                && auth::constant_time_eq(&auth::pkce_challenge(&body.code_verifier), &code.code_challenge);
            if code.expires_at_ms <= now_ms()
                || code.client_id != body.client_id
                || code.redirect_uri != body.redirect_uri
                || !verifier_ok
            {
                return Ok(token_error("invalid_grant", "the code does not match this request"));
            }
            issue_tokens(&state, &code.user_id, &code.client_id).await
        }
        "refresh_token" => {
            let hash = auth::token_hash(&body.refresh_token);
            let Some(stored) = state.db.token(&hash).await? else {
                return Ok(token_error("invalid_grant", "unknown refresh token"));
            };
            let client_id = stored.client_id.clone().unwrap_or_default();
            let expired = stored.expires_at_ms.is_some_and(|at| at <= now_ms());
            if stored.kind != TokenKind::Refresh || expired || client_id != body.client_id {
                return Ok(token_error("invalid_grant", "the refresh token is not valid for this client"));
            }
            // Rotation: each refresh token works once.
            state.db.delete_token(&hash).await?;
            issue_tokens(&state, &stored.user_id, &client_id).await
        }
        _ => Ok(token_error("unsupported_grant_type", "use authorization_code or refresh_token")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri_rules() {
        for ok in [
            "https://claude.ai/api/mcp/auth_callback",
            "https://chatgpt.com/connector_platform_oauth_redirect",
            "http://localhost:6274/oauth/callback",
            "http://127.0.0.1/cb",
            "cursor://anysphere.cursor-mcp/oauth/callback",
        ] {
            assert!(valid_redirect_uri(ok), "{ok}");
        }
        for bad in [
            "http://example.com/cb",
            "http://localhost.evil.com/cb",
            "javascript://alert(1)",
            "data://text/html,x",
            "file:///etc/passwd",
            "https://",
            "not a uri",
            "https://a.com/cb#fragment",
            "https://a.com/\"><script>",
        ] {
            assert!(!valid_redirect_uri(bad), "{bad}");
        }
    }

    #[test]
    fn escaping_and_encoding() {
        assert_eq!(escape(r#"<a href="x">&'"#), "&lt;a href=&quot;x&quot;&gt;&amp;&#39;");
        assert_eq!(percent_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    }
}
