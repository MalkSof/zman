//! zman-daemon: samples the active window and idle time, and records events.

use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use zman_core::{Config, Lang, Paths};
use zman_daemon::{Control, RunError};

fn main() {
    if let Err(e) = run(Lang::detect()) {
        eprintln!("zman-daemon: {e}");
        std::process::exit(1);
    }
}

fn run(lang: Lang) -> Result<(), Box<dyn Error>> {
    let paths = Paths::resolve()?;
    let config = Config::load(&paths.config_file)?;

    let control = Arc::new(Control::default());
    let on_ctrl_c = control.clone();
    ctrlc::set_handler(move || on_ctrl_c.stop.store(true, Ordering::SeqCst))?;

    let result = zman_daemon::run(&paths, config, &control, || {
        println!(
            "{} {}",
            lang.pick("zman-daemon tracking. Database:", "zman-daemon registrando. Base de datos:"),
            paths.db_file.display()
        );
        println!("{}", lang.pick("Press Ctrl+C to stop.", "Ctrl+C para detenerlo."));
    });
    match result {
        Ok(()) => {
            println!("{}", lang.pick("zman-daemon stopped.", "zman-daemon detenido."));
            Ok(())
        }
        Err(RunError::AlreadyRunning) => Err(lang
            .pick("another zman tracker is already running", "ya hay otro tracker de zman corriendo")
            .into()),
        Err(e) => Err(e.into()),
    }
}
