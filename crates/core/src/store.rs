use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Row, params};
use ulid::Ulid;

use crate::error::Result;
use crate::event::Event;
use crate::period::Range;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS events (
    id            TEXT PRIMARY KEY,
    device_id     TEXT NOT NULL,
    start_ms      INTEGER NOT NULL,
    duration_ms   INTEGER NOT NULL,
    app           TEXT NOT NULL,
    title         TEXT NOT NULL,
    data          TEXT NOT NULL DEFAULT '{}',
    updated_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS events_start ON events (start_ms);
CREATE INDEX IF NOT EXISTS events_updated ON events (updated_at_ms);
";

const EVENT_COLUMNS: &str = "id, device_id, start_ms, duration_ms, app, title, data, updated_at_ms";

/// SQLite-backed event storage.
pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        // WAL lets the CLI read while the daemon writes.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Store::init(conn)
    }

    pub fn open_in_memory() -> Result<Store> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Store> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        Ok(Store { conn })
    }

    /// The persistent id of this installation, created on first use.
    pub fn device_id(&self) -> Result<String> {
        self.conn.execute(
            "INSERT OR IGNORE INTO meta (key, value) VALUES ('device_id', ?1)",
            params![Ulid::generate().to_string()],
        )?;
        let id = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = 'device_id'", [], |row| row.get(0))?;
        Ok(id)
    }

    /// Small key/value settings kept next to the data (sync account, cursor).
    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        let value = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |row| row.get(0))
            .optional()?;
        Ok(value)
    }

    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn meta_delete(&self, key: &str) -> Result<()> {
        self.conn.execute("DELETE FROM meta WHERE key = ?1", params![key])?;
        Ok(())
    }

    /// Events created or changed after a sync cursor, oldest change first.
    /// The cursor is the `(updated_at_ms, id)` of the last event already sent.
    pub fn events_after(&self, updated_at_ms: i64, id: &str, limit: u32) -> Result<Vec<Event>> {
        let sql = format!(
            "SELECT {EVENT_COLUMNS} FROM events
             WHERE (updated_at_ms, id) > (?1, ?2)
             ORDER BY updated_at_ms, id LIMIT ?3"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let events = stmt
            .query_map(params![updated_at_ms, id, limit], read_event)?
            .collect::<rusqlite::Result<Vec<Event>>>()?;
        Ok(events)
    }

    pub fn insert_event(&self, event: &Event) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events (id, device_id, start_ms, duration_ms, app, title, data, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.id,
                event.device_id,
                event.start_ms,
                event.duration_ms,
                event.app,
                event.title,
                event.data.to_string(),
                event.updated_at_ms,
            ],
        )?;
        Ok(())
    }

    /// The only mutation events ever receive: a new duration.
    pub fn set_duration(&self, id: &str, duration_ms: i64, updated_at_ms: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE events SET duration_ms = ?2, updated_at_ms = ?3 WHERE id = ?1",
            params![id, duration_ms, updated_at_ms],
        )?;
        Ok(())
    }

    /// The most recently started event, if any.
    pub fn last_event(&self) -> Result<Option<Event>> {
        let sql = format!("SELECT {EVENT_COLUMNS} FROM events ORDER BY start_ms DESC, id DESC LIMIT 1");
        let event = self.conn.query_row(&sql, [], read_event).optional()?;
        Ok(event)
    }

    /// The latest `limit` events, oldest first.
    pub fn recent_events(&self, limit: u32) -> Result<Vec<Event>> {
        let sql = format!("SELECT {EVENT_COLUMNS} FROM events ORDER BY start_ms DESC, id DESC LIMIT ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut events = stmt
            .query_map(params![limit], read_event)?
            .collect::<rusqlite::Result<Vec<Event>>>()?;
        events.reverse();
        Ok(events)
    }

    /// Events that overlap `range`, oldest first. Events are returned whole;
    /// callers clip them to the range.
    pub fn events_between(&self, range: Range) -> Result<Vec<Event>> {
        let sql = format!(
            "SELECT {EVENT_COLUMNS} FROM events
             WHERE start_ms < ?2 AND start_ms + duration_ms > ?1
             ORDER BY start_ms, id"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let events = stmt
            .query_map(params![range.start_ms, range.end_ms], read_event)?
            .collect::<rusqlite::Result<Vec<Event>>>()?;
        Ok(events)
    }

    pub fn count_events(&self) -> Result<u64> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;
        Ok(count as u64)
    }
}

fn read_event(row: &Row<'_>) -> rusqlite::Result<Event> {
    let data: String = row.get(6)?;
    Ok(Event {
        id: row.get(0)?,
        device_id: row.get(1)?,
        start_ms: row.get(2)?,
        duration_ms: row.get(3)?,
        app: row.get(4)?,
        title: row.get(5)?,
        // Stored by us as valid JSON; fall back to null rather than fail a read.
        data: serde_json::from_str(&data).unwrap_or(serde_json::Value::Null),
        updated_at_ms: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, start_ms: i64) -> Event {
        Event {
            id: id.into(),
            device_id: "dev".into(),
            start_ms,
            duration_ms: 1000,
            app: "app.exe".into(),
            title: "Title".into(),
            data: serde_json::json!({"k": "v"}),
            updated_at_ms: start_ms,
        }
    }

    #[test]
    fn device_id_is_stable() {
        let store = Store::open_in_memory().unwrap();
        let first = store.device_id().unwrap();
        assert_eq!(first.len(), 26);
        assert_eq!(store.device_id().unwrap(), first);
    }

    #[test]
    fn insert_and_read_back() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.last_event().unwrap(), None);

        let a = event("A", 1000);
        store.insert_event(&a).unwrap();
        assert_eq!(store.last_event().unwrap(), Some(a));
        assert_eq!(store.count_events().unwrap(), 1);
    }

    #[test]
    fn set_duration_bumps_updated_at() {
        let store = Store::open_in_memory().unwrap();
        store.insert_event(&event("A", 1000)).unwrap();
        store.set_duration("A", 5000, 6000).unwrap();

        let stored = store.last_event().unwrap().unwrap();
        assert_eq!(stored.duration_ms, 5000);
        assert_eq!(stored.updated_at_ms, 6000);
    }

    #[test]
    fn recent_events_are_the_latest_in_chronological_order() {
        let store = Store::open_in_memory().unwrap();
        for (id, start) in [("A", 1000), ("B", 2000), ("C", 3000)] {
            store.insert_event(&event(id, start)).unwrap();
        }
        let ids: Vec<String> = store.recent_events(2).unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, vec!["B", "C"]);
    }

    #[test]
    fn events_between_returns_overlapping_events() {
        let store = Store::open_in_memory().unwrap();
        // Each lasts 1000 ms.
        for (id, start) in [("A", 0), ("B", 1500), ("C", 3000)] {
            store.insert_event(&event(id, start)).unwrap();
        }
        let ids = |start_ms, end_ms| -> Vec<String> {
            let range = Range { start_ms, end_ms };
            store.events_between(range).unwrap().into_iter().map(|e| e.id).collect()
        };
        assert_eq!(ids(0, 5000), vec!["A", "B", "C"]);
        assert_eq!(ids(900, 1600), vec!["A", "B"]);
        // Touching the boundary is not overlapping.
        assert_eq!(ids(1000, 1500), Vec::<String>::new());
        assert_eq!(ids(2500, 3000), Vec::<String>::new());
    }

    #[test]
    fn meta_values_can_be_set_replaced_and_deleted() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.meta_get("k").unwrap(), None);
        store.meta_set("k", "one").unwrap();
        store.meta_set("k", "two").unwrap();
        assert_eq!(store.meta_get("k").unwrap().as_deref(), Some("two"));
        store.meta_delete("k").unwrap();
        assert_eq!(store.meta_get("k").unwrap(), None);
    }

    #[test]
    fn events_after_walks_changes_in_order_without_repeats() {
        let store = Store::open_in_memory().unwrap();
        // B and C share the same updated_at_ms: the id breaks the tie.
        for (id, start, updated) in [("A", 0, 10), ("C", 2000, 20), ("B", 1000, 20), ("D", 3000, 30)] {
            let mut e = event(id, start);
            e.updated_at_ms = updated;
            store.insert_event(&e).unwrap();
        }
        let ids = |ms, id: &str, limit| -> Vec<String> {
            store.events_after(ms, id, limit).unwrap().into_iter().map(|e| e.id).collect()
        };
        assert_eq!(ids(0, "", 10), vec!["A", "B", "C", "D"]);
        assert_eq!(ids(0, "", 2), vec!["A", "B"]);
        assert_eq!(ids(20, "B", 10), vec!["C", "D"]);
        assert_eq!(ids(30, "D", 10), Vec::<String>::new());

        // An event that grows moves past the cursor again.
        store.set_duration("A", 9000, 40).unwrap();
        assert_eq!(ids(30, "D", 10), vec!["A"]);
    }

    #[test]
    fn duplicate_ids_are_rejected() {
        let store = Store::open_in_memory().unwrap();
        store.insert_event(&event("A", 1000)).unwrap();
        assert!(store.insert_event(&event("A", 2000)).is_err());
    }
}
