//! Everything OS specific lives behind [`Platform`]. To support a new OS,
//! add a module implementing the trait and wire it into [`current`].

use std::time::Duration;

use zman_core::WindowInfo;

#[cfg(windows)]
mod windows;

pub trait Platform {
    /// The window that currently has focus, or `None` if there is none.
    fn active_window(&self) -> Option<WindowInfo>;

    /// Time since the last keyboard or mouse input. Implementations must only
    /// ask the OS *when* input last happened, never observe the input itself.
    fn idle_time(&self) -> Duration;
}

#[cfg(windows)]
pub fn current() -> Result<Box<dyn Platform>, String> {
    Ok(Box::new(windows::WindowsPlatform))
}

#[cfg(not(windows))]
pub fn current() -> Result<Box<dyn Platform>, String> {
    Err(format!("zman does not support {} yet", std::env::consts::OS))
}
