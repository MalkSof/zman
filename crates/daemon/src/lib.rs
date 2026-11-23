//! The tracking loop, shared by the `zman-daemon` binary and the desktop app.

pub mod browser;
pub mod platform;
pub mod sync;

use std::fmt;
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use zman_core::{Config, Paths, Store, Tracker, now_ms};

#[derive(Debug)]
pub enum RunError {
    /// Another process (daemon or desktop app) is already tracking.
    AlreadyRunning,
    Other(String),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::AlreadyRunning => write!(f, "another zman tracker is already running"),
            RunError::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RunError {}

fn other(e: impl fmt::Display) -> RunError {
    RunError::Other(e.to_string())
}

/// Flags another thread uses to control a running tracker.
#[derive(Debug, Default)]
pub struct Control {
    /// Set to end [`run`].
    pub stop: AtomicBool,
    /// While set, nothing is recorded.
    pub paused: AtomicBool,
}

/// Tracks the active window until `control.stop` is set.
///
/// Fails fast with [`RunError::AlreadyRunning`] if another tracker holds the
/// lock. `on_ready` is called once tracking has actually started.
pub fn run(paths: &Paths, config: Config, control: &Control, on_ready: impl FnOnce()) -> Result<(), RunError> {
    std::fs::create_dir_all(&paths.data_dir).map_err(other)?;

    // Held for as long as this function runs; the OS releases it if we crash.
    let lock = File::create(&paths.lock_file).map_err(other)?;
    if lock.try_lock().is_err() {
        return Err(RunError::AlreadyRunning);
    }

    let platform = platform::current().map_err(other)?;
    let poll_interval = Duration::from_secs(config.poll_interval_secs);
    // The browser extension is optional: if the port is taken, tracking
    // simply goes on without URLs.
    let active_tab = match config.browser.port {
        0 => None,
        port => match browser::listen(port, config.browser.full_urls, browser::checker(paths)) {
            Ok(active) => Some(active),
            Err(e) => {
                eprintln!("zman: browser extension support is off (port {port}: {e})");
                None
            }
        },
    };
    let store = Store::open(&paths.db_file).map_err(other)?;
    let sync_config = config.clone();
    let mut tracker = Tracker::new(store, config).map_err(other)?;
    on_ready();

    // Signed-in devices upload on their own, on a separate connection so a
    // slow network never delays tracking. Scoped: it ends with this function.
    std::thread::scope(|scope| {
        scope.spawn(|| sync_periodically(paths, &sync_config, control));
        track(&mut tracker, platform.as_ref(), active_tab.as_ref(), poll_interval, control);
    });
    Ok(())
}

/// Syncs shortly after start and then every few minutes, until stopped.
fn sync_periodically(paths: &Paths, config: &Config, control: &Control) {
    const EVERY: Duration = Duration::from_secs(5 * 60);
    let mut next = Instant::now() + Duration::from_secs(20);
    while !control.stop.load(Ordering::SeqCst) {
        if Instant::now() >= next {
            next = Instant::now() + EVERY;
            let result = Store::open(&paths.db_file).map_err(|e| e.to_string()).and_then(|store| sync::run(&store, config));
            if let Err(e) = result {
                eprintln!("zman: sync failed, will retry: {e}");
            }
        }
        sleep(Duration::from_millis(250));
    }
}

fn track(
    tracker: &mut Tracker,
    platform: &dyn platform::Platform,
    active_tab: Option<&browser::ActiveTab>,
    poll_interval: Duration,
    control: &Control,
) {

    while !control.stop.load(Ordering::SeqCst) {
        if !control.paused.load(Ordering::SeqCst) {
            let mut window = platform.active_window();
            if let (Some(window), Some(active_tab)) = (window.as_mut(), active_tab) {
                active_tab.annotate(window);
            }
            let idle_ms = platform.idle_time().as_millis() as i64;
            if let Err(e) = tracker.tick(now_ms(), window, idle_ms) {
                // A failed write loses one sample; keep tracking.
                eprintln!("zman: {e}");
            }
        }

        // Sleep in short steps so a stop request is handled promptly.
        let wake = Instant::now() + poll_interval;
        while !control.stop.load(Ordering::SeqCst) && Instant::now() < wake {
            sleep(Duration::from_millis(100));
        }
    }
}
