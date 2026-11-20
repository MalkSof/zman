//! Builds the JSON document the dashboard draws: one call, one view.

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use serde_json::{Value, json};
use zman_core::event::display_app;
use zman_core::period;
use zman_core::report::{self, Summary};
use zman_core::{Categories, Lang, Range, Store, now_ms};

const FOCUS_MIN_MS: i64 = 25 * 60_000;
const FOCUS_BREAK_MS: i64 = 2 * 60_000;
const LIST_LIMIT: usize = 8;

pub struct View<'a> {
    pub store: &'a Store,
    pub categories: &'a Categories,
    pub lang: Lang,
    pub tz: &'a TimeZone,
}

impl View<'_> {
    /// `period` is "day" or "week"; `date` (YYYY-MM-DD) defaults to today.
    pub fn build(&self, period: &str, date: Option<&str>) -> Result<Value, String> {
        let today = period::today(self.tz);
        let date = match date {
            Some(text) => period::parse_date(text).map_err(text_of)?,
            None => today,
        };
        match period {
            "week" => self.week(date, today),
            _ => self.day(date, today),
        }
        .map_err(text_of)
    }

    fn day(&self, date: Date, today: Date) -> zman_core::Result<Value> {
        let lang = self.lang;
        let range = period::day_range(date, self.tz)?;
        let edges = period::hour_edges(date, self.tz)?;
        let long = period::format_long_date(date, lang);
        let title = if date == today {
            format!("{} · {long}", lang.pick("Today", "Hoy"))
        } else {
            capitalize(&long)
        };

        let mut out = self.common(range, period::day_range(period::add_days(date, -1)?, self.tz)?)?;
        let events = self.store.events_between(range)?;
        let now = now_ms();
        let buckets: Vec<Value> = report::timeline(&events, &edges, self.categories)
            .iter()
            .enumerate()
            .map(|(hour, bucket)| {
                let mut value = bucket_json(bucket, now);
                value["label"] = json!(format!("{hour:02}"));
                value["show_label"] = json!(hour % 3 == 0);
                value["tooltip"] = json!(format!(
                    "{} – {}",
                    self.clock(bucket.range.start_ms),
                    self.clock(bucket.range.end_ms)
                ));
                value
            })
            .collect();

        out["period"] = json!("day");
        out["date"] = json!(date.to_string());
        out["title"] = json!(title);
        out["is_current"] = json!(date == today);
        out["prev_date"] = json!(period::add_days(date, -1)?.to_string());
        out["next_date"] = if date < today { json!(period::add_days(date, 1)?.to_string()) } else { Value::Null };
        out["buckets"] = json!(buckets);
        Ok(out)
    }

    fn week(&self, date: Date, today: Date) -> zman_core::Result<Value> {
        let lang = self.lang;
        let monday = period::week_start(date)?;
        let this_monday = period::week_start(today)?;
        let range = period::days_range(monday, 7, self.tz)?;
        let edges = period::day_edges(monday, 7, self.tz)?;
        let span = format!(
            "{} – {}",
            period::format_short_date(monday, lang),
            period::format_short_date(period::add_days(monday, 6)?, lang)
        );
        let title = if monday == this_monday {
            format!("{} · {span}", lang.pick("This week", "Esta semana"))
        } else {
            span
        };

        let previous = period::days_range(period::add_days(monday, -7)?, 7, self.tz)?;
        let mut out = self.common(range, previous)?;
        let events = self.store.events_between(range)?;
        let now = now_ms();
        let mut buckets = Vec::new();
        for (i, bucket) in report::timeline(&events, &edges, self.categories).iter().enumerate() {
            let day = period::add_days(monday, i as i64)?;
            let weekday: String = period::weekday_name(day, lang).chars().take(3).collect();
            let mut value = bucket_json(bucket, now);
            value["label"] = json!(format!("{} {}", capitalize(&weekday), day.day()));
            value["show_label"] = json!(true);
            value["tooltip"] = json!(capitalize(&period::format_long_date(day, lang)));
            buckets.push(value);
        }

        out["period"] = json!("week");
        out["date"] = json!(monday.to_string());
        out["title"] = json!(title);
        out["is_current"] = json!(monday == this_monday);
        out["prev_date"] = json!(period::add_days(monday, -7)?.to_string());
        out["next_date"] =
            if monday < this_monday { json!(period::add_days(monday, 7)?.to_string()) } else { Value::Null };
        out["buckets"] = json!(buckets);
        Ok(out)
    }

    /// Everything that does not depend on whether the view is a day or a week.
    fn common(&self, range: Range, previous: Range) -> zman_core::Result<Value> {
        let events = self.store.events_between(range)?;
        let summary = report::summarize(&events, range, self.categories);
        let focus = report::focus_blocks(&events, range, self.categories, FOCUS_MIN_MS, FOCUS_BREAK_MS);

        let before = self.store.events_between(previous)?;
        let prev_summary = report::summarize(&before, previous, self.categories);
        let prev_focus = report::focus_blocks(&before, previous, self.categories, FOCUS_MIN_MS, FOCUS_BREAK_MS);

        let focus_blocks: Vec<Value> = focus
            .iter()
            .map(|block| {
                json!({
                    "start_ms": block.start_ms,
                    "end_ms": block.end_ms,
                    "productive_ms": block.productive_ms,
                    "category": block.category,
                    "start_label": self.clock(block.start_ms),
                    "end_label": self.clock(block.end_ms),
                })
            })
            .collect();

        let (order, colors) = self.palette();

        Ok(json!({
            "lang": self.lang.pick("en", "es"),
            "range_start_ms": range.start_ms,
            "range_end_ms": range.end_ms,
            "total_ms": summary.total_ms,
            "productivity": summary.productivity,
            "focus_ms": focus.iter().map(|b| b.productive_ms).sum::<i64>(),
            "focus_blocks": focus_blocks,
            "previous": {
                "total_ms": prev_summary.total_ms,
                "productivity": prev_summary.productivity,
                "focus_ms": prev_focus.iter().map(|b| b.productive_ms).sum::<i64>(),
            },
            "category_order": order,
            "category_colors": colors,
            "categories": summary.categories.iter().map(|c| json!({
                "name": c.name, "score": c.score, "ms": c.ms,
            })).collect::<Vec<_>>(),
            "apps": summary.apps.iter().take(LIST_LIMIT).map(|a| json!({
                "name": display_app(&a.app), "category": a.category, "score": a.score, "ms": a.ms,
            })).collect::<Vec<_>>(),
            "sites": summary.domains.iter().take(LIST_LIMIT).map(|d| json!({
                "name": d.domain, "category": d.category, "score": d.score, "ms": d.ms,
            })).collect::<Vec<_>>(),
        }))
    }

    /// Stacking order and color slot (1-8, or 0 for neutral gray) of every
    /// category. Productive categories come first and take colors from the
    /// start of the palette; distracting ones go last and take them from the
    /// end, where the warm reds are. Colors depend only on the rules, never
    /// on how much time a category has in the view.
    fn palette(&self) -> (Vec<String>, serde_json::Map<String, Value>) {
        const SLOTS: usize = 8;
        let mut list = self.categories.list();
        let uncategorized = list.pop().map(|(name, _)| name.to_string()).unwrap_or_default();
        list.sort_by_key(|(_, score)| -score); // stable: rule order within a score

        let distracting = list.iter().filter(|(_, score)| *score < 0).count();
        let positive = list.len() - distracting;
        let mut colors = serde_json::Map::new();
        for (i, (name, score)) in list.iter().enumerate() {
            let slot = if *score < 0 {
                // Last distracting category gets slot 8, the one before 7...
                let from_end = list.len() - 1 - i;
                if from_end < SLOTS.saturating_sub(positive.min(SLOTS)) { SLOTS - from_end } else { 0 }
            } else if i < SLOTS { i + 1 } else { 0 };
            colors.insert(name.to_string(), json!(slot));
        }
        colors.insert(uncategorized.clone(), json!(0));

        let mut order: Vec<String> = list.iter().map(|(name, _)| name.to_string()).collect();
        // Uncategorized sits between the neutral and the distracting groups.
        order.insert(positive, uncategorized);
        (order, colors)
    }

    /// "09:30" in the user's time zone.
    fn clock(&self, ms: i64) -> String {
        Timestamp::from_millisecond(ms)
            .map(|ts| ts.to_zoned_datetime(self.tz.clone()).strftime("%H:%M").to_string())
            .unwrap_or_default()
    }
}

fn bucket_json(bucket: &Summary, now: i64) -> Value {
    let parts: serde_json::Map<String, Value> =
        bucket.categories.iter().map(|c| (c.name.clone(), json!(c.ms))).collect();
    json!({
        "total_ms": bucket.total_ms,
        "productivity": bucket.productivity,
        "parts": parts,
        "is_now": bucket.range.start_ms <= now && now < bucket.range.end_ms,
    })
}

fn text_of(e: zman_core::Error) -> String {
    e.to_string()
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
