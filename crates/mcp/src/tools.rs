//! Tool definitions and implementations.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde_json::{Value, json};
use zman_core::event::display_app;
use zman_core::period::{self, format_duration};
use zman_core::report::{self, Summary};
use zman_core::{Lang, Range};

use crate::Server;

/// Longest range a single call may cover.
const MAX_RANGE_DAYS: i64 = 366;
const DEFAULT_FOCUS_MINUTES: i64 = 25;
/// Interruptions up to this long do not break a focus block.
const FOCUS_BREAK_MS: i64 = 2 * 60_000;

const NAMES: &[&str] = &[
    "daily_summary",
    "weekly_summary",
    "time_by_category",
    "time_by_app",
    "compare_periods",
    "focus_blocks",
];

const FOCUS_NAMES: &[&str] = &["start_focus", "stop_focus"];
const CLIENT_NAMES: &[&str] = &["time_by_client", "record_payment"];

pub fn exists(name: &str, with_focus: bool, with_clients: bool) -> bool {
    NAMES.contains(&name) || (with_focus && FOCUS_NAMES.contains(&name)) || (with_clients && CLIENT_NAMES.contains(&name))
}

/// Clients and payments. Only offered where zman runs on the user's own computer.
pub fn client_definitions() -> Value {
    let date = |description: &str| json!({ "type": "string", "format": "date", "description": description });
    json!([
        {
            "name": "time_by_client",
            "description": "Hours spent on each of the user's clients over a date range, recognised \
                automatically from the activity, plus what each client paid in that range and the \
                real rate per hour (paid / hours). Own projects count hours but have no rate. Also \
                returns time that matched no client.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": date("First day of the range (YYYY-MM-DD)."),
                    "to": date("Last day of the range, inclusive (YYYY-MM-DD)."),
                    "timezone": { "type": "string", "description": "IANA time zone. Defaults to the user's." },
                },
                "required": ["from", "to"],
            },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "record_payment",
            "description": "Saves a payment from one of the user's clients (money received or billed), \
                so time_by_client can compute the rate per hour. Use it when the user tells you a \
                client paid them. The client must already exist in the zman app.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client": { "type": "string", "description": "Client name, as in the zman app." },
                    "amount": { "type": "number", "exclusiveMinimum": 0 },
                    "currency": { "type": "string", "default": "ARS", "description": "ARS, USD..." },
                    "date": date("Day of the payment (YYYY-MM-DD). Defaults to today."),
                    "note": { "type": "string", "description": "Optional, e.g. the invoice number." },
                },
                "required": ["client", "amount"],
            },
            "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false },
        },
    ])
}

/// The two tools that act instead of reporting. Only offered where zman
/// runs on the user's own computer.
pub fn focus_definitions() -> Value {
    let annotations = json!({ "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false });
    json!([
        {
            "name": "start_focus",
            "description": "Starts a focus session: for the given minutes, zman blocks distracting \
                websites (entertainment, social media) in the user's browser. Use it when the user \
                asks to focus or to block distractions. The user must have allowed assistants to \
                do this in the zman app.",
            "inputSchema": {
                "type": "object",
                "properties": { "minutes": {
                    "type": "integer", "minimum": 5, "maximum": 480, "default": 50,
                    "description": "Length of the session in minutes.",
                } },
                "required": [],
            },
            "annotations": annotations,
        },
        {
            "name": "stop_focus",
            "description": "Ends the current focus session, unblocking the websites it was blocking.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] },
            "annotations": annotations,
        },
    ])
}

pub fn definitions() -> Value {
    let date = |description: &str| json!({ "type": "string", "format": "date", "description": description });
    let timezone = json!({
        "type": "string",
        "description": "IANA time zone, e.g. America/Argentina/Buenos_Aires. Defaults to the user's.",
    });
    let tool = |name: &str, description: &str, properties: Value, required: &[&str]| {
        let mut properties = properties;
        properties["timezone"] = timezone.clone();
        json!({
            "name": name,
            "description": description,
            "inputSchema": { "type": "object", "properties": properties, "required": required },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        })
    };
    let from_to = json!({
        "from": date("First day of the range (YYYY-MM-DD)."),
        "to": date("Last day of the range, inclusive (YYYY-MM-DD)."),
    });

    json!([
        tool(
            "daily_summary",
            "Summary of one day: time tracked, productivity score (0-100), time per category, \
             top apps and websites, minutes tracked in each hour, and focus time.",
            json!({ "date": date("Day to summarize (YYYY-MM-DD). Defaults to today.") }),
            &[],
        ),
        tool(
            "weekly_summary",
            "Summary of a week (Monday to Sunday): totals, productivity score, a breakdown per \
             day, time per category and top apps.",
            json!({ "date": date("Any day of the week to summarize (YYYY-MM-DD). Defaults to the current week.") }),
            &[],
        ),
        tool(
            "time_by_category",
            "Time spent in each category (e.g. Development, Communication, Entertainment) over a date range.",
            from_to.clone(),
            &["from", "to"],
        ),
        tool(
            "time_by_app",
            "Time spent in each application, and each website when known, over a date range.",
            {
                let mut properties = from_to.clone();
                properties["limit"] = json!({
                    "type": "integer", "minimum": 1, "maximum": 100, "default": 20,
                    "description": "Maximum number of apps and of sites to return.",
                });
                properties
            },
            &["from", "to"],
        ),
        tool(
            "compare_periods",
            "Compares two date ranges (A, the baseline, against B): time tracked, productivity \
             score and time per category, with the change from A to B.",
            json!({
                "a_from": date("First day of period A (YYYY-MM-DD)."),
                "a_to": date("Last day of period A, inclusive."),
                "b_from": date("First day of period B (YYYY-MM-DD)."),
                "b_to": date("Last day of period B, inclusive."),
            }),
            &["a_from", "a_to", "b_from", "b_to"],
        ),
        tool(
            "focus_blocks",
            "Stretches of sustained productive work over a date range: when each started and \
             ended, how long it lasted and its main category. Interruptions under 2 minutes do \
             not break a block.",
            {
                let mut properties = from_to;
                properties["min_minutes"] = json!({
                    "type": "integer", "minimum": 5, "maximum": 240, "default": DEFAULT_FOCUS_MINUTES,
                    "description": "Shortest block to report, in minutes of productive time.",
                });
                properties
            },
            &["from", "to"],
        ),
    ])
}

pub fn call(server: &Server<'_>, name: &str, args: &Value) -> Result<Value, String> {
    let tz = match args.get("timezone").and_then(Value::as_str) {
        Some(name) => TimeZone::get(name).map_err(|_| format!("Unknown time zone: {name}"))?,
        None => server.default_tz.clone(),
    };
    let ctx = Ctx { server, tz };
    match name {
        "daily_summary" => ctx.daily_summary(args),
        "weekly_summary" => ctx.weekly_summary(args),
        "time_by_category" => ctx.time_by_category(args),
        "time_by_app" => ctx.time_by_app(args),
        "compare_periods" => ctx.compare_periods(args),
        "focus_blocks" => ctx.focus_blocks(args),
        "start_focus" => ctx.start_focus(args),
        "stop_focus" => ctx.stop_focus(),
        "time_by_client" => ctx.time_by_client(args),
        "record_payment" => ctx.record_payment(args),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

struct Ctx<'a> {
    server: &'a Server<'a>,
    tz: TimeZone,
}

impl Ctx<'_> {
    fn start_focus(&self, args: &Value) -> Result<Value, String> {
        let focus = self.server.focus.ok_or("Focus sessions are not available here.")?;
        let minutes = args.get("minutes").and_then(Value::as_i64).unwrap_or(50);
        if !(5..=480).contains(&minutes) {
            return Err("minutes must be between 5 and 480".into());
        }
        let until_ms = focus.start_focus(minutes as u32)?;
        Ok(json!({
            "focus": "on",
            "minutes": minutes,
            "ends_at": self.local_time(until_ms),
            "note": "Distracting websites are now blocked in browsers that have the zman extension.",
        }))
    }

    fn stop_focus(&self) -> Result<Value, String> {
        let focus = self.server.focus.ok_or("Focus sessions are not available here.")?;
        focus.stop_focus()?;
        Ok(json!({ "focus": "off" }))
    }

    fn time_by_client(&self, args: &Value) -> Result<Value, String> {
        let book = self.server.clients.ok_or("Clients are not available here.")?;
        let clients = book.clients()?;
        if clients.is_empty() {
            return Err("The user has no clients yet. They can add them in the zman app, under Clients.".into());
        }
        let (from, to, range) = self.range(args, "from", "to")?;
        let events = self.server.source.events_between(range)?;
        let report = zman_core::client::report(&events, range, &clients, &self.tz);
        let money = |list: &[zman_core::client::Money]| -> Value {
            list.iter().map(|m| json!({ "currency": m.currency, "amount": m.amount })).collect()
        };
        Ok(json!({
            "from": from.to_string(),
            "to": to.to_string(),
            "tracked_minutes": minutes(report.total_ms),
            "unassigned_minutes": minutes(report.unassigned_ms),
            "clients": report.clients.iter().map(|c| json!({
                "name": c.name,
                "own_project": c.own,
                "minutes": minutes(c.ms),
                "hours": format_duration(c.ms),
                "paid": money(&c.paid),
                "per_hour": money(&c.per_hour),
            })).collect::<Vec<_>>(),
        }))
    }

    fn record_payment(&self, args: &Value) -> Result<Value, String> {
        let book = self.server.clients.ok_or("Clients are not available here.")?;
        let client = args.get("client").and_then(Value::as_str).ok_or("client is required")?;
        let amount = args.get("amount").and_then(Value::as_f64).filter(|a| *a > 0.0).ok_or("amount must be a positive number")?;
        let date = optional_date(args, "date")?.unwrap_or_else(|| period::today(&self.tz));
        let currency = args.get("currency").and_then(Value::as_str).unwrap_or("ARS").trim().to_uppercase();
        let note = args.get("note").and_then(Value::as_str).unwrap_or("").to_string();
        book.record_payment(client, zman_core::Payment { date: date.to_string(), amount, currency: currency.clone(), note })?;
        Ok(json!({ "saved": true, "client": client, "amount": amount, "currency": currency, "date": date.to_string() }))
    }

    fn daily_summary(&self, args: &Value) -> Result<Value, String> {
        let date = optional_date(args, "date")?.unwrap_or_else(|| period::today(&self.tz));
        let range = period::day_range(date, &self.tz).map_err(err)?;
        let events = self.server.source.events_between(range)?;
        let categories = &self.server.categories;

        let summary = report::summarize(&events, range, categories);
        let edges = period::hour_edges(date, &self.tz).map_err(err)?;
        let hourly: Vec<i64> = report::timeline(&events, &edges, categories)
            .iter()
            .map(|hour| minutes(hour.total_ms))
            .collect();
        let focus = report::focus_blocks(
            &events,
            range,
            categories,
            DEFAULT_FOCUS_MINUTES * 60_000,
            FOCUS_BREAK_MS,
        );
        let focus_ms: i64 = focus.iter().map(|b| b.productive_ms).sum();

        let mut out = totals(&summary);
        out["date"] = json!(date.to_string());
        out["weekday"] = json!(period::weekday_name(date, Lang::En));
        out["categories"] = categories_json(&summary);
        out["apps"] = apps_json(&summary, 10);
        out["sites"] = sites_json(&summary, 10);
        out["hourly_minutes"] = json!(hourly);
        out["focus"] = json!({
            "blocks": focus.len(),
            "minutes": minutes(focus_ms),
            "duration": format_duration(focus_ms),
        });
        Ok(out)
    }

    fn weekly_summary(&self, args: &Value) -> Result<Value, String> {
        let date = optional_date(args, "date")?.unwrap_or_else(|| period::today(&self.tz));
        let monday = period::week_start(date).map_err(err)?;
        let range = period::days_range(monday, 7, &self.tz).map_err(err)?;
        let events = self.server.source.events_between(range)?;
        let categories = &self.server.categories;

        let summary = report::summarize(&events, range, categories);
        let edges = period::day_edges(monday, 7, &self.tz).map_err(err)?;
        let mut days = Vec::new();
        for (i, day) in report::timeline(&events, &edges, categories).iter().enumerate() {
            let day_date = period::add_days(monday, i as i64).map_err(err)?;
            let mut entry = totals(day);
            entry["date"] = json!(day_date.to_string());
            entry["weekday"] = json!(period::weekday_name(day_date, Lang::En));
            days.push(entry);
        }

        let mut out = totals(&summary);
        out["from"] = json!(monday.to_string());
        out["to"] = json!(period::add_days(monday, 6).map_err(err)?.to_string());
        out["days"] = json!(days);
        out["categories"] = categories_json(&summary);
        out["apps"] = apps_json(&summary, 10);
        out["sites"] = sites_json(&summary, 10);
        Ok(out)
    }

    fn time_by_category(&self, args: &Value) -> Result<Value, String> {
        let (from, to, summary) = self.range_summary(args, "from", "to")?;
        let mut out = totals(&summary);
        out["from"] = json!(from.to_string());
        out["to"] = json!(to.to_string());
        out["categories"] = categories_json(&summary);
        Ok(out)
    }

    fn time_by_app(&self, args: &Value) -> Result<Value, String> {
        let limit = match args.get("limit") {
            None | Some(Value::Null) => 20,
            Some(value) => value
                .as_u64()
                .filter(|n| (1..=100).contains(n))
                .ok_or("limit must be an integer between 1 and 100")? as usize,
        };
        let (from, to, summary) = self.range_summary(args, "from", "to")?;
        let shown: i64 = summary.apps.iter().take(limit).map(|a| a.ms).sum();
        let mut out = totals(&summary);
        out["from"] = json!(from.to_string());
        out["to"] = json!(to.to_string());
        out["apps"] = apps_json(&summary, limit);
        out["other_apps_minutes"] = json!(minutes(summary.total_ms - shown));
        out["sites"] = sites_json(&summary, limit);
        Ok(out)
    }

    fn compare_periods(&self, args: &Value) -> Result<Value, String> {
        let (a_from, a_to, a) = self.range_summary(args, "a_from", "a_to")?;
        let (b_from, b_to, b) = self.range_summary(args, "b_from", "b_to")?;

        // Every category present in either period, biggest combined first.
        let mut names: Vec<(&str, i8)> = Vec::new();
        for category in a.categories.iter().chain(&b.categories) {
            if !names.iter().any(|(name, _)| *name == category.name) {
                names.push((&category.name, category.score));
            }
        }
        let ms_in = |summary: &Summary, name: &str| {
            summary.categories.iter().find(|c| c.name == name).map_or(0, |c| c.ms)
        };
        names.sort_by_key(|(name, _)| -(ms_in(&a, name) + ms_in(&b, name)));
        let categories: Vec<Value> = names
            .iter()
            .map(|(name, score)| {
                let (a_ms, b_ms) = (ms_in(&a, name), ms_in(&b, name));
                json!({
                    "name": name,
                    "productivity_score": score,
                    "a_minutes": minutes(a_ms),
                    "b_minutes": minutes(b_ms),
                    "change_minutes": minutes(b_ms) - minutes(a_ms),
                    "change_percent": percent_change(a_ms, b_ms),
                })
            })
            .collect();

        let period_json = |from: Date, to: Date, summary: &Summary| {
            let mut out = totals(summary);
            out["from"] = json!(from.to_string());
            out["to"] = json!(to.to_string());
            out
        };
        let productivity_change = match (a.productivity, b.productivity) {
            (Some(a), Some(b)) => json!(b as i64 - a as i64),
            _ => Value::Null,
        };
        Ok(json!({
            "a": period_json(a_from, a_to, &a),
            "b": period_json(b_from, b_to, &b),
            "change": {
                "tracked_minutes": minutes(b.total_ms) - minutes(a.total_ms),
                "tracked_percent": percent_change(a.total_ms, b.total_ms),
                "productivity": productivity_change,
            },
            "categories": categories,
        }))
    }

    fn focus_blocks(&self, args: &Value) -> Result<Value, String> {
        let min_minutes = match args.get("min_minutes") {
            None | Some(Value::Null) => DEFAULT_FOCUS_MINUTES,
            Some(value) => value
                .as_i64()
                .filter(|n| (5..=240).contains(n))
                .ok_or("min_minutes must be an integer between 5 and 240")?,
        };
        let (from, to, range) = self.range(args, "from", "to")?;
        let events = self.server.source.events_between(range)?;
        let blocks = report::focus_blocks(
            &events,
            range,
            &self.server.categories,
            min_minutes * 60_000,
            FOCUS_BREAK_MS,
        );
        let total_ms: i64 = blocks.iter().map(|b| b.productive_ms).sum();
        let longest_ms = blocks.iter().map(|b| b.end_ms - b.start_ms).max().unwrap_or(0);
        let list: Vec<Value> = blocks
            .iter()
            .map(|block| {
                json!({
                    "start": self.local_time(block.start_ms),
                    "end": self.local_time(block.end_ms),
                    "minutes": minutes(block.end_ms - block.start_ms),
                    "productive_minutes": minutes(block.productive_ms),
                    "category": block.category,
                })
            })
            .collect();
        Ok(json!({
            "from": from.to_string(),
            "to": to.to_string(),
            "min_minutes": min_minutes,
            "total_focus_minutes": minutes(total_ms),
            "total_focus": format_duration(total_ms),
            "longest_block_minutes": minutes(longest_ms),
            "blocks": list,
        }))
    }

    /// Reads an inclusive date range from `args`.
    fn range(&self, args: &Value, from_key: &str, to_key: &str) -> Result<(Date, Date, Range), String> {
        let from = optional_date(args, from_key)?.ok_or(format!("{from_key} is required (YYYY-MM-DD)"))?;
        let to = optional_date(args, to_key)?.ok_or(format!("{to_key} is required (YYYY-MM-DD)"))?;
        if to < from {
            return Err(format!("{to_key} must not be before {from_key}"));
        }
        let range = Range {
            start_ms: period::start_of_day(from, &self.tz).map_err(err)?,
            end_ms: period::start_of_day(period::add_days(to, 1).map_err(err)?, &self.tz).map_err(err)?,
        };
        if range.duration_ms() > MAX_RANGE_DAYS * 86_400_000 {
            return Err(format!("the range is too long: at most {MAX_RANGE_DAYS} days"));
        }
        Ok((from, to, range))
    }

    fn range_summary(&self, args: &Value, from_key: &str, to_key: &str) -> Result<(Date, Date, Summary), String> {
        let (from, to, range) = self.range(args, from_key, to_key)?;
        let events = self.server.source.events_between(range)?;
        Ok((from, to, report::summarize(&events, range, &self.server.categories)))
    }

    /// "2026-11-20T09:00" in the call's time zone.
    fn local_time(&self, ms: i64) -> String {
        Timestamp::from_millisecond(ms)
            .map(|ts| ts.to_zoned_datetime(self.tz.clone()).strftime("%Y-%m-%dT%H:%M").to_string())
            .unwrap_or_default()
    }
}

fn err(e: zman_core::Error) -> String {
    e.to_string()
}

fn optional_date(args: &Value, key: &str) -> Result<Option<Date>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => period::parse_date(text)
            .map(Some)
            .map_err(|_| format!("{key}: invalid date \"{text}\", expected YYYY-MM-DD")),
        Some(_) => Err(format!("{key} must be a string (YYYY-MM-DD)")),
    }
}

fn minutes(ms: i64) -> i64 {
    (ms as f64 / 60_000.0).round() as i64
}

fn percent_change(a_ms: i64, b_ms: i64) -> Value {
    if a_ms == 0 {
        return Value::Null;
    }
    json!(((b_ms - a_ms) as f64 / a_ms as f64 * 100.0).round() as i64)
}

fn share(ms: i64, total_ms: i64) -> i64 {
    (ms as f64 / total_ms.max(1) as f64 * 100.0).round() as i64
}

fn totals(summary: &Summary) -> Value {
    json!({
        "tracked_minutes": minutes(summary.total_ms),
        "tracked": format_duration(summary.total_ms),
        "productivity": summary.productivity,
    })
}

fn categories_json(summary: &Summary) -> Value {
    summary
        .categories
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "minutes": minutes(c.ms),
                "duration": format_duration(c.ms),
                "share_percent": share(c.ms, summary.total_ms),
                "productivity_score": c.score,
            })
        })
        .collect()
}

fn apps_json(summary: &Summary, limit: usize) -> Value {
    summary
        .apps
        .iter()
        .take(limit)
        .map(|a| {
            json!({
                "name": display_app(&a.app),
                "minutes": minutes(a.ms),
                "duration": format_duration(a.ms),
                "share_percent": share(a.ms, summary.total_ms),
                "category": a.category,
            })
        })
        .collect()
}

fn sites_json(summary: &Summary, limit: usize) -> Value {
    summary
        .domains
        .iter()
        .take(limit)
        .map(|d| {
            json!({
                "name": d.domain,
                "minutes": minutes(d.ms),
                "duration": format_duration(d.ms),
                "share_percent": share(d.ms, summary.total_ms),
                "category": d.category,
            })
        })
        .collect()
}
