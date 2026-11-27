//! Storage. `Db::Postgres` is the real thing; `Db::Memory` keeps everything
//! in RAM and exists for tests and for trying the server without a database.
//! Both sit behind the same methods so the rest of the server cannot tell.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sqlx::postgres::{PgPool, PgPoolOptions, PgRow};
use sqlx::Row;
use zman_core::{Event, Range};

pub type DbResult<T> = Result<T, String>;

fn db_err(e: sqlx::Error) -> String {
    format!("database error: {e}")
}

const SCHEMA: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS users (
        id            TEXT PRIMARY KEY,
        email         TEXT NOT NULL UNIQUE,
        password_hash TEXT NOT NULL,
        timezone      TEXT,
        categories    TEXT,
        created_at_ms BIGINT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS devices (
        user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        id           TEXT NOT NULL,
        name         TEXT NOT NULL,
        last_seen_ms BIGINT NOT NULL,
        PRIMARY KEY (user_id, id)
    )",
    "CREATE TABLE IF NOT EXISTS tokens (
        hash          TEXT PRIMARY KEY,
        user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        kind          TEXT NOT NULL,
        device_id     TEXT,
        client_id     TEXT,
        expires_at_ms BIGINT
    )",
    "CREATE TABLE IF NOT EXISTS events (
        user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        id            TEXT NOT NULL,
        device_id     TEXT NOT NULL,
        start_ms      BIGINT NOT NULL,
        duration_ms   BIGINT NOT NULL,
        app           TEXT NOT NULL,
        title         TEXT NOT NULL,
        data          JSONB NOT NULL,
        updated_at_ms BIGINT NOT NULL,
        PRIMARY KEY (user_id, id)
    )",
    "CREATE INDEX IF NOT EXISTS events_user_start ON events (user_id, start_ms)",
    "CREATE TABLE IF NOT EXISTS oauth_clients (
        id            TEXT PRIMARY KEY,
        name          TEXT NOT NULL,
        redirect_uris TEXT NOT NULL,
        created_at_ms BIGINT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS oauth_codes (
        hash           TEXT PRIMARY KEY,
        client_id      TEXT NOT NULL,
        user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        redirect_uri   TEXT NOT NULL,
        code_challenge TEXT NOT NULL,
        expires_at_ms  BIGINT NOT NULL
    )",
];

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    /// IANA time zone last reported by one of the user's devices.
    pub timezone: Option<String>,
    /// The user's category rules as JSON, if they customized them.
    pub categories: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub last_seen_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    /// Long-lived, held by a device to upload its events.
    Device,
    /// Short-lived OAuth access token, for the MCP endpoint.
    Access,
    /// OAuth refresh token.
    Refresh,
}

impl TokenKind {
    fn as_str(self) -> &'static str {
        match self {
            TokenKind::Device => "device",
            TokenKind::Access => "access",
            TokenKind::Refresh => "refresh",
        }
    }

    fn parse(text: &str) -> Option<TokenKind> {
        match text {
            "device" => Some(TokenKind::Device),
            "access" => Some(TokenKind::Access),
            "refresh" => Some(TokenKind::Refresh),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub user_id: String,
    pub kind: TokenKind,
    pub device_id: Option<String>,
    pub client_id: Option<String>,
    pub expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OauthClient {
    pub id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OauthCode {
    pub client_id: String,
    pub user_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub expires_at_ms: i64,
}

#[derive(Default)]
pub struct Memory {
    users: Vec<User>,
    devices: HashMap<(String, String), Device>,
    tokens: HashMap<String, Token>,
    events: HashMap<(String, String), Event>,
    clients: HashMap<String, OauthClient>,
    codes: HashMap<String, OauthCode>,
}

#[derive(Clone)]
pub enum Db {
    Postgres(PgPool),
    Memory(Arc<Mutex<Memory>>),
}

impl Db {
    pub fn memory() -> Db {
        Db::Memory(Arc::default())
    }

    /// Connects and creates the tables if they do not exist yet.
    pub async fn connect(url: &str) -> DbResult<Db> {
        let pool = PgPoolOptions::new().max_connections(10).connect(url).await.map_err(db_err)?;
        for statement in SCHEMA {
            sqlx::query(*statement).execute(&pool).await.map_err(db_err)?;
        }
        Ok(Db::Postgres(pool))
    }

    // ---------- Users ----------

    /// Creates a user. Returns `None` if the email is already registered.
    pub async fn create_user(&self, email: &str, password_hash: &str, now_ms: i64) -> DbResult<Option<String>> {
        let id = ulid::Ulid::generate().to_string();
        match self {
            Db::Postgres(pool) => {
                let done = sqlx::query(
                    "INSERT INTO users (id, email, password_hash, created_at_ms) VALUES ($1, $2, $3, $4)
                     ON CONFLICT (email) DO NOTHING",
                )
                .bind(&id)
                .bind(email)
                .bind(password_hash)
                .bind(now_ms)
                .execute(pool)
                .await
                .map_err(db_err)?;
                Ok((done.rows_affected() == 1).then_some(id))
            }
            Db::Memory(memory) => {
                let mut memory = memory.lock().unwrap();
                if memory.users.iter().any(|u| u.email == email) {
                    return Ok(None);
                }
                memory.users.push(User {
                    id: id.clone(),
                    email: email.to_string(),
                    password_hash: password_hash.to_string(),
                    timezone: None,
                    categories: None,
                });
                Ok(Some(id))
            }
        }
    }

    pub async fn user_by_email(&self, email: &str) -> DbResult<Option<User>> {
        match self {
            Db::Postgres(pool) => sqlx::query(
                "SELECT id, email, password_hash, timezone, categories FROM users WHERE email = $1",
            )
            .bind(email)
            .fetch_optional(pool)
            .await
            .map_err(db_err)?
            .map(|row| read_user(&row))
            .transpose(),
            Db::Memory(memory) => Ok(memory.lock().unwrap().users.iter().find(|u| u.email == email).cloned()),
        }
    }

    pub async fn user_by_id(&self, id: &str) -> DbResult<Option<User>> {
        match self {
            Db::Postgres(pool) => sqlx::query(
                "SELECT id, email, password_hash, timezone, categories FROM users WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(db_err)?
            .map(|row| read_user(&row))
            .transpose(),
            Db::Memory(memory) => Ok(memory.lock().unwrap().users.iter().find(|u| u.id == id).cloned()),
        }
    }

    /// The users created most recently, newest first, and how many there are
    /// in all. Ids are ULIDs, so they sort by creation time.
    pub async fn recent_users(&self, limit: u32) -> DbResult<(Vec<User>, u64)> {
        match self {
            Db::Postgres(pool) => {
                let rows = sqlx::query("SELECT id, email, password_hash, timezone, categories FROM users ORDER BY id DESC LIMIT $1")
                    .bind(limit as i64)
                    .fetch_all(pool)
                    .await
                    .map_err(db_err)?;
                let users = rows.iter().map(read_user).collect::<DbResult<Vec<User>>>()?;
                let total: i64 = sqlx::query("SELECT COUNT(*) AS n FROM users")
                    .fetch_one(pool)
                    .await
                    .map_err(db_err)?
                    .try_get("n")
                    .map_err(db_err)?;
                Ok((users, total as u64))
            }
            Db::Memory(memory) => {
                let memory = memory.lock().unwrap();
                let mut users = memory.users.clone();
                users.sort_by(|a, b| b.id.cmp(&a.id));
                users.truncate(limit as usize);
                Ok((users, memory.users.len() as u64))
            }
        }
    }

    pub async fn update_settings(&self, user_id: &str, timezone: Option<&str>, categories: Option<&str>) -> DbResult<()> {
        match self {
            Db::Postgres(pool) => {
                sqlx::query("UPDATE users SET timezone = $2, categories = $3 WHERE id = $1")
                    .bind(user_id)
                    .bind(timezone)
                    .bind(categories)
                    .execute(pool)
                    .await
                    .map_err(db_err)?;
            }
            Db::Memory(memory) => {
                if let Some(user) = memory.lock().unwrap().users.iter_mut().find(|u| u.id == user_id) {
                    user.timezone = timezone.map(str::to_string);
                    user.categories = categories.map(str::to_string);
                }
            }
        }
        Ok(())
    }

    // ---------- Devices ----------

    /// Registers a device or refreshes its name and last-seen time.
    pub async fn upsert_device(&self, user_id: &str, device_id: &str, name: &str, now_ms: i64) -> DbResult<()> {
        match self {
            Db::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO devices (user_id, id, name, last_seen_ms) VALUES ($1, $2, $3, $4)
                     ON CONFLICT (user_id, id) DO UPDATE SET name = EXCLUDED.name, last_seen_ms = EXCLUDED.last_seen_ms",
                )
                .bind(user_id)
                .bind(device_id)
                .bind(name)
                .bind(now_ms)
                .execute(pool)
                .await
                .map_err(db_err)?;
            }
            Db::Memory(memory) => {
                memory.lock().unwrap().devices.insert(
                    (user_id.to_string(), device_id.to_string()),
                    Device { id: device_id.to_string(), name: name.to_string(), last_seen_ms: now_ms },
                );
            }
        }
        Ok(())
    }

    pub async fn touch_device(&self, user_id: &str, device_id: &str, now_ms: i64) -> DbResult<()> {
        match self {
            Db::Postgres(pool) => {
                sqlx::query("UPDATE devices SET last_seen_ms = $3 WHERE user_id = $1 AND id = $2")
                    .bind(user_id)
                    .bind(device_id)
                    .bind(now_ms)
                    .execute(pool)
                    .await
                    .map_err(db_err)?;
            }
            Db::Memory(memory) => {
                let key = (user_id.to_string(), device_id.to_string());
                if let Some(device) = memory.lock().unwrap().devices.get_mut(&key) {
                    device.last_seen_ms = now_ms;
                }
            }
        }
        Ok(())
    }

    pub async fn devices(&self, user_id: &str) -> DbResult<Vec<Device>> {
        match self {
            Db::Postgres(pool) => {
                let rows = sqlx::query("SELECT id, name, last_seen_ms FROM devices WHERE user_id = $1 ORDER BY name, id")
                    .bind(user_id)
                    .fetch_all(pool)
                    .await
                    .map_err(db_err)?;
                rows.iter()
                    .map(|row| {
                        Ok(Device {
                            id: row.try_get("id").map_err(db_err)?,
                            name: row.try_get("name").map_err(db_err)?,
                            last_seen_ms: row.try_get("last_seen_ms").map_err(db_err)?,
                        })
                    })
                    .collect()
            }
            Db::Memory(memory) => {
                let memory = memory.lock().unwrap();
                let mut devices: Vec<Device> =
                    memory.devices.iter().filter(|((user, _), _)| user == user_id).map(|(_, d)| d.clone()).collect();
                devices.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
                Ok(devices)
            }
        }
    }

    // ---------- Tokens ----------

    pub async fn insert_token(&self, hash: &str, token: &Token) -> DbResult<()> {
        match self {
            Db::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO tokens (hash, user_id, kind, device_id, client_id, expires_at_ms)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(hash)
                .bind(&token.user_id)
                .bind(token.kind.as_str())
                .bind(&token.device_id)
                .bind(&token.client_id)
                .bind(token.expires_at_ms)
                .execute(pool)
                .await
                .map_err(db_err)?;
            }
            Db::Memory(memory) => {
                memory.lock().unwrap().tokens.insert(hash.to_string(), token.clone());
            }
        }
        Ok(())
    }

    pub async fn token(&self, hash: &str) -> DbResult<Option<Token>> {
        match self {
            Db::Postgres(pool) => {
                let row = sqlx::query(
                    "SELECT user_id, kind, device_id, client_id, expires_at_ms FROM tokens WHERE hash = $1",
                )
                .bind(hash)
                .fetch_optional(pool)
                .await
                .map_err(db_err)?;
                let Some(row) = row else {
                    return Ok(None);
                };
                let kind: String = row.try_get("kind").map_err(db_err)?;
                Ok(Some(Token {
                    user_id: row.try_get("user_id").map_err(db_err)?,
                    kind: TokenKind::parse(&kind).ok_or("unknown token kind")?,
                    device_id: row.try_get("device_id").map_err(db_err)?,
                    client_id: row.try_get("client_id").map_err(db_err)?,
                    expires_at_ms: row.try_get("expires_at_ms").map_err(db_err)?,
                }))
            }
            Db::Memory(memory) => Ok(memory.lock().unwrap().tokens.get(hash).cloned()),
        }
    }

    pub async fn delete_token(&self, hash: &str) -> DbResult<()> {
        match self {
            Db::Postgres(pool) => {
                sqlx::query("DELETE FROM tokens WHERE hash = $1").bind(hash).execute(pool).await.map_err(db_err)?;
            }
            Db::Memory(memory) => {
                memory.lock().unwrap().tokens.remove(hash);
            }
        }
        Ok(())
    }

    // ---------- Events ----------

    /// Stores a batch of events from one device. An event already stored is
    /// only replaced by a newer version of itself (larger `updated_at_ms`)
    /// from the same device, so uploads can be repeated safely.
    ///
    /// `events` must not contain the same id twice.
    pub async fn upsert_events(&self, user_id: &str, events: &[Event]) -> DbResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        match self {
            Db::Postgres(pool) => {
                let column = |f: fn(&Event) -> String| events.iter().map(f).collect::<Vec<String>>();
                let numbers = |f: fn(&Event) -> i64| events.iter().map(f).collect::<Vec<i64>>();
                sqlx::query(
                    "INSERT INTO events (user_id, id, device_id, start_ms, duration_ms, app, title, data, updated_at_ms)
                     SELECT $1, t.id, t.device_id, t.start_ms, t.duration_ms, t.app, t.title, t.data::jsonb, t.updated_at_ms
                     FROM UNNEST($2::text[], $3::text[], $4::bigint[], $5::bigint[], $6::text[], $7::text[], $8::text[], $9::bigint[])
                          AS t(id, device_id, start_ms, duration_ms, app, title, data, updated_at_ms)
                     ON CONFLICT (user_id, id) DO UPDATE
                        SET duration_ms = EXCLUDED.duration_ms,
                            data = EXCLUDED.data,
                            updated_at_ms = EXCLUDED.updated_at_ms
                      WHERE events.updated_at_ms < EXCLUDED.updated_at_ms
                        AND events.device_id = EXCLUDED.device_id",
                )
                .bind(user_id)
                .bind(column(|e| e.id.clone()))
                .bind(column(|e| e.device_id.clone()))
                .bind(numbers(|e| e.start_ms))
                .bind(numbers(|e| e.duration_ms))
                .bind(column(|e| e.app.clone()))
                .bind(column(|e| e.title.clone()))
                .bind(column(|e| e.data.to_string()))
                .bind(numbers(|e| e.updated_at_ms))
                .execute(pool)
                .await
                .map_err(db_err)?;
            }
            Db::Memory(memory) => {
                let mut memory = memory.lock().unwrap();
                for event in events {
                    let key = (user_id.to_string(), event.id.clone());
                    let replace = match memory.events.get(&key) {
                        Some(stored) => {
                            stored.updated_at_ms < event.updated_at_ms && stored.device_id == event.device_id
                        }
                        None => true,
                    };
                    if replace {
                        memory.events.insert(key, event.clone());
                    }
                }
            }
        }
        Ok(())
    }

    /// The user's events, from all their devices, that overlap `range`.
    pub async fn events_between(&self, user_id: &str, range: Range) -> DbResult<Vec<Event>> {
        match self {
            Db::Postgres(pool) => {
                let rows = sqlx::query(
                    "SELECT id, device_id, start_ms, duration_ms, app, title, data::text AS data, updated_at_ms
                     FROM events
                     WHERE user_id = $1 AND start_ms < $3 AND start_ms + duration_ms > $2
                     ORDER BY start_ms, id",
                )
                .bind(user_id)
                .bind(range.start_ms)
                .bind(range.end_ms)
                .fetch_all(pool)
                .await
                .map_err(db_err)?;
                rows.iter()
                    .map(|row| {
                        let data: String = row.try_get("data").map_err(db_err)?;
                        Ok(Event {
                            id: row.try_get("id").map_err(db_err)?,
                            device_id: row.try_get("device_id").map_err(db_err)?,
                            start_ms: row.try_get("start_ms").map_err(db_err)?,
                            duration_ms: row.try_get("duration_ms").map_err(db_err)?,
                            app: row.try_get("app").map_err(db_err)?,
                            title: row.try_get("title").map_err(db_err)?,
                            data: serde_json::from_str(&data).unwrap_or(serde_json::Value::Null),
                            updated_at_ms: row.try_get("updated_at_ms").map_err(db_err)?,
                        })
                    })
                    .collect()
            }
            Db::Memory(memory) => {
                let memory = memory.lock().unwrap();
                let mut events: Vec<Event> = memory
                    .events
                    .iter()
                    .filter(|((user, _), e)| user == user_id && e.start_ms < range.end_ms && e.end_ms() > range.start_ms)
                    .map(|(_, e)| e.clone())
                    .collect();
                events.sort_by(|a, b| (a.start_ms, &a.id).cmp(&(b.start_ms, &b.id)));
                Ok(events)
            }
        }
    }

    // ---------- OAuth ----------

    pub async fn insert_client(&self, client: &OauthClient, now_ms: i64) -> DbResult<()> {
        match self {
            Db::Postgres(pool) => {
                let uris = serde_json::to_string(&client.redirect_uris).map_err(|e| e.to_string())?;
                sqlx::query("INSERT INTO oauth_clients (id, name, redirect_uris, created_at_ms) VALUES ($1, $2, $3, $4)")
                    .bind(&client.id)
                    .bind(&client.name)
                    .bind(uris)
                    .bind(now_ms)
                    .execute(pool)
                    .await
                    .map_err(db_err)?;
            }
            Db::Memory(memory) => {
                memory.lock().unwrap().clients.insert(client.id.clone(), client.clone());
            }
        }
        Ok(())
    }

    pub async fn client(&self, id: &str) -> DbResult<Option<OauthClient>> {
        match self {
            Db::Postgres(pool) => {
                let row = sqlx::query("SELECT id, name, redirect_uris FROM oauth_clients WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(db_err)?;
                let Some(row) = row else {
                    return Ok(None);
                };
                let uris: String = row.try_get("redirect_uris").map_err(db_err)?;
                Ok(Some(OauthClient {
                    id: row.try_get("id").map_err(db_err)?,
                    name: row.try_get("name").map_err(db_err)?,
                    redirect_uris: serde_json::from_str(&uris).map_err(|e| e.to_string())?,
                }))
            }
            Db::Memory(memory) => Ok(memory.lock().unwrap().clients.get(id).cloned()),
        }
    }

    pub async fn insert_code(&self, hash: &str, code: &OauthCode) -> DbResult<()> {
        match self {
            Db::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO oauth_codes (hash, client_id, user_id, redirect_uri, code_challenge, expires_at_ms)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(hash)
                .bind(&code.client_id)
                .bind(&code.user_id)
                .bind(&code.redirect_uri)
                .bind(&code.code_challenge)
                .bind(code.expires_at_ms)
                .execute(pool)
                .await
                .map_err(db_err)?;
            }
            Db::Memory(memory) => {
                memory.lock().unwrap().codes.insert(hash.to_string(), code.clone());
            }
        }
        Ok(())
    }

    /// Returns the code and deletes it: an authorization code works once.
    pub async fn take_code(&self, hash: &str) -> DbResult<Option<OauthCode>> {
        match self {
            Db::Postgres(pool) => {
                let row = sqlx::query(
                    "DELETE FROM oauth_codes WHERE hash = $1
                     RETURNING client_id, user_id, redirect_uri, code_challenge, expires_at_ms",
                )
                .bind(hash)
                .fetch_optional(pool)
                .await
                .map_err(db_err)?;
                let Some(row) = row else {
                    return Ok(None);
                };
                Ok(Some(OauthCode {
                    client_id: row.try_get("client_id").map_err(db_err)?,
                    user_id: row.try_get("user_id").map_err(db_err)?,
                    redirect_uri: row.try_get("redirect_uri").map_err(db_err)?,
                    code_challenge: row.try_get("code_challenge").map_err(db_err)?,
                    expires_at_ms: row.try_get("expires_at_ms").map_err(db_err)?,
                }))
            }
            Db::Memory(memory) => Ok(memory.lock().unwrap().codes.remove(hash)),
        }
    }
}

fn read_user(row: &PgRow) -> DbResult<User> {
    Ok(User {
        id: row.try_get("id").map_err(db_err)?,
        email: row.try_get("email").map_err(db_err)?,
        password_hash: row.try_get("password_hash").map_err(db_err)?,
        timezone: row.try_get("timezone").map_err(db_err)?,
        categories: row.try_get("categories").map_err(db_err)?,
    })
}
