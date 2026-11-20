use ulid::Ulid;

use crate::config::Config;
use crate::error::Result;
use crate::event::Event;
use crate::store::Store;

/// The active window as reported by the platform layer.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub app: String,
    pub title: String,
}

/// What a call to [`Tracker::tick`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tick {
    /// A new event was created.
    Started,
    /// The window did not change; the current event was extended.
    Extended,
    /// No input for longer than the AFK threshold; nothing recorded.
    Afk,
    /// There is no active window (e.g. locked screen); nothing recorded.
    NoWindow,
    /// The window matches an exclusion rule; nothing recorded.
    Excluded,
}

/// Turns periodic samples (active window + idle time) into stored events.
///
/// Holds no platform code: the daemon feeds it samples, tests feed it fakes.
pub struct Tracker {
    store: Store,
    config: Config,
    device_id: String,
    /// The event being extended. Lives only in memory, so a restarted daemon
    /// always begins a fresh event.
    current: Option<Event>,
}

impl Tracker {
    pub fn new(store: Store, config: Config) -> Result<Tracker> {
        let device_id = store.device_id()?;
        Ok(Tracker { store, config, device_id, current: None })
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Processes one sample taken at `now_ms`. `idle_ms` is how long ago the
    /// last keyboard/mouse input happened.
    pub fn tick(&mut self, now_ms: i64, window: Option<WindowInfo>, idle_ms: i64) -> Result<Tick> {
        if idle_ms >= self.config.afk_threshold_ms() {
            // The user actually left at the last input, not now: give back
            // the time recorded since then.
            if let Some(current) = self.current.take() {
                let last_input_ms = now_ms - idle_ms;
                let trimmed = (last_input_ms - current.start_ms).clamp(0, current.duration_ms);
                if trimmed != current.duration_ms {
                    self.store.set_duration(&current.id, trimmed, now_ms)?;
                }
            }
            return Ok(Tick::Afk);
        }

        let Some(window) = window else {
            self.close_current(now_ms)?;
            return Ok(Tick::NoWindow);
        };
        if self.config.is_excluded(&window.app, &window.title) {
            self.close_current(now_ms)?;
            return Ok(Tick::Excluded);
        }

        let same_window = self
            .current
            .as_ref()
            .is_some_and(|c| c.app == window.app && c.title == window.title);
        if same_window && self.extend_current(now_ms)? {
            return Ok(Tick::Extended);
        }

        self.close_current(now_ms)?;
        let event = Event {
            id: Ulid::generate().to_string(),
            device_id: self.device_id.clone(),
            start_ms: now_ms,
            duration_ms: 0,
            app: window.app,
            title: window.title,
            data: serde_json::json!({}),
            updated_at_ms: now_ms,
        };
        self.store.insert_event(&event)?;
        self.current = Some(event);
        Ok(Tick::Started)
    }

    /// Stretches the current event up to `now_ms`. Returns false (and leaves
    /// it untouched) if too much time passed since it was last extended, or
    /// the clock went backwards.
    fn extend_current(&mut self, now_ms: i64) -> Result<bool> {
        let Some(current) = self.current.as_mut() else {
            return Ok(false);
        };
        let gap = now_ms - current.end_ms();
        if gap < 0 || gap > self.config.max_gap_ms() {
            return Ok(false);
        }
        current.duration_ms = now_ms - current.start_ms;
        current.updated_at_ms = now_ms;
        self.store.set_duration(&current.id, current.duration_ms, now_ms)?;
        Ok(true)
    }

    /// Ends the current event at `now_ms`, since the window was active right
    /// up to this sample.
    fn close_current(&mut self, now_ms: i64) -> Result<()> {
        self.extend_current(now_ms)?;
        self.current = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_700_000_000_000;

    fn tracker() -> Tracker {
        tracker_with(Config::default())
    }

    fn tracker_with(config: Config) -> Tracker {
        Tracker::new(Store::open_in_memory().unwrap(), config).unwrap()
    }

    fn win(app: &str, title: &str) -> Option<WindowInfo> {
        Some(WindowInfo { app: app.into(), title: title.into() })
    }

    fn events(t: &Tracker) -> Vec<Event> {
        t.store().recent_events(100).unwrap()
    }

    #[test]
    fn same_window_extends_one_event() {
        let mut t = tracker();
        assert_eq!(t.tick(T0, win("code.exe", "main.rs"), 0).unwrap(), Tick::Started);
        assert_eq!(t.tick(T0 + 1000, win("code.exe", "main.rs"), 0).unwrap(), Tick::Extended);
        assert_eq!(t.tick(T0 + 2000, win("code.exe", "main.rs"), 0).unwrap(), Tick::Extended);

        let events = events(&t);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].start_ms, T0);
        assert_eq!(events[0].duration_ms, 2000);
        assert_eq!(events[0].updated_at_ms, T0 + 2000);
        assert_eq!(events[0].device_id, t.store().device_id().unwrap());
    }

    #[test]
    fn window_change_closes_previous_and_starts_new() {
        let mut t = tracker();
        t.tick(T0, win("code.exe", "main.rs"), 0).unwrap();
        t.tick(T0 + 1000, win("code.exe", "main.rs"), 0).unwrap();
        assert_eq!(t.tick(T0 + 2000, win("chrome.exe", "Docs"), 0).unwrap(), Tick::Started);

        let events = events(&t);
        assert_eq!(events.len(), 2);
        // The first event runs right up to the switch: no time is lost.
        assert_eq!(events[0].duration_ms, 2000);
        assert_eq!(events[1].app, "chrome.exe");
        assert_eq!(events[1].start_ms, T0 + 2000);
        assert_eq!(events[1].duration_ms, 0);
    }

    #[test]
    fn title_change_in_same_app_starts_new_event() {
        let mut t = tracker();
        t.tick(T0, win("chrome.exe", "Tab A"), 0).unwrap();
        assert_eq!(t.tick(T0 + 1000, win("chrome.exe", "Tab B"), 0).unwrap(), Tick::Started);
        assert_eq!(events(&t).len(), 2);
    }

    #[test]
    fn long_gap_is_not_filled_in() {
        let mut t = tracker();
        t.tick(T0, win("code.exe", "main.rs"), 0).unwrap();
        t.tick(T0 + 1000, win("code.exe", "main.rs"), 0).unwrap();
        // The machine was suspended for an hour; same window on wake-up.
        let later = T0 + 3_600_000;
        assert_eq!(t.tick(later, win("code.exe", "main.rs"), 0).unwrap(), Tick::Started);

        let events = events(&t);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].duration_ms, 1000);
        assert_eq!(events[1].start_ms, later);
    }

    #[test]
    fn clock_going_backwards_starts_new_event() {
        let mut t = tracker();
        t.tick(T0, win("code.exe", "main.rs"), 0).unwrap();
        t.tick(T0 + 1000, win("code.exe", "main.rs"), 0).unwrap();
        assert_eq!(t.tick(T0 - 5000, win("code.exe", "main.rs"), 0).unwrap(), Tick::Started);
        assert_eq!(events(&t)[1].duration_ms, 1000);
    }

    #[test]
    fn afk_trims_event_back_to_last_input() {
        let mut t = tracker(); // AFK threshold: 180 s
        // Last input at T0 + 10 s, then the user walks away.
        for s in 0..180 {
            let idle = ((s - 10) * 1000).max(0);
            t.tick(T0 + s * 1000, win("code.exe", "main.rs"), idle).unwrap();
        }
        assert_eq!(events(&t)[0].duration_ms, 179_000);

        // 180 s after the last input the threshold is reached.
        let now = T0 + 190_000;
        assert_eq!(t.tick(now, win("code.exe", "main.rs"), 180_000).unwrap(), Tick::Afk);
        assert_eq!(t.tick(now + 1000, win("code.exe", "main.rs"), 181_000).unwrap(), Tick::Afk);

        let events = events(&t);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].duration_ms, 10_000);
        assert_eq!(events[0].updated_at_ms, now);
    }

    #[test]
    fn coming_back_from_afk_starts_new_event() {
        let mut t = tracker();
        t.tick(T0, win("code.exe", "main.rs"), 0).unwrap();
        t.tick(T0 + 200_000, win("code.exe", "main.rs"), 200_000).unwrap();
        let back = T0 + 600_000;
        assert_eq!(t.tick(back, win("code.exe", "main.rs"), 0).unwrap(), Tick::Started);

        let events = events(&t);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].duration_ms, 0);
        assert_eq!(events[1].start_ms, back);
    }

    #[test]
    fn afk_trim_never_goes_negative_or_grows() {
        let mut t = tracker();
        // Event starts after the last input (user idle but under threshold).
        t.tick(T0, win("code.exe", "main.rs"), 100_000).unwrap();
        t.tick(T0 + 1000, win("code.exe", "main.rs"), 101_000).unwrap();
        t.tick(T0 + 80_000, win("code.exe", "main.rs"), 180_000).unwrap();
        assert_eq!(events(&t)[0].duration_ms, 0);
    }

    #[test]
    fn excluded_windows_are_never_stored() {
        let config = Config::parse("[exclude]\napps = [\"keepass*\"]\ntitles = [\"*banco*\"]").unwrap();
        let mut t = tracker_with(config);
        t.tick(T0, win("code.exe", "main.rs"), 0).unwrap();
        assert_eq!(t.tick(T0 + 1000, win("KeePassXC.exe", "Vault"), 0).unwrap(), Tick::Excluded);
        assert_eq!(t.tick(T0 + 2000, win("chrome.exe", "Mi Banco"), 0).unwrap(), Tick::Excluded);
        assert_eq!(t.tick(T0 + 3000, win("code.exe", "main.rs"), 0).unwrap(), Tick::Started);

        let events = events(&t);
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.app == "code.exe"));
        // The first event ended when the excluded window took focus.
        assert_eq!(events[0].duration_ms, 1000);
        assert_eq!(events[1].start_ms, T0 + 3000);
    }

    #[test]
    fn no_window_records_nothing() {
        let mut t = tracker();
        assert_eq!(t.tick(T0, None, 0).unwrap(), Tick::NoWindow);
        assert!(events(&t).is_empty());
    }

    #[test]
    fn event_ids_are_unique() {
        let mut t = tracker();
        for i in 0..50 {
            t.tick(T0 + i * 1000, win("app.exe", &format!("Title {i}")), 0).unwrap();
        }
        let mut ids: Vec<String> = events(&t).into_iter().map(|e| e.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 50);
    }
}
