/// A span of time during which one window was active.
///
/// Events are append-only. The only mutation ever applied to a stored event
/// is changing `duration_ms` (heartbeat extension or AFK trim), and every
/// such change bumps `updated_at_ms` so a future sync can pick it up with a
/// cursor on that column.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// ULID: globally unique and sortable by creation time.
    pub id: String,
    /// The installation that recorded the event.
    pub device_id: String,
    /// Start, in milliseconds since the Unix epoch (UTC).
    pub start_ms: i64,
    pub duration_ms: i64,
    /// Application name as reported by the platform (e.g. `chrome.exe`).
    pub app: String,
    /// Window title.
    pub title: String,
    /// Free-form extras (a JSON object), for data later phases will add.
    pub data: serde_json::Value,
    pub updated_at_ms: i64,
}

impl Event {
    pub fn end_ms(&self) -> i64 {
        self.start_ms + self.duration_ms
    }
}
