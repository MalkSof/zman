//! zman-daemon: samples the active window and idle time, and records events.

mod platform;

use std::error::Error;
use std::fs::File;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use zman_core::{Config, Lang, Paths, Store, Tracker, now_ms};

fn main() {
    if let Err(e) = run(Lang::detect()) {
        eprintln!("zman-daemon: {e}");
        std::process::exit(1);
    }
}

fn run(lang: Lang) -> Result<(), Box<dyn Error>> {
    let paths = Paths::resolve()?;
    std::fs::create_dir_all(&paths.data_dir)?;

    // Held for the life of the process; the OS releases it when we exit.
    let lock = File::create(&paths.lock_file)?;
    if lock.try_lock().is_err() {
        return Err(lang
            .pick("another zman-daemon is already running", "ya hay otro zman-daemon corriendo")
            .into());
    }

    let config = Config::load(&paths.config_file)?;
    let platform = platform::current()?;
    let poll_interval = Duration::from_secs(config.poll_interval_secs);
    let mut tracker = Tracker::new(Store::open(&paths.db_file)?, config)?;

    let running = Arc::new(AtomicBool::new(true));
    let flag = running.clone();
    ctrlc::set_handler(move || flag.store(false, Ordering::SeqCst))?;

    println!(
        "{} {}",
        lang.pick("zman-daemon tracking. Database:", "zman-daemon registrando. Base de datos:"),
        paths.db_file.display()
    );
    println!("{}", lang.pick("Press Ctrl+C to stop.", "Ctrl+C para detenerlo."));

    while running.load(Ordering::SeqCst) {
        let window = platform.active_window();
        let idle_ms = platform.idle_time().as_millis() as i64;
        if let Err(e) = tracker.tick(now_ms(), window, idle_ms) {
            // A failed write loses one sample; keep tracking.
            eprintln!("zman-daemon: {e}");
        }

        // Sleep in short steps so Ctrl+C is handled promptly.
        let wake = Instant::now() + poll_interval;
        while running.load(Ordering::SeqCst) && Instant::now() < wake {
            sleep(Duration::from_millis(100));
        }
    }

    println!("{}", lang.pick("zman-daemon stopped.", "zman-daemon detenido."));
    Ok(())
}
