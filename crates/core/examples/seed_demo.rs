//! Fills a database with two weeks of made-up activity, to try the reports
//! and the dashboard without waiting for real data.
//!
//! ```sh
//! ZMAN_HOME=./demo cargo run -p zman-core --example seed_demo
//! ZMAN_HOME=./demo cargo run -p zman-cli -- week
//! ```

use jiff::tz::TimeZone;
use zman_core::period::{add_days, start_of_day, today};
use zman_core::{Event, Paths, Store};

const MIN: i64 = 60_000;

/// (app, title, url, typical minutes)
const ACTIVITIES: &[(&str, &str, &str, i64)] = &[
    ("Code.exe", "tracker.rs - zman - Visual Studio Code", "", 34),
    ("Code.exe", "report.rs - zman - Visual Studio Code", "", 26),
    ("WindowsTerminal.exe", "cargo test", "", 9),
    ("chrome.exe", "MalkSof/zman - Google Chrome", "https://github.com/MalkSof/zman", 12),
    ("chrome.exe", "rusqlite - Rust - Google Chrome", "https://docs.rs/rusqlite", 8),
    ("slack.exe", "general - Slack", "", 6),
    ("chrome.exe", "Inbox - Gmail - Google Chrome", "https://mail.google.com/mail/u/0/", 7),
    ("chrome.exe", "Sprint planning - Google Docs", "https://docs.google.com/document/d/1", 14),
    ("figma.exe", "Dashboard - Figma", "", 18),
    ("chrome.exe", "Lo-fi beats - YouTube - Google Chrome", "https://www.youtube.com/watch", 11),
    ("chrome.exe", "Home / X - Google Chrome", "https://x.com/home", 5),
    ("WhatsApp.exe", "WhatsApp", "", 4),
    ("chrome.exe", "Hacker News - Google Chrome", "https://news.ycombinator.com/", 6),
    ("explorer.exe", "Downloads", "", 2),
    ("Spotify.exe", "Spotify Premium", "", 3),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve()?;
    let store = Store::open(&paths.db_file)?;
    let device_id = store.device_id()?;
    let tz = TimeZone::system();
    let today = today(&tz);

    // A tiny deterministic generator: the demo looks the same on every run.
    let mut seed: u64 = 0x5EED_2026;
    let mut next = |bound: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % bound
    };

    let mut count = 0;
    for days_ago in (0..14).rev() {
        let date = add_days(today, -days_ago)?;
        let weekend = date.weekday().to_monday_zero_offset() >= 5;
        let day_start = start_of_day(date, &tz)?;
        // Workdays: 9:00 to ~18:30 with a lunch break. Weekends: a short session.
        let (mut cursor, end) = if weekend {
            (day_start + (11 * 60 + next(90) as i64) * MIN, day_start + (13 * 60) * MIN)
        } else {
            (day_start + (9 * 60 + next(40) as i64) * MIN, day_start + (18 * 60 + next(60) as i64) * MIN)
        };
        let lunch = day_start + 13 * 60 * MIN;
        let end = if days_ago == 0 { end.min(zman_core::now_ms()) } else { end };

        while cursor < end {
            if !weekend && cursor >= lunch && cursor < lunch + 50 * MIN {
                cursor = lunch + (50 + next(20) as i64) * MIN;
                continue;
            }
            // Mornings lean towards the first (productive) activities.
            let morning = cursor < lunch;
            let pool = if morning && next(10) < 7 { 9 } else { ACTIVITIES.len() as u64 };
            let (app, title, url, minutes) = ACTIVITIES[next(pool) as usize];
            let duration = ((minutes * (50 + next(100) as i64)) / 100).max(1) * MIN;
            let duration = duration.min(end - cursor);
            let data = if url.is_empty() { serde_json::json!({}) } else { serde_json::json!({ "url": url }) };
            store.insert_event(&Event {
                id: ulid::Ulid::generate().to_string(),
                device_id: device_id.clone(),
                start_ms: cursor,
                duration_ms: duration,
                app: app.to_string(),
                title: title.to_string(),
                data,
                updated_at_ms: cursor + duration,
            })?;
            count += 1;
            // Occasionally step away for a few minutes.
            cursor += duration + if next(6) == 0 { next(12) as i64 * MIN } else { 0 };
        }
    }
    println!("Inserted {count} demo events into {}", paths.db_file.display());
    Ok(())
}
