//! Core of zman: event model, SQLite storage, config and tracking logic.
//! Nothing in this crate is platform specific.

pub mod category;
pub mod config;
pub mod error;
pub mod event;
pub mod export;
pub mod glob;
pub mod lang;
pub mod paths;
pub mod period;
pub mod report;
pub mod settings;
pub mod store;
pub mod tracker;

pub use category::{Categories, CategoryRule};
pub use config::Config;
pub use error::{Error, Result};
pub use event::Event;
pub use lang::Lang;
pub use paths::Paths;
pub use period::Range;
pub use settings::Settings;
pub use store::Store;
pub use tracker::{Tick, Tracker, WindowInfo};

/// Current wall-clock time as milliseconds since the Unix epoch (UTC).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
