//! zman desktop app: tracks in the background, lives in the system tray and
//! shows the dashboard.

// No console window in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod connections;
mod dashboard;

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use jiff::tz::TimeZone;
use serde_json::{Value, json};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use zman_core::{Config, Lang, Paths, Store};
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

struct AppState {
    paths: Paths,
    lang: Lang,
    control: Arc<Control>,
    tracking: Mutex<Tracking>,
    pause_item: Mutex<Option<CheckMenuItem<Wry>>>,
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
        lang,
        control: Arc::new(Control::default()),
        tracking: Mutex::new(Tracking::Starting),
        pause_item: Mutex::new(None),
    };

    tauri::Builder::default()
        // A second launch just brings the existing window forward.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_FLAG])))
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            set_autostart,
            set_paused,
            get_connections,
            set_claude_desktop,
            sync_login,
            sync_logout,
            sync_now
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            start_tracking(app.handle().clone());
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
    let lang = app.state::<AppState>().lang;
    let open = MenuItem::with_id(app, "open", lang.pick("Open zman", "Abrir zman"), true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(
        app,
        "pause",
        lang.pick("Pause tracking", "Pausar el registro"),
        true,
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", lang.pick("Quit", "Salir"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pause, &PredefinedMenuItem::separator(app)?, &quit])?;
    *app.state::<AppState>().pause_item.lock().unwrap() = Some(pause);

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
    if let Some(item) = state.pause_item.lock().unwrap().as_ref() {
        let _ = item.set_checked(paused);
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
        "db_path": state.paths.db_file.display().to_string(),
        "config_path": state.paths.config_file.display().to_string(),
    })
}

fn dashboard_json(paths: &Paths, lang: Lang, period: &str, date: Option<&str>) -> Result<Value, String> {
    // The config is re-read on every call so edits to the category rules
    // show up without restarting.
    let config = Config::load(&paths.config_file).map_err(|e| e.to_string())?;
    let store = Store::open(&paths.db_file).map_err(|e| e.to_string())?;
    let categories = config.categories(lang);
    let view = dashboard::View { store: &store, categories: &categories, lang, tz: &TimeZone::system() };
    view.build(period, date)
}

#[tauri::command]
fn get_dashboard(app: AppHandle, state: State<'_, AppState>, period: String, date: Option<String>) -> Result<Value, String> {
    let mut data = dashboard_json(&state.paths, state.lang, &period, date.as_deref())?;
    data["status"] = status_json(&app);
    Ok(data)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<Value, String> {
    let autolaunch = app.autolaunch();
    if enabled { autolaunch.enable() } else { autolaunch.disable() }.map_err(|e| e.to_string())?;
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
