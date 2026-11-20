//! Aggregations over events. Everything here works on a slice of events
//! already loaded, so the same code serves the local database and, later,
//! any other source of events.

use std::collections::HashMap;

use crate::category::Categories;
use crate::event::Event;
use crate::period::Range;

#[derive(Debug, Clone, PartialEq)]
pub struct CategoryUsage {
    pub name: String,
    pub score: i8,
    pub ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppUsage {
    pub app: String,
    /// Category where this app spent most of its time.
    pub category: String,
    pub score: i8,
    pub ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainUsage {
    pub domain: String,
    pub category: String,
    pub score: i8,
    pub ms: i64,
}

/// Time tracked within a range, broken down by category, app and site.
/// All lists are sorted by time, longest first.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub range: Range,
    pub total_ms: i64,
    /// 0 (all very distracting) to 100 (all very productive); `None` when
    /// nothing was tracked.
    pub productivity: Option<u8>,
    pub categories: Vec<CategoryUsage>,
    pub apps: Vec<AppUsage>,
    pub domains: Vec<DomainUsage>,
}

/// Milliseconds of `event` that fall inside `range`.
fn overlap_ms(event: &Event, range: Range) -> i64 {
    (event.end_ms().min(range.end_ms) - event.start_ms.max(range.start_ms)).max(0)
}

pub fn summarize(events: &[Event], range: Range, categories: &Categories) -> Summary {
    let mut total_ms = 0;
    let mut weighted = 0i64;
    let mut by_category: HashMap<&str, (i8, i64)> = HashMap::new();
    // Per app / domain: time spent in each category.
    let mut by_app: HashMap<&str, HashMap<&str, (i8, i64)>> = HashMap::new();
    let mut by_domain: HashMap<String, HashMap<&str, (i8, i64)>> = HashMap::new();

    for event in events {
        let ms = overlap_ms(event, range);
        if ms == 0 {
            continue;
        }
        let (category, score) = categories.classify(event);
        total_ms += ms;
        weighted += ms * (score as i64 + 2);
        by_category.entry(category).or_insert((score, 0)).1 += ms;
        by_app.entry(&event.app).or_default().entry(category).or_insert((score, 0)).1 += ms;
        if let Some(domain) = event.domain() {
            by_domain.entry(domain).or_default().entry(category).or_insert((score, 0)).1 += ms;
        }
    }

    let mut categories: Vec<CategoryUsage> = by_category
        .into_iter()
        .map(|(name, (score, ms))| CategoryUsage { name: name.to_string(), score, ms })
        .collect();
    categories.sort_by(|a, b| b.ms.cmp(&a.ms).then_with(|| a.name.cmp(&b.name)));

    // The main category of an app or site is where it spent the most time.
    let dominant = |split: &HashMap<&str, (i8, i64)>| {
        let total: i64 = split.values().map(|(_, ms)| ms).sum();
        let (name, (score, _)) = split
            .iter()
            .max_by(|a, b| a.1.1.cmp(&b.1.1).then_with(|| b.0.cmp(a.0)))
            .expect("at least one category per entry");
        (name.to_string(), *score, total)
    };

    let mut apps: Vec<AppUsage> = by_app
        .iter()
        .map(|(app, split)| {
            let (category, score, ms) = dominant(split);
            AppUsage { app: app.to_string(), category, score, ms }
        })
        .collect();
    apps.sort_by(|a, b| b.ms.cmp(&a.ms).then_with(|| a.app.cmp(&b.app)));

    let mut domains: Vec<DomainUsage> = by_domain
        .iter()
        .map(|(domain, split)| {
            let (category, score, ms) = dominant(split);
            DomainUsage { domain: domain.clone(), category, score, ms }
        })
        .collect();
    domains.sort_by(|a, b| b.ms.cmp(&a.ms).then_with(|| a.domain.cmp(&b.domain)));

    let productivity = (total_ms > 0).then(|| ((weighted * 25) as f64 / total_ms as f64).round() as u8);
    Summary { range, total_ms, productivity, categories, apps, domains }
}

/// One summary per consecutive pair of `edges` (e.g. the hours of a day or
/// the days of a week).
pub fn timeline(events: &[Event], edges: &[i64], categories: &Categories) -> Vec<Summary> {
    edges
        .windows(2)
        .map(|w| summarize(events, Range { start_ms: w[0], end_ms: w[1] }, categories))
        .collect()
}

/// A stretch of sustained productive work.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusBlock {
    pub start_ms: i64,
    pub end_ms: i64,
    /// Time inside the block actually spent on productive activity.
    pub productive_ms: i64,
    /// Category with the most time in the block.
    pub category: String,
}

/// Finds focus blocks: runs of productive events (score > 0) where no
/// interruption lasts longer than `max_break_ms`, and whose productive time
/// adds up to at least `min_ms`.
pub fn focus_blocks(
    events: &[Event],
    range: Range,
    categories: &Categories,
    min_ms: i64,
    max_break_ms: i64,
) -> Vec<FocusBlock> {
    struct Open<'a> {
        start_ms: i64,
        end_ms: i64,
        productive_ms: i64,
        by_category: HashMap<&'a str, i64>,
    }
    fn close(open: Open<'_>, min_ms: i64, out: &mut Vec<FocusBlock>) {
        if open.productive_ms < min_ms {
            return;
        }
        let category = open
            .by_category
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(name, _)| name.to_string())
            .unwrap_or_default();
        out.push(FocusBlock {
            start_ms: open.start_ms,
            end_ms: open.end_ms,
            productive_ms: open.productive_ms,
            category,
        });
    }

    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|e| e.start_ms);

    let mut blocks = Vec::new();
    let mut open: Option<Open<'_>> = None;
    for event in sorted {
        let ms = overlap_ms(event, range);
        let (category, score) = categories.classify(event);
        if ms == 0 || score <= 0 {
            continue;
        }
        let start = event.start_ms.max(range.start_ms);
        let end = start + ms;
        match open.as_mut() {
            Some(block) if start - block.end_ms <= max_break_ms => {
                block.end_ms = block.end_ms.max(end);
                block.productive_ms += ms;
                *block.by_category.entry(category).or_default() += ms;
            }
            _ => {
                if let Some(done) = open.take() {
                    close(done, min_ms, &mut blocks);
                }
                open = Some(Open {
                    start_ms: start,
                    end_ms: end,
                    productive_ms: ms,
                    by_category: HashMap::from([(category, ms)]),
                });
            }
        }
    }
    if let Some(done) = open {
        close(done, min_ms, &mut blocks);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::CategoryRule;
    use crate::lang::Lang;

    const MIN: i64 = 60_000;

    fn event(app: &str, title: &str, start_min: i64, minutes: i64) -> Event {
        Event {
            id: format!("{app}-{start_min}"),
            device_id: "dev".into(),
            start_ms: start_min * MIN,
            duration_ms: minutes * MIN,
            app: app.into(),
            title: title.into(),
            data: serde_json::json!({}),
            updated_at_ms: 0,
        }
    }

    fn cats() -> Categories {
        let rule = |name: &str, score: i8, app: &str| CategoryRule {
            name: name.into(),
            score,
            apps: vec![app.into()],
            titles: vec![],
            domains: vec![],
        };
        Categories::new(
            vec![rule("Dev", 2, "code.exe"), rule("Chat", 0, "slack.exe"), rule("Fun", -2, "game.exe")],
            Lang::En,
        )
    }

    fn range(start_min: i64, end_min: i64) -> Range {
        Range { start_ms: start_min * MIN, end_ms: end_min * MIN }
    }

    #[test]
    fn empty_summary() {
        let s = summarize(&[], range(0, 60), &cats());
        assert_eq!(s.total_ms, 0);
        assert_eq!(s.productivity, None);
        assert!(s.categories.is_empty() && s.apps.is_empty());
    }

    #[test]
    fn totals_by_category_and_app() {
        let events = [
            event("code.exe", "a.rs", 0, 30),
            event("slack.exe", "general", 30, 10),
            event("code.exe", "b.rs", 40, 20),
            event("other.exe", "?", 60, 5),
        ];
        let s = summarize(&events, range(0, 120), &cats());
        assert_eq!(s.total_ms, 65 * MIN);

        let names: Vec<(&str, i64)> = s.categories.iter().map(|c| (c.name.as_str(), c.ms / MIN)).collect();
        assert_eq!(names, vec![("Dev", 50), ("Chat", 10), ("Uncategorized", 5)]);

        assert_eq!(s.apps[0].app, "code.exe");
        assert_eq!(s.apps[0].ms, 50 * MIN);
        assert_eq!(s.apps[0].category, "Dev");
        assert_eq!(s.apps.len(), 3);
    }

    #[test]
    fn events_are_clipped_to_the_range() {
        // 23:30 -> 00:30 crossing the boundary at minute 60.
        let events = [event("code.exe", "a.rs", 30, 60)];
        assert_eq!(summarize(&events, range(0, 60), &cats()).total_ms, 30 * MIN);
        assert_eq!(summarize(&events, range(60, 120), &cats()).total_ms, 30 * MIN);
        assert_eq!(summarize(&events, range(200, 300), &cats()).total_ms, 0);
    }

    #[test]
    fn productivity_score() {
        let all_dev = [event("code.exe", "a", 0, 10)];
        assert_eq!(summarize(&all_dev, range(0, 60), &cats()).productivity, Some(100));

        let all_fun = [event("game.exe", "g", 0, 10)];
        assert_eq!(summarize(&all_fun, range(0, 60), &cats()).productivity, Some(0));

        let neutral = [event("slack.exe", "s", 0, 10)];
        assert_eq!(summarize(&neutral, range(0, 60), &cats()).productivity, Some(50));

        let mixed = [event("code.exe", "a", 0, 30), event("game.exe", "g", 30, 10)];
        assert_eq!(summarize(&mixed, range(0, 60), &cats()).productivity, Some(75));
    }

    #[test]
    fn domains_are_aggregated() {
        let mut a = event("chrome.exe", "Repo", 0, 10);
        a.data = serde_json::json!({ "url": "https://github.com/a/b" });
        let mut b = event("chrome.exe", "Issue", 10, 5);
        b.data = serde_json::json!({ "url": "https://www.github.com/a/b/issues" });
        let s = summarize(&[a, b, event("code.exe", "x", 15, 5)], range(0, 60), &cats());
        assert_eq!(s.domains.len(), 1);
        assert_eq!(s.domains[0].domain, "github.com");
        assert_eq!(s.domains[0].ms, 15 * MIN);
    }

    #[test]
    fn timeline_splits_by_bucket() {
        let events = [event("code.exe", "a", 50, 30)];
        let buckets = timeline(&events, &[0, 60 * MIN, 120 * MIN, 180 * MIN], &cats());
        let totals: Vec<i64> = buckets.iter().map(|b| b.total_ms / MIN).collect();
        assert_eq!(totals, vec![10, 20, 0]);
    }

    #[test]
    fn focus_block_survives_short_breaks() {
        let events = [
            event("code.exe", "a", 0, 20),
            event("slack.exe", "quick reply", 20, 1),
            event("code.exe", "b", 21, 20),
        ];
        let blocks = focus_blocks(&events, range(0, 120), &cats(), 25 * MIN, 2 * MIN);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start_ms, 0);
        assert_eq!(blocks[0].end_ms, 41 * MIN);
        assert_eq!(blocks[0].productive_ms, 40 * MIN);
        assert_eq!(blocks[0].category, "Dev");
    }

    #[test]
    fn long_break_splits_and_short_blocks_are_dropped() {
        let events = [
            event("code.exe", "a", 0, 30),
            event("game.exe", "g", 30, 10),
            event("code.exe", "b", 40, 10),
        ];
        let blocks = focus_blocks(&events, range(0, 120), &cats(), 25 * MIN, 2 * MIN);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].end_ms, 30 * MIN);
    }

    #[test]
    fn unproductive_time_is_never_focus() {
        let events = [event("game.exe", "g", 0, 60), event("slack.exe", "s", 60, 60)];
        assert!(focus_blocks(&events, range(0, 180), &cats(), 25 * MIN, 2 * MIN).is_empty());
    }
}
