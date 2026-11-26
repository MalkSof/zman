//! A Model Context Protocol server over zman data. Read-only, except for
//! starting and stopping focus sessions when the user has allowed it.
//!
//! The protocol layer here is transport-agnostic: [`Server::handle`] takes
//! one JSON-RPC message and returns the reply, if any. The `zman-mcp` binary
//! feeds it lines from stdin; an HTTP server can feed it request bodies.
//!
//! Tools only ever return aggregates (time per category, app or site),
//! never raw events or window titles.

mod tools;

use jiff::tz::TimeZone;
use serde_json::{Value, json};
use zman_core::{Categories, Event, Range};

/// Serves MCP over stdio on the local database until stdin closes: one
/// JSON-RPC message per line in, one per line out. Nothing else may be
/// written to stdout.
pub fn serve_stdio() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{BufRead, Write};

    let paths = zman_core::Paths::resolve()?;
    let config = zman_core::Config::load(&paths.config_file)?;
    // Opening creates an empty database if nothing was tracked yet, so the
    // tools answer "nothing tracked" instead of failing.
    let store = zman_core::Store::open(&paths.db_file)?;
    let settings = zman_core::Settings::load(&store)?;
    let server = Server {
        source: &store,
        categories: settings.categories(&config, zman_core::Lang::detect()),
        default_tz: TimeZone::system(),
        focus: Some(&store),
        clients: Some(&store),
    };

    let mut stdout = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.handle_text(&line) {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Protocol revisions this server can speak, newest first.
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "zman is an automatic time tracker. These tools report how the user \
spent their time on their devices: totals per category, app and website, a productivity score \
from 0 to 100, and focus blocks. All data is aggregated; window titles are never exposed. \
When time_by_client is offered, use it for questions about clients: hours per client, what each \
paid and the real rate per hour; record_payment saves a payment the user tells you about. \
When start_focus is offered, use it to block distracting websites for a while if the user asks to focus. \
Dates are YYYY-MM-DD in the user's time zone and ranges are inclusive.";

/// Where the server reads events from.
pub trait EventSource {
    /// Events overlapping `range`, in any order.
    fn events_between(&self, range: Range) -> Result<Vec<Event>, String>;
}

impl EventSource for zman_core::Store {
    fn events_between(&self, range: Range) -> Result<Vec<Event>, String> {
        zman_core::Store::events_between(self, range).map_err(|e| e.to_string())
    }
}

/// Starts and stops focus sessions (see `zman_core::settings::Block`).
pub trait FocusControl {
    /// Starts a session of `minutes` and returns when it ends, in
    /// milliseconds since the Unix epoch.
    fn start_focus(&self, minutes: u32) -> Result<i64, String>;
    fn stop_focus(&self) -> Result<(), String>;
}

/// The user's clients and their payments (see `zman_core::client`).
pub trait ClientBook {
    fn clients(&self) -> Result<Vec<zman_core::Client>, String>;
    /// Adds a payment to the client named `name` (ignoring case).
    fn record_payment(&self, name: &str, payment: zman_core::Payment) -> Result<(), String>;
}

impl ClientBook for zman_core::Store {
    fn clients(&self) -> Result<Vec<zman_core::Client>, String> {
        Ok(zman_core::Settings::load(self).map_err(|e| e.to_string())?.clients)
    }

    fn record_payment(&self, name: &str, payment: zman_core::Payment) -> Result<(), String> {
        let mut settings = zman_core::Settings::load(self).map_err(|e| e.to_string())?;
        let names: Vec<String> = settings.clients.iter().map(|c| c.name.clone()).collect();
        let client = settings
            .clients
            .iter_mut()
            .find(|c| c.name.trim().eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| format!("No client named \"{name}\". Clients: {}", names.join(", ")))?;
        client.payments.push(payment);
        settings.save(self).map_err(|e| e.to_string())
    }
}

const FOCUS_NOT_ALLOWED: &str = "The user has not allowed assistants to control focus sessions. \
They can turn it on in the zman app: Focus, then \"Let my AI assistant start focus sessions\".";

impl FocusControl for zman_core::Store {
    fn start_focus(&self, minutes: u32) -> Result<i64, String> {
        let mut settings = zman_core::Settings::load(self).map_err(|e| e.to_string())?;
        if !settings.assistant_focus {
            return Err(FOCUS_NOT_ALLOWED.into());
        }
        let until_ms = zman_core::now_ms() + minutes as i64 * 60_000;
        settings.block.focus_until_ms = until_ms;
        settings.save(self).map_err(|e| e.to_string())?;
        Ok(until_ms)
    }

    fn stop_focus(&self) -> Result<(), String> {
        let mut settings = zman_core::Settings::load(self).map_err(|e| e.to_string())?;
        if !settings.assistant_focus {
            return Err(FOCUS_NOT_ALLOWED.into());
        }
        settings.block.focus_until_ms = 0;
        settings.save(self).map_err(|e| e.to_string())
    }
}

pub struct Server<'a> {
    pub source: &'a dyn EventSource,
    /// Present only where focus sessions can be controlled: on the user's
    /// own computer. Without it the server is strictly read-only.
    pub focus: Option<&'a dyn FocusControl>,
    /// Present only on the user's own computer, where clients and payments live.
    pub clients: Option<&'a dyn ClientBook>,
    pub categories: Categories,
    /// Time zone used when a tool call does not name one.
    pub default_tz: TimeZone,
}

impl Server<'_> {
    /// Handles one JSON-RPC message given as text. Returns the serialized
    /// response, or `None` for notifications.
    pub fn handle_text(&self, text: &str) -> Option<String> {
        let reply = match serde_json::from_str::<Value>(text) {
            Ok(message) => self.handle(&message)?,
            Err(_) => error_reply(Value::Null, -32700, "Parse error"),
        };
        Some(reply.to_string())
    }

    /// Handles one parsed JSON-RPC message.
    pub fn handle(&self, message: &Value) -> Option<Value> {
        let method = message.get("method").and_then(Value::as_str);
        let Some(id) = message.get("id").filter(|id| !id.is_null()).cloned() else {
            // Notifications (and client responses) are never answered.
            return None;
        };
        let Some(method) = method else {
            return Some(error_reply(id, -32600, "Invalid request"));
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        let result = match method {
            "initialize" => Ok(initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tool_definitions() })),
            "tools/call" => self.call_tool(&params),
            _ => Err((-32601, format!("Method not found: {method}"))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error_reply(id, code, &message),
        })
    }

    fn tool_definitions(&self) -> Value {
        let mut list = tools::definitions();
        if self.focus.is_some() {
            if let (Some(list), Value::Array(extra)) = (list.as_array_mut(), tools::focus_definitions()) {
                list.extend(extra);
            }
        }
        if self.clients.is_some() {
            if let (Some(list), Value::Array(extra)) = (list.as_array_mut(), tools::client_definitions()) {
                list.extend(extra);
            }
        }
        list
    }

    fn call_tool(&self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((-32602, "Missing tool name".to_string()))?;
        if !tools::exists(name, self.focus.is_some(), self.clients.is_some()) {
            return Err((-32602, format!("Unknown tool: {name}")));
        }
        let empty = json!({});
        let args = params.get("arguments").filter(|a| a.is_object()).unwrap_or(&empty);

        // Problems with the arguments or the data are reported inside the
        // result, so the model can read them and correct itself.
        Ok(match tools::call(self, name, args) {
            Ok(data) => json!({
                "content": [{ "type": "text", "text": format!("{data:#}") }],
                "isError": false,
            }),
            Err(message) => json!({
                "content": [{ "type": "text", "text": message }],
                "isError": true,
            }),
        })
    }
}

fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
    let version = PROTOCOL_VERSIONS
        .iter()
        .find(|v| **v == requested)
        .unwrap_or(&PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "zman", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn error_reply(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zman_core::{Lang, Store};

    const MIN: i64 = 60_000;
    /// 2026-11-20T00:00:00Z, a Friday.
    const DAY: i64 = 1_795_132_800_000;

    fn store() -> Store {
        let store = Store::open_in_memory().unwrap();
        let add = |app: &str, title: &str, url: &str, start_min: i64, minutes: i64| {
            let start_ms = DAY + start_min * MIN;
            store
                .insert_event(&Event {
                    id: format!("{app}-{start_min}"),
                    device_id: "dev".into(),
                    start_ms,
                    duration_ms: minutes * MIN,
                    app: app.into(),
                    title: title.into(),
                    data: if url.is_empty() { json!({}) } else { json!({ "url": url }) },
                    updated_at_ms: start_ms,
                })
                .unwrap();
        };
        // Friday: 60 min coding from 09:00, 20 min of video, 30 min on GitHub.
        add("code.exe", "secret-project.rs", "", 9 * 60, 60);
        add("chrome.exe", "Funny cats - YouTube", "https://youtube.com/watch", 10 * 60, 20);
        add("chrome.exe", "Private repo", "https://github.com/a/b", 10 * 60 + 20, 30);
        // The day before: 30 min coding.
        add("code.exe", "other.rs", "", -24 * 60 + 9 * 60, 30);
        store
    }

    fn server(store: &Store) -> Server<'_> {
        Server { source: store, categories: Categories::defaults(Lang::En), default_tz: TimeZone::UTC, focus: None, clients: None }
    }

    fn call(server: &Server<'_>, name: &str, args: Value) -> (Value, bool) {
        let request = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": name, "arguments": args },
        });
        let reply = server.handle(&request).unwrap();
        let result = &reply["result"];
        let text = result["content"][0]["text"].as_str().unwrap();
        let is_error = result["isError"].as_bool().unwrap();
        (serde_json::from_str(text).unwrap_or_else(|_| json!(text)), is_error)
    }

    #[test]
    fn initialize_negotiates_version() {
        let store = store();
        let server = server(&store);
        let reply = server
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-03-26", "capabilities": {}}}))
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(reply["result"]["serverInfo"]["name"], "zman");

        let reply = server
            .handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "initialize",
                "params": {"protocolVersion": "1999-01-01"}}))
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_fail() {
        let store = store();
        let server = server(&store);
        assert!(server.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let reply = server.handle(&json!({"jsonrpc": "2.0", "id": 5, "method": "resources/list"})).unwrap();
        assert_eq!(reply["error"]["code"], -32601);
        assert_eq!(reply["id"], 5);
        let reply: Value = serde_json::from_str(&server.handle_text("{not json").unwrap()).unwrap();
        assert_eq!(reply["error"]["code"], -32700);
        assert_eq!(server.handle(&json!({"jsonrpc": "2.0", "id": 9, "method": "ping"})).unwrap()["result"], json!({}));
    }

    #[test]
    fn tools_are_listed_as_read_only() {
        let store = store();
        let reply = server(&store).handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            ["daily_summary", "weekly_summary", "time_by_category", "time_by_app", "compare_periods", "focus_blocks"]
        );
        for tool in tools {
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
            assert_eq!(tool["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn focus_tools_need_the_users_permission() {
        let store = store();
        let server = Server { focus: Some(&store), ..server(&store) };
        let reply = server.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 8);
        assert_eq!(tools[6]["name"], "start_focus");
        assert_eq!(tools[6]["annotations"]["readOnlyHint"], false);

        // Off by default: the assistant is told how the user can allow it.
        let (message, is_error) = call(&server, "start_focus", json!({"minutes": 25}));
        assert!(is_error);
        assert!(message.as_str().unwrap().contains("not allowed"));
        assert_eq!(zman_core::Settings::load(&store).unwrap().block.focus_until_ms, 0);

        let mut settings = zman_core::Settings::load(&store).unwrap();
        settings.assistant_focus = true;
        settings.save(&store).unwrap();
        let (data, is_error) = call(&server, "start_focus", json!({"minutes": 25}));
        assert!(!is_error);
        assert_eq!(data["minutes"], 25);
        assert!(zman_core::Settings::load(&store).unwrap().block.focus_active(zman_core::now_ms()));
        assert!(call(&server, "start_focus", json!({"minutes": 1})).1);

        assert!(!call(&server, "stop_focus", json!({})).1);
        assert_eq!(zman_core::Settings::load(&store).unwrap().block.focus_until_ms, 0);

        // A server without focus control (the remote one) does not know the tools.
        let reply = self::server(&store)
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "start_focus"}}))
            .unwrap();
        assert_eq!(reply["error"]["code"], -32602);
    }

    #[test]
    fn daily_summary_reports_aggregates() {
        let store = store();
        let (data, is_error) = call(&server(&store), "daily_summary", json!({"date": "2026-11-20"}));
        assert!(!is_error);
        assert_eq!(data["date"], "2026-11-20");
        assert_eq!(data["tracked_minutes"], 110);
        assert_eq!(data["categories"][0]["name"], "Development");
        assert_eq!(data["categories"][0]["minutes"], 90);
        assert_eq!(data["apps"][0]["name"], "code");
        assert_eq!(data["sites"][0]["name"], "github.com");
        assert_eq!(data["hourly_minutes"].as_array().unwrap().len(), 24);
        assert_eq!(data["hourly_minutes"][9], 60);
        assert_eq!(data["hourly_minutes"][10], 50);
        // 90 productive minutes at 100 and 20 distracting at 0.
        assert_eq!(data["productivity"], 82);
    }

    #[test]
    fn window_titles_never_leak() {
        let store = store();
        let server = server(&store);
        for (tool, args) in [
            ("daily_summary", json!({"date": "2026-11-20"})),
            ("weekly_summary", json!({"date": "2026-11-20"})),
            ("time_by_app", json!({"from": "2026-11-19", "to": "2026-11-20"})),
            ("focus_blocks", json!({"from": "2026-11-19", "to": "2026-11-20"})),
        ] {
            let text = call(&server, tool, args).0.to_string();
            assert!(!text.contains("secret-project"), "{tool} leaked a title");
            assert!(!text.contains("Private repo"), "{tool} leaked a title");
            assert!(!text.contains("/a/b"), "{tool} leaked a URL path");
        }
    }

    #[test]
    fn weekly_summary_has_seven_days() {
        let store = store();
        let (data, _) = call(&server(&store), "weekly_summary", json!({"date": "2026-11-20"}));
        assert_eq!(data["from"], "2026-11-16");
        assert_eq!(data["to"], "2026-11-22");
        let days = data["days"].as_array().unwrap();
        assert_eq!(days.len(), 7);
        assert_eq!(days[3]["tracked_minutes"], 30);
        assert_eq!(days[4]["tracked_minutes"], 110);
        assert_eq!(days[4]["weekday"], "Friday");
        assert_eq!(data["tracked_minutes"], 140);
    }

    #[test]
    fn ranges_by_category_and_app() {
        let store = store();
        let server = server(&store);
        let range = json!({"from": "2026-11-19", "to": "2026-11-20"});
        let (data, _) = call(&server, "time_by_category", range.clone());
        assert_eq!(data["categories"][0]["name"], "Development");
        assert_eq!(data["categories"][0]["minutes"], 120);

        let mut args = range;
        args["limit"] = json!(1);
        let (data, _) = call(&server, "time_by_app", args);
        assert_eq!(data["apps"].as_array().unwrap().len(), 1);
        assert_eq!(data["apps"][0]["minutes"], 90);
        assert_eq!(data["other_apps_minutes"], 50);
    }

    #[test]
    fn compare_periods_reports_changes() {
        let store = store();
        let (data, _) = call(
            &server(&store),
            "compare_periods",
            json!({"a_from": "2026-11-19", "a_to": "2026-11-19", "b_from": "2026-11-20", "b_to": "2026-11-20"}),
        );
        assert_eq!(data["a"]["tracked_minutes"], 30);
        assert_eq!(data["b"]["tracked_minutes"], 110);
        assert_eq!(data["change"]["tracked_minutes"], 80);
        let dev = data["categories"].as_array().unwrap().iter().find(|c| c["name"] == "Development").unwrap();
        assert_eq!(dev["a_minutes"], 30);
        assert_eq!(dev["b_minutes"], 90);
        assert_eq!(dev["change_minutes"], 60);
    }

    #[test]
    fn focus_blocks_are_found() {
        let store = store();
        let (data, _) = call(
            &server(&store),
            "focus_blocks",
            json!({"from": "2026-11-20", "to": "2026-11-20", "min_minutes": 25}),
        );
        let blocks = data["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["start"], "2026-11-20T09:00");
        assert_eq!(blocks[0]["minutes"], 60);
        assert_eq!(blocks[0]["category"], "Development");
        assert_eq!(data["total_focus_minutes"], 90);
    }

    #[test]
    fn time_zone_argument_shifts_the_day() {
        let store = store();
        let (data, _) = call(
            &server(&store),
            "daily_summary",
            json!({"date": "2026-11-20", "timezone": "America/Argentina/Buenos_Aires"}),
        );
        // 09:00 UTC is 06:00 in Buenos Aires.
        assert_eq!(data["hourly_minutes"][6], 60);
    }

    #[test]
    fn bad_arguments_are_tool_errors() {
        let store = store();
        let server = server(&store);
        for (tool, args) in [
            ("daily_summary", json!({"date": "tomorrow"})),
            ("time_by_category", json!({"from": "2026-11-20"})),
            ("time_by_category", json!({"from": "2026-11-20", "to": "2026-11-01"})),
            ("time_by_category", json!({"from": "2020-01-01", "to": "2026-11-01"})),
            ("daily_summary", json!({"timezone": "Mars/Olympus"})),
        ] {
            let (message, is_error) = call(&server, tool, args.clone());
            assert!(is_error, "{tool} {args} should fail, got {message}");
        }
        let reply = server
            .handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "delete_everything"}}))
            .unwrap();
        assert_eq!(reply["error"]["code"], -32602);
    }
}
