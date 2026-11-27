//! Desktop notifications: a focus session that ended, the daily goal
//! reached, a long stretch of distraction, and the weekly recap.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use jiff::tz::TimeZone;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;
use zman_core::period::{self, format_duration};
use zman_core::report::{Summary, summarize};
use zman_core::{Categories, Config, Lang, Range, Settings, Store, now_ms};

use crate::AppState;

const CHECK_EVERY: Duration = Duration::from_secs(60);
const NUDGE_WINDOW_MS: i64 = 30 * 60_000;
const NUDGE_AFTER_MS: i64 = 25 * 60_000;
const NUDGE_PAUSE: Duration = Duration::from_secs(60 * 60);

const KEY_GOAL_DAY: &str = "notified_goal_day";
const KEY_RECAP_WEEK: &str = "notified_recap_week";

/// A notification to show: title and body.
type Note = (String, String);

/// Time spent in categories with a positive (or negative) score.
fn scored_ms(summary: &Summary, productive: bool) -> i64 {
    summary.categories.iter().filter(|c| if productive { c.score > 0 } else { c.score < 0 }).map(|c| c.ms).sum()
}

fn goal_note(today: &Summary, goal_minutes: u32, lang: Lang) -> Option<Note> {
    let done = scored_ms(today, true);
    (goal_minutes > 0 && done >= goal_minutes as i64 * 60_000).then(|| {
        (
            lang.pick("Daily goal reached", "Meta del día cumplida").to_string(),
            match lang {
                Lang::En => format!("{} of productive time today. Well done.", format_duration(done)),
                Lang::Es => format!("{} de tiempo productivo hoy. Bien ahí.", format_duration(done)),
            },
        )
    })
}

fn nudge_note(recent: &Summary, lang: Lang) -> Option<Note> {
    let wasted = scored_ms(recent, false);
    if wasted < NUDGE_AFTER_MS {
        return None;
    }
    let top = recent.categories.iter().filter(|c| c.score < 0).max_by_key(|c| c.ms)?;
    let minutes = wasted / 60_000;
    Some((
        lang.pick("Time to get back?", "¿Volvemos?").to_string(),
        match lang {
            Lang::En => format!("{minutes} of the last 30 minutes went to {}. Start a focus session in Tu Hora Vale.", top.name),
            Lang::Es => format!("{minutes} de los últimos 30 minutos se fueron en {}. Iniciá una sesión de foco en Tu Hora Vale.", top.name),
        },
    ))
}

fn recap_note(last: &Summary, before: &Summary, lang: Lang) -> Option<Note> {
    let score = last.productivity?;
    let total = format_duration(last.total_ms);
    let change = before.productivity.map(|b| score as i32 - b as i32).filter(|d| *d != 0);
    let body = match (lang, change) {
        (Lang::En, Some(d)) => format!("{total} tracked, productivity {score}/100 ({d:+} vs the week before)."),
        (Lang::En, None) => format!("{total} tracked, productivity {score}/100."),
        (Lang::Es, Some(d)) => format!("{total} registradas, productividad {score}/100 ({d:+} vs. la semana anterior)."),
        (Lang::Es, None) => format!("{total} registradas, productividad {score}/100."),
    };
    Some((lang.pick("Your week in Tu Hora Vale", "Tu semana en Tu Hora Vale").to_string(), body))
}

struct Checker {
    /// End of the focus session last seen running.
    focus_until_ms: i64,
    last_nudge: Option<Instant>,
}

impl Checker {
    fn check(&mut self, app: &AppHandle) -> zman_core::Result<Vec<Note>> {
        let state = app.state::<AppState>();
        let lang = *state.lang.lock().unwrap();
        let store = Store::open(&state.paths.db_file)?;
        let settings = Settings::load(&store)?;
        let config = Config::load(&state.paths.config_file).unwrap_or_default();
        let categories = settings.categories(&config, lang);
        let tz = TimeZone::system();
        let now = now_ms();
        let mut notes = Vec::new();

        // A session that ran its course (not one the user ended early).
        if self.focus_until_ms > 0 && now >= self.focus_until_ms && settings.block.focus_until_ms == self.focus_until_ms {
            notes.push((
                lang.pick("Focus session finished", "Terminó la sesión de foco").to_string(),
                lang.pick("Nice work. Take a break.", "Buen trabajo. Tomate un descanso.").to_string(),
            ));
        }
        self.focus_until_ms = if settings.block.focus_active(now) { settings.block.focus_until_ms } else { 0 };

        let summary = |range: Range, categories: &Categories| -> zman_core::Result<Summary> {
            Ok(summarize(&store.events_between(range)?, range, categories))
        };
        let today = period::today(&tz);

        if store.meta_get(KEY_GOAL_DAY)?.as_deref() != Some(&today.to_string()) {
            let day = summary(period::day_range(today, &tz)?, &categories)?;
            if let Some(note) = goal_note(&day, settings.goal_minutes, lang) {
                store.meta_set(KEY_GOAL_DAY, &today.to_string())?;
                notes.push(note);
            }
        }

        let quiet = self.last_nudge.is_some_and(|at| at.elapsed() < NUDGE_PAUSE);
        if !quiet && settings.block.strict_until_ms(now, &tz).is_none() {
            let recent = summary(Range { start_ms: now - NUDGE_WINDOW_MS, end_ms: now }, &categories)?;
            if let Some(note) = nudge_note(&recent, lang) {
                self.last_nudge = Some(Instant::now());
                notes.push(note);
            }
        }

        let monday = period::week_start(today)?;
        if store.meta_get(KEY_RECAP_WEEK)?.as_deref() != Some(&monday.to_string()) {
            store.meta_set(KEY_RECAP_WEEK, &monday.to_string())?;
            let last = summary(period::days_range(period::add_days(monday, -7)?, 7, &tz)?, &categories)?;
            let before = summary(period::days_range(period::add_days(monday, -14)?, 7, &tz)?, &categories)?;
            notes.extend(recap_note(&last, &before, lang));
        }

        Ok(if settings.notifications { notes } else { Vec::new() })
    }
}

/// Checks once a minute, for the life of the app.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut checker = Checker { focus_until_ms: 0, last_nudge: None };
        let stop = app.state::<AppState>().control.clone();
        while !stop.stop.load(Ordering::SeqCst) {
            match checker.check(&app) {
                Ok(notes) => {
                    for (title, body) in notes {
                        let _ = app.notification().builder().title(title).body(body).show();
                    }
                }
                Err(e) => eprintln!("zman: {e}"),
            }
            let wake = Instant::now() + CHECK_EVERY;
            while !stop.stop.load(Ordering::SeqCst) && Instant::now() < wake {
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use zman_core::Event;

    const MIN: i64 = 60_000;

    fn summary(parts: &[(&str, i64)]) -> Summary {
        let mut start = 0;
        let events: Vec<Event> = parts
            .iter()
            .map(|(app, minutes)| {
                let event = Event {
                    id: format!("{app}{start}"),
                    device_id: "d".into(),
                    start_ms: start,
                    duration_ms: minutes * MIN,
                    app: app.to_string(),
                    title: String::new(),
                    data: serde_json::json!({}),
                    updated_at_ms: 0,
                };
                start += minutes * MIN;
                event
            })
            .collect();
        summarize(&events, Range { start_ms: 0, end_ms: start.max(1) }, &Categories::defaults(Lang::En))
    }

    #[test]
    fn goal_is_announced_only_once_reached() {
        let day = summary(&[("code.exe", 120), ("spotify.exe", 200)]);
        assert!(goal_note(&day, 0, Lang::En).is_none());
        assert!(goal_note(&day, 121, Lang::En).is_none());
        assert_eq!(goal_note(&day, 120, Lang::En).unwrap().1, "2h 00m of productive time today. Well done.");
    }

    #[test]
    fn nudge_needs_most_of_the_window_wasted() {
        assert!(nudge_note(&summary(&[("code.exe", 10), ("steam.exe", 20)]), Lang::En).is_none());
        let note = nudge_note(&summary(&[("code.exe", 4), ("steam.exe", 26)]), Lang::Es).unwrap();
        assert!(note.1.starts_with("26 de los últimos 30 minutos se fueron en Entertainment"));
    }

    #[test]
    fn recap_compares_with_the_week_before() {
        let good = summary(&[("code.exe", 600)]);
        let mixed = summary(&[("code.exe", 300), ("steam.exe", 300)]);
        let empty = summary(&[]);
        assert!(recap_note(&empty, &good, Lang::En).is_none());
        assert_eq!(recap_note(&good, &mixed, Lang::En).unwrap().1, "10h 00m tracked, productivity 100/100 (+50 vs the week before).");
        assert_eq!(recap_note(&good, &empty, Lang::Es).unwrap().1, "10h 00m registradas, productividad 100/100.");
    }
}
