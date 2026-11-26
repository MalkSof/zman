//! zman desktop app: tracks in the background, lives in the system tray and
//! shows the dashboard.

// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod connections;
mod dashboard;
mod nudges;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use jiff::tz::TimeZone;
use serde_json::{Value, json};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use zman_core::{Config, Lang, Paths, Settings, Store, now_ms, period};
use zman_daemon::{Control, RunError, sync};

/// Passed by the autostart entry so the app starts in the tray.
const HIDDEN_FLAG: &str = "--hidden";

enum Tracking {
    Starting,
    Running,
    /// A separate `zman-daemon` is doing the tracking.
    External,
    Failed(String),
}

/// The tray menu entries, kept so their text can follow the language.
struct TrayItems {
    open: MenuItem<Wry>,
    pause: CheckMenuItem<Wry>,
    quit: MenuItem<Wry>,
}

struct AppState {
    paths: Paths,
    lang: Mutex<Lang>,
    control: Arc<Control>,
    tracking: Mutex<Tracking>,
    tray: Mutex<Option<TrayItems>>,
}

fn tray_labels(lang: Lang) -> (&'static str, &'static str, &'static str) {
    (
        lang.pick("Open zman", "Abrir zman"),
        lang.pick("Pause tracking", "Pausar el registro"),
        lang.pick("Quit", "Salir"),
    )
}

fn main() {
    let paths = match Paths::resolve() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("zman: {e}");
            std::process::exit(1);
        }
    };
    let lang = Lang::detect();

    // `zman-app --dump-dashboard day|week [date]` prints the dashboard data
    // and exits: handy for debugging and for refreshing the UI's demo data.
    let args: Vec<String> = std::env::args().collect();
    // `zman-app --mcp` is how AI assistants launch zman as a local MCP server.
    if args.iter().any(|a| a == connections::MCP_FLAG) {
        if let Err(e) = zman_mcp::serve_stdio() {
            eprintln!("zman: {e}");
            std::process::exit(1);
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--dump-dashboard") {
        let period = args.get(i + 1).map_or("day", String::as_str);
        let date = args.get(i + 2).map(String::as_str);
        match dashboard_json(&paths, lang, period, date) {
            Ok(value) => println!("{value:#}"),
            Err(e) => {
                eprintln!("zman: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let state = AppState {
        paths,
        lang: Mutex::new(lang),
        control: Arc::new(Control::default()),
        tracking: Mutex::new(Tracking::Starting),
        tray: Mutex::new(None),
    };

    tauri::Builder::default()
        // A second launch just brings the existing window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_FLAG])))
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            set_autostart,
            set_paused,
            set_language,
            get_connections,
            set_claude_desktop,
            sync_login,
            sync_logout,
            sync_now,
            get_settings,
            save_settings,
            set_focus,
            set_category,
            export_csv,
            get_clients,
            save_clients
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            start_tracking(app.handle().clone());
            nudges::start(app.handle().clone());
            if !std::env::args().any(|a| a == HIDDEN_FLAG) {
                show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app tracking in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to start zman");
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let (open_label, pause_label, quit_label) = tray_labels(*app.state::<AppState>().lang.lock().unwrap());
    let open = MenuItem::with_id(app, "open", open_label, true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", pause_label, true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pause, &PredefinedMenuItem::separator(app)?, &quit])?;
    *app.state::<AppState>().tray.lock().unwrap() = Some(TrayItems { open, pause, quit });

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("zman")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_window(app),
            "pause" => {
                let state = app.state::<AppState>();
                let paused = !state.control.paused.load(Ordering::SeqCst);
                apply_paused(app, paused);
            }
            "quit" => {
                app.state::<AppState>().control.stop.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Runs the tracker on its own thread for the life of the app.
fn start_tracking(app: AppHandle) {
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let set = |tracking: Tracking| {
            *state.tracking.lock().unwrap() = tracking;
            let _ = app.emit("status-changed", ());
        };
        let config = match Config::load(&state.paths.config_file) {
            Ok(config) => config,
            Err(e) => return set(Tracking::Failed(e.to_string())),
        };
        let result = zman_daemon::run(&state.paths, config, &state.control, || set(Tracking::Running));
        match result {
            Ok(()) => {}
            Err(RunError::AlreadyRunning) => set(Tracking::External),
            Err(e) => set(Tracking::Failed(e.to_string())),
        }
    });
}

fn apply_paused(app: &AppHandle, paused: bool) {
    let state = app.state::<AppState>();
    state.control.paused.store(paused, Ordering::SeqCst);
    if let Some(items) = state.tray.lock().unwrap().as_ref() {
        let _ = items.pause.set_checked(paused);
    }
    let _ = app.emit("status-changed", ());
}

fn status_json(app: &AppHandle) -> Value {
    let state = app.state::<AppState>();
    let paused = state.control.paused.load(Ordering::SeqCst);
    let (tracking, error) = match &*state.tracking.lock().unwrap() {
        Tracking::Starting | Tracking::Running if paused => ("paused", None),
        Tracking::Starting | Tracking::Running => ("running", None),
        Tracking::External => ("external", None),
        Tracking::Failed(message) => ("stopped", Some(message.clone())),
    };
    json!({
        "tracking": tracking,
        "error": error,
        "autostart": app.autolaunch().is_enabled().unwrap_or(false),
        // "auto" while the app follows the system language.
        "language": Lang::saved().map_or("auto", Lang::code),
        "db_path": state.paths.db_file.display().to_string(),
        "config_path": state.paths.config_file.display().to_string(),
    })
}

fn dashboard_json(paths: &Paths, lang: Lang, period: &str, date: Option<&str>) -> Result<Value, String> {
    // The config is re-read on every call so edits to the category rules
    // show up without restarting.
    let config = Config::load(&paths.config_file).map_err(|e| e.to_string())?;
    let store = Store::open(&paths.db_file).map_err(|e| e.to_string())?;
    let settings = Settings::load(&store).map_err(|e| e.to_string())?;
    let categories = settings.categories(&config, lang);
    let view = dashboard::View { store: &store, categories: &categories, lang, tz: &TimeZone::system() };
    let mut data = view.build(period, date)?;
    data["goal_minutes"] = json!(settings.goal_minutes);
    data["focus_until_ms"] = json!(if settings.block.focus_active(now_ms()) { settings.block.focus_until_ms } else { 0 });
    Ok(data)
}

// ---------- Focus, blocking, categories and export ----------

fn open_store(state: &AppState) -> Result<Store, String> {
    Store::open(&state.paths.db_file).map_err(|e| e.to_string())
}

/// The settings plus what the dialog needs to draw them.
fn settings_json(state: &AppState, store: &Store) -> Result<Value, String> {
    let settings = Settings::load(store).map_err(|e| e.to_string())?;
    let config = Config::load(&state.paths.config_file).map_err(|e| e.to_string())?;
    let categories = settings.categories(&config, *state.lang.lock().unwrap());
    let names: Vec<Value> = categories.list().iter().map(|(name, score)| json!({ "name": name, "score": score })).collect();
    Ok(json!({ "settings": settings, "categories": names, "now_ms": now_ms() }))
}

#[tauri::command(async)]
fn get_settings(state: State<'_, AppState>) -> Result<Value, String> {
    settings_json(&state, &open_store(&state)?)
}

/// Saves what the Focus dialog edits. The running focus session and the
/// category overrides have their own commands and are left alone.
#[tauri::command(async)]
fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Value, String> {
    let store = open_store(&state)?;
    let current = Settings::load(&store).map_err(|e| e.to_string())?;
    let mut next = settings;
    next.block.focus_until_ms = current.block.focus_until_ms;
    next.overrides = current.overrides;
    // Clients have their own dialog and command.
    next.clients = current.clients;
    let clean = |list: &mut Vec<String>| {
        for item in list.iter_mut() {
            // People paste addresses: keep just the site.
            *item = zman_core::event::domain_of(item.trim()).unwrap_or_default();
        }
        list.retain(|item| !item.is_empty());
        list.dedup();
    };
    clean(&mut next.block.sites);
    next.block.limits.retain(|limit| !limit.target.trim().is_empty() && limit.minutes > 0);
    next.block.schedules.retain(|s| !s.days.is_empty() && s.start_min < s.end_min && s.end_min <= 24 * 60);
    next.goal_minutes = next.goal_minutes.min(24 * 60);
    next.save(&store).map_err(|e| e.to_string())?;
    settings_json(&state, &store)
}

/// Starts a focus session of `minutes`, or ends the current one with 0.
#[tauri::command(async)]
fn set_focus(app: AppHandle, state: State<'_, AppState>, minutes: u32) -> Result<Value, String> {
    let store = open_store(&state)?;
    let mut settings = Settings::load(&store).map_err(|e| e.to_string())?;
    settings.block.focus_until_ms = if minutes == 0 { 0 } else { now_ms() + minutes.min(480) as i64 * 60_000 };
    settings.save(&store).map_err(|e| e.to_string())?;
    let _ = app.emit("status-changed", ());
    settings_json(&state, &store)
}

/// Moves an app or site to a category; `category: null` returns it to the rules.
#[tauri::command(async)]
fn set_category(state: State<'_, AppState>, kind: String, name: String, category: Option<String>) -> Result<(), String> {
    if kind != "app" && kind != "site" {
        return Err("kind must be \"app\" or \"site\"".into());
    }
    let store = open_store(&state)?;
    let mut settings = Settings::load(&store).map_err(|e| e.to_string())?;
    settings.set_override(&kind, &name, category.as_deref());
    settings.save(&store).map_err(|e| e.to_string())
}

// ---------- Clients ----------

/// Hours and payments per client for the range on screen and for its month,
/// the clients themselves (for the editor), and folders or sites that look
/// like work for someone and match no client yet.
fn clients_json(store: &Store, start_ms: i64, end_ms: i64) -> Result<Value, String> {
    let text = |e: zman_core::Error| e.to_string();
    let tz = TimeZone::system();
    let clients = Settings::load(store).map_err(text)?.clients;
    let range = zman_core::Range { start_ms, end_ms };

    let day = period::date_of(start_ms, &tz);
    let first = day.first_of_month();
    let next = first.checked_add(jiff::Span::new().months(1)).map_err(|e| e.to_string())?;
    let month = zman_core::Range {
        start_ms: period::start_of_day(first, &tz).map_err(text)?,
        end_ms: period::start_of_day(next, &tz).map_err(text)?,
    };
    let month_events = store.events_between(month).map_err(text)?;
    let events = store.events_between(range).map_err(text)?;
    let on_screen = zman_core::client::report(&events, range, &clients, &tz);
    let in_month = zman_core::client::report(&month_events, month, &clients, &tz);

    // Suggestions: project folders (terminal and editor titles) and sites
    // with no client, over the last 30 days.
    let recent = zman_core::Range { start_ms: now_ms() - 30 * 86_400_000, end_ms: now_ms() };
    let mut seen: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for event in store.events_between(recent).map_err(text)? {
        if zman_core::client::client_of(&clients, &event).is_some() {
            continue;
        }
        if let Some(folder) = project_folder(&event) {
            *seen.entry(folder).or_default() += event.duration_ms;
        }
    }
    let mut suggestions: Vec<(String, i64)> = seen.into_iter().filter(|(_, ms)| *ms >= 10 * 60_000).collect();
    suggestions.sort_by(|a, b| b.1.cmp(&a.1));
    suggestions.truncate(8);

    Ok(json!({
        "clients": clients,
        "period": on_screen,
        "month": in_month,
        "month_start": first.to_string(),
        "suggestions": suggestions.iter().map(|(name, ms)| json!({ "name": name, "ms": ms })).collect::<Vec<_>>(),
    }))
}

/// The project folder in a terminal title (`user: folder`) or an editor
/// title (`file - folder - Visual Studio Code`).
fn project_folder(event: &zman_core::Event) -> Option<String> {
    let app = event.app.to_lowercase();
    let title = event.title.trim();
    let name = if app.contains("terminal") || app.contains("powershell") || app.contains("cmd") {
        let tail = title.rsplit(':').next()?.trim();
        tail.rsplit(['\\', '/']).next()?.trim().to_string()
    } else if app == "code.exe" || app.contains("cursor") {
        let parts: Vec<&str> = title.split(" - ").collect();
        if parts.len() >= 3 { parts[parts.len() - 2].trim().to_string() } else { return None }
    } else {
        return None;
    };
    let name = name.trim_start_matches('~').trim().to_string();
    let words = name.split_whitespace().count();
    (!name.is_empty() && name.len() <= 40 && words <= 3 && !name.contains("Administrator")).then_some(name)
}

#[tauri::command(async)]
fn get_clients(state: State<'_, AppState>, start_ms: i64, end_ms: i64) -> Result<Value, String> {
    clients_json(&open_store(&state)?, start_ms, end_ms)
}

#[tauri::command(async)]
fn save_clients(
    state: State<'_, AppState>,
    clients: Vec<zman_core::Client>,
    start_ms: i64,
    end_ms: i64,
) -> Result<Value, String> {
    let store = open_store(&state)?;
    let mut settings = Settings::load(&store).map_err(|e| e.to_string())?;
    let clean = |list: Vec<String>| -> Vec<String> {
        let mut out: Vec<String> = list.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        out.dedup();
        out
    };
    settings.clients = clients
        .into_iter()
        .filter(|c| !c.name.trim().is_empty())
        .map(|mut c| {
            c.name = c.name.trim().to_string();
            c.keywords = clean(c.keywords);
            c.domains = clean(c.domains).into_iter().filter_map(|d| zman_core::event::domain_of(&d)).collect();
            c.apps = clean(c.apps);
            c.payments.retain(|p| p.amount > 0.0 && period::parse_date(&p.date).is_ok());
            for p in &mut c.payments {
                p.currency = if p.currency.trim().is_empty() { "ARS".into() } else { p.currency.trim().to_uppercase() };
            }
            c
        })
        .collect();
    settings.save(&store).map_err(|e| e.to_string())?;
    clients_json(&store, start_ms, end_ms)
}

/// Writes the events of the day or week on screen to a CSV file in the
/// Downloads folder and returns its path.
#[tauri::command(async)]
fn export_csv(state: State<'_, AppState>, period: String, date: Option<String>) -> Result<String, String> {
    let text = |e: zman_core::Error| e.to_string();
    let tz = TimeZone::system();
    let day = match date.as_deref() {
        Some(date) => period::parse_date(date).map_err(text)?,
        None => period::today(&tz),
    };
    let (first, range) = if period == "week" {
        let monday = period::week_start(day).map_err(text)?;
        (monday, period::days_range(monday, 7, &tz).map_err(text)?)
    } else {
        (day, period::day_range(day, &tz).map_err(text)?)
    };

    let store = open_store(&state)?;
    let config = Config::load(&state.paths.config_file).map_err(text)?;
    let categories = Settings::load(&store).map_err(text)?.categories(&config, *state.lang.lock().unwrap());
    let events = store.events_between(range).map_err(text)?;

    let folder = directories::UserDirs::new()
        .and_then(|dirs| dirs.download_dir().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| state.paths.data_dir.clone());
    let name = if period == "week" { format!("zman-week-{first}.csv") } else { format!("zman-{first}.csv") };
    let path = folder.join(name);
    // The byte order mark makes Excel read accents correctly.
    let content = format!("\u{feff}{}",zman_core::export::csv(&events, &categories, &tz));
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
}

#[tauri::command]
fn get_dashboard(app: AppHandle, state: State<'_, AppState>, period: String, date: Option<String>) -> Result<Value, String> {
    let lang = *state.lang.lock().unwrap();
    let mut data = dashboard_json(&state.paths, lang, &period, date.as_deref())?;
    data["status"] = status_json(&app);
    Ok(data)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<Value, String> {
    let autolaunch = app.autolaunch();
    if enabled { autolaunch.enable() } else { autolaunch.disable() }.map_err(|e| e.to_string())?;
    Ok(status_json(&app))
}

/// Switches the language of the whole app: "en", "es", or "auto" to follow
/// the system. The choice is saved and the CLI follows it too.
#[tauri::command]
fn set_language(app: AppHandle, state: State<'_, AppState>, language: String) -> Result<Value, String> {
    let choice = match language.as_str() {
        "en" => Some(Lang::En),
        "es" => Some(Lang::Es),
        _ => None,
    };
    Lang::save(choice).map_err(|e| e.to_string())?;
    let lang = Lang::detect();
    *state.lang.lock().unwrap() = lang;
    if let Some(items) = state.tray.lock().unwrap().as_ref() {
        let (open, pause, quit) = tray_labels(lang);
        let _ = items.open.set_text(open);
        let _ = items.pause.set_text(pause);
        let _ = items.quit.set_text(quit);
    }
    Ok(status_json(&app))
}

#[tauri::command]
fn set_paused(app: AppHandle, paused: bool) -> Value {
    apply_paused(&app, paused);
    status_json(&app)
}

// ---------- Connections: AI assistants and sync ----------

/// Full path of this executable, which is also the MCP server (`--mcp`).
fn own_exe() -> Result<String, String> {
    std::env::current_exe().map(|path| path.display().to_string()).map_err(|e| e.to_string())
}

fn connections_json(state: &AppState) -> Result<Value, String> {
    let exe = own_exe()?;
    let claude = connections::claude_config_path();
    let store = Store::open(&state.paths.db_file).map_err(|e| e.to_string())?;
    let status = sync::status(&store)?;
    Ok(json!({
        "claude_desktop": {
            "installed": claude.is_some(),
            "connected": claude.as_deref().is_some_and(connections::is_connected),
        },
        "claude_code_command": format!("claude mcp add zman -- \"{exe}\" {}", connections::MCP_FLAG),
        "sync": status.account.map(|account| json!({
            "server": account.server,
            "email": account.email,
            "mcp_url": format!("{}/mcp", account.server),
            "last_sync_ms": status.last_sync_ms,
        })),
    }))
}

#[tauri::command(async)]
fn get_connections(state: State<'_, AppState>) -> Result<Value, String> {
    connections_json(&state)
}

#[tauri::command(async)]
fn set_claude_desktop(state: State<'_, AppState>, connected: bool) -> Result<Value, String> {
    let path = connections::claude_config_path().ok_or("Claude Desktop is not installed on this computer")?;
    if connected {
        connections::connect(&path, &own_exe()?)?;
    } else {
        connections::disconnect(&path)?;
    }
    connections_json(&state)
}

/// Signs this device in to a sync server (creating the account when asked)
/// and uploads what is already tracked. The password is not stored.
#[tauri::command(async)]
fn sync_login(state: State<'_, AppState>, server: String, email: String, password: String, register: bool) -> Result<Value, String> {
    let store = Store::open(&state.paths.db_file).map_err(|e| e.to_string())?;
    sync::login(&store, &server, &email, &password, register)?;
    let config = Config::load(&state.paths.config_file).map_err(|e| e.to_string())?;
    sync::run(&store, &config)?;
    connections_json(&state)
}

#[tauri::command(async)]
fn sync_logout(state: State<'_, AppState>) -> Result<Value, String> {
    let store = Store::open(&state.paths.db_file).map_err(|e| e.to_string())?;
    sync::logout(&store)?;
    connections_json(&state)
}

#[tauri::command(async)]
fn sync_now(state: State<'_, AppState>) -> Result<Value, String> {
    let store = Store::open(&state.paths.db_file).map_err(|e| e.to_string())?;
    let config = Config::load(&state.paths.config_file).map_err(|e| e.to_string())?;
    sync::run(&store, &config)?;
    connections_json(&state)
}
