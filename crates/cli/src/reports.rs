//! `zman today`, `zman week` and `zman apps`.

use std::error::Error;

use anstream::println;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use zman_core::event::display_app;
use zman_core::period::{self, format_duration};
use zman_core::report::{self, Summary};
use zman_core::{Config, Lang, Paths, Range, Store};

use crate::ui::{bar, bold, dim, fit, paint, productivity_style, score_style, spark};

const BAR_WIDTH: usize = 24;
const NAME_WIDTH: usize = 18;

/// Everything a report needs, loaded once.
struct Context {
    lang: Lang,
    tz: TimeZone,
    store: Store,
    config: Config,
}

impl Context {
    fn load(lang: Lang) -> Result<Context, Box<dyn Error>> {
        let paths = Paths::resolve()?;
        if !paths.db_file.exists() {
            return Err(lang
                .pick(
                    "no data yet: start zman-daemon to begin tracking",
                    "todavía no hay datos: iniciá zman-daemon para empezar a registrar",
                )
                .into());
        }
        Ok(Context {
            lang,
            tz: TimeZone::system(),
            store: Store::open(&paths.db_file)?,
            config: Config::load(&paths.config_file)?,
        })
    }

    fn date(&self, arg: Option<&str>) -> Result<Date, Box<dyn Error>> {
        match arg {
            Some(text) => Ok(period::parse_date(text)?),
            None => Ok(period::today(&self.tz)),
        }
    }

    fn summary(&self, range: Range) -> Result<Summary, Box<dyn Error>> {
        let events = self.store.events_between(range)?;
        Ok(report::summarize(&events, range, &self.config.categories(self.lang)))
    }

    fn timeline(&self, edges: &[i64]) -> Result<Vec<Summary>, Box<dyn Error>> {
        let range = Range { start_ms: edges[0], end_ms: edges[edges.len() - 1] };
        let events = self.store.events_between(range)?;
        Ok(report::timeline(&events, edges, &self.config.categories(self.lang)))
    }
}

pub fn today(lang: Lang, date: Option<&str>) -> Result<(), Box<dyn Error>> {
    let ctx = Context::load(lang)?;
    let date = ctx.date(date)?;
    let summary = ctx.summary(period::day_range(date, &ctx.tz)?)?;

    let is_today = date == period::today(&ctx.tz);
    let long_date = period::format_long_date(date, lang);
    let title = if is_today {
        format!("{} · {long_date}", lang.pick("Today", "Hoy"))
    } else {
        capitalize(&long_date)
    };
    print_header(&title, &summary, None, lang);
    if summary.total_ms == 0 {
        return Ok(());
    }

    // One two-cell column per hour, as tall as the share of the hour tracked.
    let hours = ctx.timeline(&period::hour_edges(date, &ctx.tz)?)?;
    section(lang.pick("Hours", "Horas"));
    let mut line = String::new();
    for hour in &hours {
        let block = spark(hour.total_ms as f64 / hour.range.duration_ms() as f64);
        let cell = format!("{block}{block}");
        match hour.productivity {
            Some(p) => line.push_str(&paint(productivity_style(p), &cell)),
            None => line.push_str(&paint(dim(), &cell)),
        }
    }
    println!("  {line}");
    let labels: String = (0..hours.len())
        .map(|h| if h % 3 == 0 { format!("{h:<2}") } else { "  ".to_string() })
        .collect();
    println!("  {}", paint(dim(), &labels));

    print_categories(&summary, lang);
    print_apps(&summary, 8, lang);
    print_domains(&summary, 6, lang);
    println!();
    Ok(())
}

pub fn week(lang: Lang, date: Option<&str>) -> Result<(), Box<dyn Error>> {
    let ctx = Context::load(lang)?;
    let monday = period::week_start(ctx.date(date)?)?;
    let sunday = period::add_days(monday, 6)?;
    let summary = ctx.summary(period::days_range(monday, 7, &ctx.tz)?)?;
    let days = ctx.timeline(&period::day_edges(monday, 7, &ctx.tz)?)?;

    let title = format!(
        "{} {} – {}",
        lang.pick("Week of", "Semana del"),
        period::format_short_date(monday, lang),
        period::format_short_date(sunday, lang),
    );
    let active_days = days.iter().filter(|d| d.total_ms > 0).count() as i64;
    let average = (active_days > 0).then(|| summary.total_ms / active_days);
    print_header(&title, &summary, average, lang);
    if summary.total_ms == 0 {
        return Ok(());
    }

    section(lang.pick("Days", "Días"));
    let longest = days.iter().map(|d| d.total_ms).max().unwrap_or(0).max(1);
    for (i, day) in days.iter().enumerate() {
        let date = period::add_days(monday, i as i64)?;
        let weekday: String = period::weekday_name(date, lang).chars().take(3).collect();
        let label = format!("{} {:>2}", capitalize(&weekday), date.day());
        let style = day.productivity.map_or(dim(), productivity_style);
        let score = day.productivity.map_or(String::new(), |p| p.to_string());
        println!(
            "  {}  {:>7}  {}  {}",
            fit(&label, 6),
            if day.total_ms > 0 { format_duration(day.total_ms) } else { "–".into() },
            paint(style, &bar(day.total_ms as f64 / longest as f64, BAR_WIDTH)),
            paint(style, &score),
        );
    }

    print_categories(&summary, lang);
    print_apps(&summary, 8, lang);
    print_domains(&summary, 6, lang);
    println!();
    Ok(())
}

pub fn apps(lang: Lang, date: Option<&str>, week: bool, limit: usize) -> Result<(), Box<dyn Error>> {
    let ctx = Context::load(lang)?;
    let date = ctx.date(date)?;
    let (range, title) = if week {
        let monday = period::week_start(date)?;
        let title = format!(
            "{} · {} {}",
            lang.pick("Apps", "Apps"),
            lang.pick("week of", "semana del"),
            period::format_short_date(monday, lang)
        );
        (period::days_range(monday, 7, &ctx.tz)?, title)
    } else {
        let title = format!("Apps · {}", period::format_long_date(date, lang));
        (period::day_range(date, &ctx.tz)?, title)
    };
    let summary = ctx.summary(range)?;
    print_header(&title, &summary, None, lang);
    if summary.total_ms > 0 {
        print_apps(&summary, limit, lang);
        println!();
    }
    Ok(())
}

fn print_header(title: &str, summary: &Summary, daily_average: Option<i64>, lang: Lang) {
    println!();
    println!("  {}", paint(bold(), title));
    let Some(productivity) = summary.productivity else {
        println!(
            "  {}",
            paint(
                dim(),
                lang.pick(
                    "Nothing tracked in this period. Is zman-daemon running?",
                    "No hay nada registrado en este período. ¿Está corriendo zman-daemon?",
                )
            )
        );
        println!();
        return;
    };
    let mut line = format!(
        "{} {}",
        paint(bold(), &format_duration(summary.total_ms)),
        lang.pick("tracked", "registradas"),
    );
    if let Some(average) = daily_average {
        line.push_str(&format!(
            "  ·  {} {}",
            format_duration(average),
            lang.pick("per active day", "por día activo")
        ));
    }
    let style = productivity_style(productivity);
    line.push_str(&format!(
        "  ·  {} {} {}",
        lang.pick("productivity", "productividad"),
        paint(style.bold(), &productivity.to_string()),
        paint(style, &bar(productivity as f64 / 100.0, 10)),
    ));
    println!("  {line}");
}

fn section(name: &str) {
    println!();
    println!("  {}", paint(dim().bold(), &name.to_uppercase()));
}

fn share(ms: i64, total_ms: i64) -> String {
    format!("{:>3}%", (ms as f64 / total_ms.max(1) as f64 * 100.0).round() as i64)
}

fn print_categories(summary: &Summary, lang: Lang) {
    section(lang.pick("Categories", "Categorías"));
    let longest = summary.categories.first().map_or(1, |c| c.ms.max(1));
    for category in &summary.categories {
        println!(
            "  {}  {:>7}  {}  {}",
            fit(&category.name, NAME_WIDTH),
            format_duration(category.ms),
            paint(dim(), &share(category.ms, summary.total_ms)),
            paint(score_style(category.score), &bar(category.ms as f64 / longest as f64, BAR_WIDTH)),
        );
    }
}

fn print_apps(summary: &Summary, limit: usize, lang: Lang) {
    section(lang.pick("Top apps", "Apps más usadas"));
    let longest = summary.apps.first().map_or(1, |a| a.ms.max(1));
    for app in summary.apps.iter().take(limit) {
        println!(
            "  {}  {:>7}  {}  {}  {}",
            fit(display_app(&app.app), NAME_WIDTH),
            format_duration(app.ms),
            paint(dim(), &share(app.ms, summary.total_ms)),
            paint(score_style(app.score), &bar(app.ms as f64 / longest as f64, BAR_WIDTH)),
            paint(dim(), &app.category),
        );
    }
    let hidden = summary.apps.len().saturating_sub(limit);
    if hidden > 0 {
        let more = format!("+{hidden} {}", lang.pick("more (see `zman apps`)", "más (ver `zman apps`)"));
        println!("  {}", paint(dim(), &more));
    }
}

fn print_domains(summary: &Summary, limit: usize, lang: Lang) {
    if summary.domains.is_empty() {
        return;
    }
    section(lang.pick("Top sites", "Sitios más visitados"));
    let longest = summary.domains.first().map_or(1, |d| d.ms.max(1));
    for site in summary.domains.iter().take(limit) {
        println!(
            "  {}  {:>7}  {}  {}  {}",
            fit(&site.domain, NAME_WIDTH),
            format_duration(site.ms),
            paint(dim(), &share(site.ms, summary.total_ms)),
            paint(score_style(site.score), &bar(site.ms as f64 / longest as f64, BAR_WIDTH)),
            paint(dim(), &site.category),
        );
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
