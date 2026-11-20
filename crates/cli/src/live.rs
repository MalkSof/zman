//! `zman watch` and `zman status`.

use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use zman_core::{Config, Event, Lang, Paths, Store, now_ms};

pub fn watch(lang: Lang) -> Result<(), Box<dyn Error>> {
    let paths = Paths::resolve()?;
    let config = Config::load(&paths.config_file)?;
    let store = Store::open(&paths.db_file)?;
    let tz = TimeZone::system();
    // An event not extended for this long is no longer being tracked.
    let stale_ms = config.max_gap_ms();

    if !daemon_running(&paths) {
        println!(
            "{}",
            lang.pick(
                "zman-daemon is not running: start it to see new activity.",
                "zman-daemon no está corriendo: inicialo para ver actividad nueva.",
            )
        );
    }
    println!(
        "{}",
        lang.pick(
            "STARTED   DURATION  APP | TITLE   (Ctrl+C to exit)",
            "INICIO    DURACIÓN  APP | TÍTULO   (Ctrl+C para salir)",
        )
    );

    // Some context first, then follow the latest event in place.
    let mut history = store.recent_events(6)?;
    let mut current: Option<String> = history.pop().map(|e| e.id);
    for event in &history {
        println!("{}", format_event(event, &tz, lang, false));
    }

    let mut last_len: usize = 0;
    loop {
        if let Some(event) = store.last_event()? {
            if current.as_deref() != Some(event.id.as_str()) {
                // A newer event took over: leave the previous line as is.
                if last_len > 0 {
                    println!();
                }
                current = Some(event.id.clone());
            }
            let paused = now_ms() - event.end_ms() > stale_ms;
            let line = format_event(&event, &tz, lang, paused);
            let len = line.chars().count();
            // Pad with spaces to erase leftovers of a longer previous line.
            print!("\r{line}{}", " ".repeat(last_len.saturating_sub(len)));
            std::io::stdout().flush()?;
            last_len = len;
        }
        sleep(Duration::from_secs(1));
    }
}

pub fn status(lang: Lang) -> Result<(), Box<dyn Error>> {
    let paths = Paths::resolve()?;
    let config_note = if paths.config_file.exists() {
        ""
    } else {
        lang.pick(" (not created, using defaults)", " (no existe, se usan los valores por defecto)")
    };
    let running = if daemon_running(&paths) {
        lang.pick("running", "corriendo")
    } else {
        lang.pick("not running", "detenido")
    };
    // Labels are padded to the same width in both languages.
    println!("Config:      {}{config_note}", paths.config_file.display());
    println!("{} {}", lang.pick("Database:   ", "Base:       "), paths.db_file.display());
    println!("Daemon:      {running}");

    let events_label = lang.pick("Events:     ", "Eventos:    ");
    if !paths.db_file.exists() {
        let none = lang.pick(
            "none yet (the daemon has never run)",
            "ninguno todavía (el daemon nunca corrió)",
        );
        println!("{events_label} {none}");
        return Ok(());
    }
    let store = Store::open(&paths.db_file)?;
    println!("{} {}", lang.pick("Device id:  ", "Dispositivo:"), store.device_id()?);
    println!("{events_label} {}", store.count_events()?);
    if let Some(event) = store.last_event()? {
        let line = format_event(&event, &TimeZone::system(), lang, false);
        println!("{} {line}", lang.pick("Last:       ", "Último:     "));
    }
    Ok(())
}

/// The daemon holds an exclusive lock on its lock file while it runs.
fn daemon_running(paths: &Paths) -> bool {
    match File::open(&paths.lock_file) {
        Ok(file) => file.try_lock().is_err(),
        Err(_) => false,
    }
}

fn format_event(event: &Event, tz: &TimeZone, lang: Lang, paused: bool) -> String {
    let started = Timestamp::from_millisecond(event.start_ms)
        .map(|ts| ts.to_zoned_datetime(tz.clone()).strftime("%H:%M:%S").to_string())
        .unwrap_or_else(|_| "??:??:??".to_string());
    let secs = event.duration_ms / 1000;
    let duration = format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60);
    let line = format!("{started}  {duration}  {} | {}", event.app, truncate(&event.title, 60));
    if paused { format!("{line}  {}", lang.pick("[paused]", "[en pausa]")) } else { line }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut short: String = text.chars().take(max_chars - 1).collect();
    short.push('…');
    short
}
