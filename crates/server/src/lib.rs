//! zman sync server: stores each user's events from all their devices and
//! serves them, aggregated, through a remote MCP endpoint behind OAuth.

pub mod api;
pub mod auth;
pub mod db;
pub mod mcp;
pub mod oauth;

use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::json;

use crate::db::{Db, Token, TokenKind};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    /// Public base URL, without a trailing slash (e.g. `https://zman.example.com`).
    /// OAuth clients are told to come back here, so it must be reachable by them.
    pub public_url: String,
    /// Whether anyone can create an account.
    pub allow_registration: bool,
}

/// An error reply: an HTTP status and a message for the client.
#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> ApiError {
        ApiError(StatusCode::BAD_REQUEST, message.into())
    }

    pub fn unauthorized() -> ApiError {
        ApiError(StatusCode::UNAUTHORIZED, "invalid or expired token".into())
    }
}

/// Storage failures are logged and reported without internal detail.
impl From<String> for ApiError {
    fn from(message: String) -> Self {
        eprintln!("zman-server: {message}");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "internal error".into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

pub fn now_ms() -> i64 {
    zman_core::now_ms()
}

/// Resolves the `Authorization: Bearer` header to a live token of `kind`.
pub async fn authenticate(state: &AppState, headers: &HeaderMap, kind: TokenKind) -> Result<(String, Token), ApiError> {
    let value = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    let secret = value
        .split_once(' ')
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, secret)| secret.trim())
        .filter(|secret| !secret.is_empty())
        .ok_or_else(ApiError::unauthorized)?;
    let hash = auth::token_hash(secret);
    let token = state.db.token(&hash).await?.ok_or_else(ApiError::unauthorized)?;
    let expired = token.expires_at_ms.is_some_and(|at| at <= now_ms());
    if token.kind != kind || expired {
        return Err(ApiError::unauthorized());
    }
    Ok((hash, token))
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(|| async { "ok" }))
        // Device API.
        .route("/api/register", post(api::register))
        .route("/api/login", post(api::login))
        .route("/api/logout", post(api::logout))
        .route("/api/me", get(api::me))
        .route("/api/settings", put(api::settings))
        .route("/api/events", post(api::events))
        // OAuth 2.1 for MCP clients.
        .route("/.well-known/oauth-protected-resource", get(oauth::resource_metadata))
        .route("/.well-known/oauth-protected-resource/mcp", get(oauth::resource_metadata))
        .route("/.well-known/oauth-authorization-server", get(oauth::server_metadata))
        .route("/oauth/register", post(oauth::register_client))
        .route("/oauth/authorize", get(oauth::authorize_page).post(oauth::authorize_submit))
        .route("/oauth/token", post(oauth::token))
        // Remote MCP.
        .route("/mcp", post(mcp::handle).get(mcp::not_allowed))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(
        "<!doctype html><meta charset=utf-8><title>zman</title>\
         <body style=\"font:16px system-ui;max-width:560px;margin:15vh auto;padding:0 24px;line-height:1.5\">\
         <h1>zman sync server</h1>\
         <p>This server stores the activity your devices sync and exposes it to AI assistants \
         through MCP at <code>/mcp</code>.</p>\
         <p><a href=\"https://github.com/MalkSof/zman\">github.com/MalkSof/zman</a></p>",
    )
}

#[cfg(test)]
mod tests;
