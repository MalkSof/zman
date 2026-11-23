//! CSV export of raw events, for spreadsheets and other tools.

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::category::Categories;
use crate::event::Event;

/// Quotes a field when it needs it (RFC 4180).
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn local(ms: i64, tz: &TimeZone) -> String {
    Timestamp::from_millisecond(ms)
        .map(|ts| ts.to_zoned_datetime(tz.clone()).strftime("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// One line per event, oldest first, with times in `tz`.
pub fn csv(events: &[Event], categories: &Categories, tz: &TimeZone) -> String {
    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|e| e.start_ms);

    let mut out = String::from("start,end,seconds,app,title,url,category\r\n");
    for event in sorted {
        let (category, _) = categories.classify(event);
        let row = [
            local(event.start_ms, tz),
            local(event.end_ms(), tz),
            (event.duration_ms / 1000).to_string(),
            field(&event.app),
            field(&event.title),
            field(event.url().unwrap_or("")),
            field(category),
        ];
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn rows_are_sorted_quoted_and_categorized() {
        let event = |id: &str, start_ms: i64, app: &str, title: &str, url: &str| Event {
            id: id.into(),
            device_id: "d".into(),
            start_ms,
            duration_ms: 90_000,
            app: app.into(),
            title: title.into(),
            data: if url.is_empty() { serde_json::json!({}) } else { serde_json::json!({ "url": url }) },
            updated_at_ms: start_ms,
        };
        let events = [
            event("b", 3_600_000, "chrome.exe", "Cats, \"the\" video", "https://youtube.com/watch"),
            event("a", 0, "code.exe", "main.rs", ""),
        ];
        let text = csv(&events, &Categories::defaults(Lang::En), &TimeZone::UTC);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "start,end,seconds,app,title,url,category");
        assert_eq!(lines[1], "1970-01-01 00:00:00,1970-01-01 00:01:30,90,code.exe,main.rs,,Development");
        assert_eq!(
            lines[2],
            "1970-01-01 01:00:00,1970-01-01 01:01:30,90,chrome.exe,\"Cats, \"\"the\"\" video\",https://youtube.com/watch,Entertainment"
        );
    }
}
