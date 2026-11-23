//! Settings the app manages for the user: site blocking, category overrides
//! and the daily goal. Unlike `config.toml`, which the user edits by hand,
//! these live in the database so every zman process sees a change at once.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

use crate::category::{Categories, CategoryRule, domain_matches};
use crate::config::Config;
use crate::error::Result;
use crate::event::{Event, domain_of};
use crate::lang::Lang;
use crate::report::Summary;
use crate::store::Store;

const KEY: &str = "settings";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub block: Block,
    /// Apps and sites the user moved to a category from the dashboard.
    pub overrides: Vec<Override>,
    /// Productive minutes to reach each day. 0 means no goal.
    pub goal_minutes: u32,
    /// Desktop notifications (goal reached, long distractions, weekly recap).
    pub notifications: bool,
    /// Lets AI assistants start and stop focus sessions through MCP.
    pub assistant_focus: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            block: Block::default(),
            overrides: Vec::new(),
            goal_minutes: 0,
            notifications: true,
            assistant_focus: false,
        }
    }
}

/// What the browser extension must keep the user away from.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Block {
    /// Sites that are always blocked, subdomains included.
    pub sites: Vec<String>,
    /// While in the future, a focus session is on: distracting sites are blocked.
    pub focus_until_ms: i64,
    pub limits: Vec<Limit>,
    /// Recurring hours during which distracting sites are blocked.
    pub schedules: Vec<Schedule>,
}

/// A daily allowance. `target` is a site (`youtube.com`) or a category name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    pub target: String,
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    /// ISO weekdays: 1 is Monday, 7 is Sunday.
    pub days: Vec<u8>,
    /// Minutes since midnight, `start_min` inclusive and `end_min` exclusive.
    pub start_min: u16,
    pub end_min: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Override {
    /// "app" or "site".
    pub kind: String,
    /// The app (`code.exe`) or the site (`github.com`).
    pub name: String,
    pub category: String,
}

/// Why a page is blocked, as told to the extension.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Verdict {
    /// "site", "focus", "schedule" or "limit".
    pub reason: &'static str,
    pub site: String,
    /// When the block ends, if it has an end.
    pub until_ms: Option<i64>,
    /// The daily allowance that ran out, for "limit".
    pub limit_minutes: Option<u32>,
}

impl Settings {
    /// Reads the settings. Missing or unreadable ones give the defaults, so
    /// a damaged value can never stop tracking.
    pub fn load(store: &Store) -> Result<Settings> {
        Ok(store
            .meta_get(KEY)?
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default())
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        store.meta_set(KEY, &serde_json::to_string(self).expect("settings are plain data"))
    }

    /// The category rules in force: the user's overrides first (sites, then
    /// apps), followed by the rules of `config.toml` or the built-in ones.
    pub fn categories(&self, config: &Config, lang: Lang) -> Categories {
        let base = config.categories(lang);
        let mut rules = Vec::new();
        for kind in ["site", "app"] {
            for o in self.overrides.iter().filter(|o| o.kind == kind) {
                let score = base.list().iter().find(|(name, _)| *name == o.category).map_or(0, |(_, score)| *score);
                let mut rule = CategoryRule {
                    name: o.category.clone(),
                    score,
                    apps: Vec::new(),
                    titles: Vec::new(),
                    domains: Vec::new(),
                };
                if kind == "site" { rule.domains.push(o.name.clone()) } else { rule.apps.push(o.name.clone()) }
                rules.push(rule);
            }
        }
        base.with_first(rules)
    }

    /// Moves an app or site to `category`, or back to the rules with `None`.
    pub fn set_override(&mut self, kind: &str, name: &str, category: Option<&str>) {
        self.overrides.retain(|o| !(o.kind == kind && o.name.eq_ignore_ascii_case(name)));
        if let Some(category) = category {
            self.overrides.push(Override { kind: kind.into(), name: name.into(), category: category.into() });
        }
    }
}

impl Schedule {
    fn contains(&self, weekday: u8, minute: u16) -> bool {
        self.days.contains(&weekday) && self.start_min <= minute && minute < self.end_min
    }
}

impl Block {
    pub fn focus_active(&self, now_ms: i64) -> bool {
        self.focus_until_ms > now_ms
    }

    /// End of the schedule that is on right now, if any.
    fn schedule_end_ms(&self, now_ms: i64, tz: &TimeZone) -> Option<i64> {
        let now = Timestamp::from_millisecond(now_ms).ok()?.to_zoned_datetime(tz.clone());
        let weekday = now.weekday().to_monday_one_offset() as u8;
        let minute = now.hour() as u16 * 60 + now.minute() as u16;
        let end = self.schedules.iter().filter(|s| s.contains(weekday, minute)).map(|s| s.end_min).max()?;
        let seconds_left = (end as i64 - minute as i64) * 60 - now.second() as i64;
        Some(now_ms + seconds_left * 1000)
    }

    /// Whether distracting sites are blocked right now, and until when.
    pub fn strict_until_ms(&self, now_ms: i64, tz: &TimeZone) -> Option<(&'static str, i64)> {
        if self.focus_active(now_ms) {
            return Some(("focus", self.focus_until_ms));
        }
        self.schedule_end_ms(now_ms, tz).map(|end| ("schedule", end))
    }

    /// Decides whether the page at `url` must be blocked.
    ///
    /// `today` is only called when a daily limit has to be checked; it
    /// returns the summary of the current day.
    pub fn check(
        &self,
        categories: &Categories,
        url: &str,
        title: &str,
        now_ms: i64,
        tz: &TimeZone,
        today: impl FnOnce() -> Option<Summary>,
    ) -> Option<Verdict> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return None;
        }
        let site = domain_of(url)?;
        let verdict = |reason, until_ms, limit_minutes| Some(Verdict { reason, site: site.clone(), until_ms, limit_minutes });

        if self.sites.iter().any(|pattern| domain_matches(pattern, &site)) {
            return verdict("site", None, None);
        }

        let page = Event {
            id: String::new(),
            device_id: String::new(),
            start_ms: now_ms,
            duration_ms: 0,
            app: String::new(),
            title: title.to_string(),
            data: serde_json::json!({ "url": url }),
            updated_at_ms: now_ms,
        };
        let (category, score) = categories.classify(&page);
        if score < 0 {
            if let Some((reason, until)) = self.strict_until_ms(now_ms, tz) {
                return verdict(reason, Some(until), None);
            }
        }

        let applies = |limit: &&Limit| domain_matches(&limit.target, &site) || limit.target.eq_ignore_ascii_case(category);
        if !self.limits.iter().any(|limit| applies(&limit)) {
            return None;
        }
        let today = today()?;
        for limit in self.limits.iter().filter(applies) {
            let used_ms: i64 = if domain_matches(&limit.target, &site) {
                today.domains.iter().filter(|d| domain_matches(&limit.target, &d.domain)).map(|d| d.ms).sum()
            } else {
                today.categories.iter().filter(|c| c.name.eq_ignore_ascii_case(&limit.target)).map(|c| c.ms).sum()
            };
            if used_ms >= limit.minutes as i64 * 60_000 {
                return verdict("limit", Some(today.range.end_ms), Some(limit.minutes));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::period::Range;
    use crate::report::summarize;

    const MIN: i64 = 60_000;
    /// 2026-11-20T00:00:00Z, a Friday.
    const DAY: i64 = 1_795_132_800_000;

    fn categories() -> Categories {
        Categories::defaults(Lang::En)
    }

    fn today(minutes_of_youtube: i64) -> Option<Summary> {
        let event = Event {
            id: "a".into(),
            device_id: "d".into(),
            start_ms: DAY,
            duration_ms: minutes_of_youtube * MIN,
            app: "chrome.exe".into(),
            title: "Video".into(),
            data: serde_json::json!({ "url": "https://www.youtube.com/watch" }),
            updated_at_ms: DAY,
        };
        Some(summarize(&[event], Range { start_ms: DAY, end_ms: DAY + 24 * 60 * MIN }, &categories()))
    }

    fn check(block: &Block, url: &str, now_ms: i64, used: i64) -> Option<Verdict> {
        block.check(&categories(), url, "", now_ms, &TimeZone::UTC, || today(used))
    }

    #[test]
    fn nothing_is_blocked_by_default() {
        let block = Block::default();
        assert_eq!(check(&block, "https://youtube.com/", DAY, 600), None);
    }

    #[test]
    fn listed_sites_are_always_blocked_with_subdomains() {
        let block = Block { sites: vec!["reddit.com".into()], ..Block::default() };
        assert_eq!(check(&block, "https://old.reddit.com/r/rust", DAY, 0).unwrap().reason, "site");
        assert_eq!(check(&block, "https://notreddit.com/", DAY, 0), None);
        assert_eq!(check(&block, "chrome://settings", DAY, 0), None);
    }

    #[test]
    fn focus_blocks_only_distracting_sites_and_ends() {
        let block = Block { focus_until_ms: DAY + 50 * MIN, ..Block::default() };
        let verdict = check(&block, "https://www.youtube.com/watch?v=1", DAY, 0).unwrap();
        assert_eq!((verdict.reason, verdict.until_ms), ("focus", Some(DAY + 50 * MIN)));
        assert_eq!(verdict.site, "youtube.com");
        assert_eq!(check(&block, "https://github.com/", DAY, 0), None);
        assert_eq!(check(&block, "https://example.org/", DAY, 0), None);
        assert_eq!(check(&block, "https://www.youtube.com/", DAY + 51 * MIN, 0), None);
    }

    #[test]
    fn schedules_follow_weekday_and_hours() {
        // Fridays from 09:00 to 18:00.
        let block = Block {
            schedules: vec![Schedule { days: vec![5], start_min: 9 * 60, end_min: 18 * 60 }],
            ..Block::default()
        };
        let at = |hours: i64, minutes: i64| DAY + (hours * 60 + minutes) * MIN;
        let verdict = check(&block, "https://netflix.com/", at(10, 30), 0).unwrap();
        assert_eq!((verdict.reason, verdict.until_ms), ("schedule", Some(at(18, 0))));
        assert_eq!(check(&block, "https://netflix.com/", at(8, 59), 0), None);
        assert_eq!(check(&block, "https://netflix.com/", at(18, 0), 0), None);
        // Saturday at the same hour.
        assert_eq!(check(&block, "https://netflix.com/", at(24 + 10, 0), 0), None);
    }

    #[test]
    fn limits_by_site_and_by_category() {
        let by_site = Block { limits: vec![Limit { target: "youtube.com".into(), minutes: 30 }], ..Block::default() };
        assert_eq!(check(&by_site, "https://youtube.com/", DAY + 60 * MIN, 29), None);
        let verdict = check(&by_site, "https://m.youtube.com/", DAY + 60 * MIN, 30).unwrap();
        assert_eq!((verdict.reason, verdict.limit_minutes), ("limit", Some(30)));
        assert_eq!(verdict.until_ms, Some(DAY + 24 * 60 * MIN));
        assert_eq!(check(&by_site, "https://netflix.com/", DAY, 500), None);

        // Time on YouTube counts against the whole category.
        let by_category =
            Block { limits: vec![Limit { target: "entertainment".into(), minutes: 20 }], ..Block::default() };
        assert_eq!(check(&by_category, "https://netflix.com/", DAY, 25).unwrap().reason, "limit");
        assert_eq!(check(&by_category, "https://github.com/", DAY, 25), None);
    }

    #[test]
    fn overrides_win_over_rules_and_can_be_undone() {
        let mut settings = Settings::default();
        let config = Config::default();
        let event = |app: &str, url: &str| Event {
            id: String::new(),
            device_id: String::new(),
            start_ms: 0,
            duration_ms: 1,
            app: app.into(),
            title: String::new(),
            data: if url.is_empty() { serde_json::json!({}) } else { serde_json::json!({ "url": url }) },
            updated_at_ms: 0,
        };
        assert_eq!(settings.categories(&config, Lang::En), Categories::defaults(Lang::En));

        settings.set_override("site", "youtube.com", Some("Work"));
        settings.set_override("app", "mystery.exe", Some("Entertainment"));
        let categories = settings.categories(&config, Lang::En);
        assert_eq!(categories.classify(&event("chrome.exe", "https://www.youtube.com/x")), ("Work", 2));
        assert_eq!(categories.classify(&event("Mystery.exe", "")), ("Entertainment", -2));

        settings.set_override("site", "YouTube.com", None);
        let categories = settings.categories(&config, Lang::En);
        assert_eq!(categories.classify(&event("chrome.exe", "https://www.youtube.com/x")).0, "Entertainment");
    }

    #[test]
    fn settings_round_trip_through_the_store() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(Settings::load(&store).unwrap(), Settings::default());
        let mut settings = Settings { goal_minutes: 240, ..Settings::default() };
        settings.block.sites.push("x.com".into());
        settings.save(&store).unwrap();
        assert_eq!(Settings::load(&store).unwrap(), settings);
        store.meta_set(KEY, "{broken").unwrap();
        assert_eq!(Settings::load(&store).unwrap(), Settings::default());
    }
}
