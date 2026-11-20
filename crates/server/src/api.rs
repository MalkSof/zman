//! The API devices use: accounts, login and event upload.

use std::collections::HashMap;
use std::sync::OnceLock;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use jiff::tz::TimeZone;
use serde::Deserialize;
use serde_json::{Value, json};
use zman_core::{CategoryRule, Event};

use crate::db::{Token, TokenKind};
use crate::{ApiError, AppState, auth, authenticate, now_ms};

/// Most events one upload may carry.
pub const MAX_BATCH: usize = 1000;
const MAX_CATEGORY_RULES: usize = 300;

#[derive(Deserialize)]
pub struct Credentials {
    email: String,
    password: String,
}

fn normalize_email(email: &str) -> Result<String, ApiError> {
    let email = email.trim().to_lowercase();
    let valid = email.len() <= 254
        && email.split_once('@').is_some_and(|(name, domain)| {
            !name.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
        })
        && !email.chars().any(char::is_whitespace);
    if valid { Ok(email) } else { Err(ApiError::bad_request("invalid email address")) }
}

/// Checks an email and password, always spending the time of a password
/// hash so the reply does not reveal whether the account exists.
pub async fn check_credentials(state: &AppState, email: &str, password: &str) -> Result<Option<crate::db::User>, ApiError> {
    // The hash of a random password nobody knows, checked when the account
    // does not exist.
    static DUMMY_HASH: OnceLock<String> = OnceLock::new();
    let Ok(email) = normalize_email(email) else {
        return Ok(None);
    };
    let user = state.db.user_by_email(&email).await?;
    let stored = user.as_ref().map(|u| u.password_hash.clone());
    let password = password.to_string();
    let matches = tokio::task::spawn_blocking(move || {
        let hash = stored.unwrap_or_else(|| {
            DUMMY_HASH.get_or_init(|| auth::hash_password(&auth::random_token()).unwrap_or_default()).clone()
        });
        auth::verify_password(&password, &hash)
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(user.filter(|_| matches))
}

pub async fn register(State(state): State<AppState>, Json(body): Json<Credentials>) -> Result<StatusCode, ApiError> {
    if !state.allow_registration {
        return Err(ApiError(StatusCode::FORBIDDEN, "registration is closed on this server".into()));
    }
    let email = normalize_email(&body.email)?;
    if body.password.chars().count() < 8 || body.password.len() > 256 {
        return Err(ApiError::bad_request("the password must have between 8 and 256 characters"));
    }
    let password = body.password;
    let hash = tokio::task::spawn_blocking(move || auth::hash_password(&password))
        .await
        .map_err(|e| e.to_string())??;
    match state.db.create_user(&email, &hash, now_ms()).await? {
        Some(_) => Ok(StatusCode::CREATED),
        None => Err(ApiError(StatusCode::CONFLICT, "that email is already registered".into())),
    }
}

#[derive(Deserialize)]
pub struct Login {
    email: String,
    password: String,
    device_id: String,
    #[serde(default)]
    device_name: String,
}

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Signs a device in and gives it a token to upload events with.
pub async fn login(State(state): State<AppState>, Json(body): Json<Login>) -> Result<Json<Value>, ApiError> {
    if !valid_id(&body.device_id) {
        return Err(ApiError::bad_request("invalid device_id"));
    }
    let user = check_credentials(&state, &body.email, &body.password)
        .await?
        .ok_or(ApiError(StatusCode::UNAUTHORIZED, "wrong email or password".into()))?;

    let name: String = body.device_name.trim().chars().take(80).collect();
    let name = if name.is_empty() { "device".to_string() } else { name };
    state.db.upsert_device(&user.id, &body.device_id, &name, now_ms()).await?;

    let secret = auth::random_token();
    let token = Token {
        user_id: user.id,
        kind: TokenKind::Device,
        device_id: Some(body.device_id),
        client_id: None,
        expires_at_ms: None,
    };
    state.db.insert_token(&auth::token_hash(&secret), &token).await?;
    Ok(Json(json!({ "token": secret, "email": user.email })))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    let (hash, _) = authenticate(&state, &headers, TokenKind::Device).await?;
    state.db.delete_token(&hash).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let (_, token) = authenticate(&state, &headers, TokenKind::Device).await?;
    let user = state.db.user_by_id(&token.user_id).await?.ok_or_else(ApiError::unauthorized)?;
    let devices: Vec<Value> = state
        .db
        .devices(&user.id)
        .await?
        .into_iter()
        .map(|d| json!({ "id": d.id, "name": d.name, "last_seen_ms": d.last_seen_ms }))
        .collect();
    Ok(Json(json!({ "email": user.email, "timezone": user.timezone, "devices": devices })))
}

#[derive(Deserialize)]
pub struct Settings {
    /// IANA name of the device's time zone.
    timezone: Option<String>,
    /// The user's category rules, or null to use the built-in ones.
    categories: Option<Vec<CategoryRule>>,
}

/// Devices report the time zone and category rules so the remote MCP
/// endpoint answers the same way the local reports do.
pub async fn settings(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<Settings>) -> Result<StatusCode, ApiError> {
    let (_, token) = authenticate(&state, &headers, TokenKind::Device).await?;
    if let Some(name) = &body.timezone {
        TimeZone::get(name).map_err(|_| ApiError::bad_request("unknown time zone"))?;
    }
    let categories = match &body.categories {
        Some(rules) if !rules.is_empty() => {
            if rules.len() > MAX_CATEGORY_RULES {
                return Err(ApiError::bad_request("too many category rules"));
            }
            for rule in rules {
                rule.validate().map_err(|e| ApiError::bad_request(e.to_string()))?;
            }
            Some(serde_json::to_string(rules).map_err(|e| e.to_string())?)
        }
        _ => None,
    };
    state.db.update_settings(&token.user_id, body.timezone.as_deref(), categories.as_deref()).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct Upload {
    events: Vec<Event>,
}

fn validate_event(event: &Event, device_id: &str) -> Result<(), ApiError> {
    let problem = if !valid_id(&event.id) {
        "invalid event id"
    } else if event.device_id != device_id {
        "an event belongs to another device"
    } else if event.duration_ms < 0 || event.start_ms < 0 || event.updated_at_ms < 0 {
        "negative time in an event"
    } else if event.app.len() > 300 || event.title.len() > 2000 {
        "app or title too long"
    } else if !event.data.is_object() || event.data.to_string().len() > 4000 {
        "event data must be a small JSON object"
    } else {
        return Ok(());
    };
    Err(ApiError::bad_request(problem))
}

/// Receives a batch of new or updated events from a device. Uploading the
/// same events again is harmless.
pub async fn events(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<Upload>) -> Result<Json<Value>, ApiError> {
    let (_, token) = authenticate(&state, &headers, TokenKind::Device).await?;
    let device_id = token.device_id.as_deref().ok_or_else(ApiError::unauthorized)?;
    if body.events.len() > MAX_BATCH {
        return Err(ApiError(StatusCode::PAYLOAD_TOO_LARGE, format!("at most {MAX_BATCH} events per upload")));
    }
    // Keep the newest version of each id: one statement cannot touch a row twice.
    let mut latest: HashMap<&str, &Event> = HashMap::new();
    for event in &body.events {
        validate_event(event, device_id)?;
        let slot = latest.entry(&event.id).or_insert(event);
        if event.updated_at_ms > slot.updated_at_ms {
            *slot = event;
        }
    }
    let batch: Vec<Event> = latest.into_values().cloned().collect();
    state.db.upsert_events(&token.user_id, &batch).await?;
    state.db.touch_device(&token.user_id, device_id, now_ms()).await?;
    Ok(Json(json!({ "accepted": body.events.len() })))
}
