//! Remote MCP endpoint (Streamable HTTP, stateless): one JSON-RPC message
//! per POST, answered with plain JSON. The tools are the same ones the
//! local `zman-mcp` serves, run over the signed-in user's synced events.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use jiff::tz::TimeZone;
use serde_json::Value;
use tokio::runtime::Handle;
use zman_core::{Categories, CategoryRule, Event, Lang, Range};
use zman_mcp::{EventSource, Server};

use crate::db::{Db, TokenKind, User};
use crate::{AppState, authenticate};

/// Reads one user's events for the (synchronous) MCP tools. Only ever used
/// from a blocking thread, where waiting on the async database is fine.
struct UserEvents {
    db: Db,
    user_id: String,
    runtime: Handle,
}

impl EventSource for UserEvents {
    fn events_between(&self, range: Range) -> Result<Vec<Event>, String> {
        self.runtime.block_on(self.db.events_between(&self.user_id, range))
    }
}

fn categories_of(user: &User) -> Categories {
    user.categories
        .as_deref()
        .and_then(|json| serde_json::from_str::<Vec<CategoryRule>>(json).ok())
        .filter(|rules| !rules.is_empty())
        .map_or_else(|| Categories::defaults(Lang::En), |rules| Categories::new(rules, Lang::En))
}

fn time_zone_of(user: &User) -> TimeZone {
    user.timezone.as_deref().and_then(|name| TimeZone::get(name).ok()).unwrap_or(TimeZone::UTC)
}

pub async fn handle(State(state): State<AppState>, headers: HeaderMap, Json(message): Json<Value>) -> Response {
    let user = match authenticate(&state, &headers, TokenKind::Access).await {
        Ok((_, token)) => match state.db.user_by_id(&token.user_id).await {
            Ok(Some(user)) => user,
            _ => return unauthorized(&state),
        },
        Err(_) => return unauthorized(&state),
    };

    let source = UserEvents { db: state.db.clone(), user_id: user.id.clone(), runtime: Handle::current() };
    let reply = tokio::task::spawn_blocking(move || {
        let server = Server { source: &source, categories: categories_of(&user), default_tz: time_zone_of(&user), focus: None, clients: None };
        server.handle(&message)
    })
    .await;

    match reply {
        Ok(Some(reply)) => Json(reply).into_response(),
        // Notifications have no reply.
        Ok(None) => StatusCode::ACCEPTED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// This server never opens a stream of its own, so GET is not offered.
pub async fn not_allowed() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")]).into_response()
}

/// A 401 that tells the client where to learn how to sign in.
fn unauthorized(state: &AppState) -> Response {
    let challenge = format!(
        "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource\"",
        state.public_url
    );
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response.headers_mut().insert(header::WWW_AUTHENTICATE, value);
    }
    response
}
