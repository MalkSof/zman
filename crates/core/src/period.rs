//! Calendar ranges (days, weeks, hours) in the user's time zone, expressed
//! as millisecond timestamps so the rest of the core stays time-zone free.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

use crate::error::{Error, Result};
use crate::lang::Lang;

/// A half-open span of time: `[start_ms, end_ms)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub start_ms: i64,
    pub end_ms: i64,
}

impl Range {
    pub fn duration_ms(&self) -> i64 {
        self.end_ms - self.start_ms
    }
}

/// Today's date in `tz`.
pub fn today(tz: &TimeZone) -> Date {
    date_of(crate::now_ms(), tz)
}

/// The calendar date a timestamp falls on in `tz`.
pub fn date_of(ms: i64, tz: &TimeZone) -> Date {
    Timestamp::from_millisecond(ms)
        .unwrap_or(Timestamp::UNIX_EPOCH)
        .to_zoned_datetime(tz.clone())
        .date()
}

pub fn parse_date(text: &str) -> Result<Date> {
    text.trim()
        .parse::<Date>()
        .map_err(|_| Error::Config(format!("invalid date \"{text}\": expected YYYY-MM-DD")))
}

/// First instant of `date` in `tz`.
pub fn start_of_day(date: Date, tz: &TimeZone) -> Result<i64> {
    Ok(date.to_zoned_datetime(tz.clone())?.timestamp().as_millisecond())
}

pub fn add_days(date: Date, days: i64) -> Result<Date> {
    Ok(date.checked_add(days.days())?)
}

/// The whole of `date`.
pub fn day_range(date: Date, tz: &TimeZone) -> Result<Range> {
    days_range(date, 1, tz)
}

/// `days` whole days starting at `first`.
pub fn days_range(first: Date, days: i64, tz: &TimeZone) -> Result<Range> {
    Ok(Range {
        start_ms: start_of_day(first, tz)?,
        end_ms: start_of_day(add_days(first, days)?, tz)?,
    })
}

/// The Monday of the week `date` belongs to.
pub fn week_start(date: Date) -> Result<Date> {
    add_days(date, -(date.weekday().to_monday_zero_offset() as i64))
}

/// Boundaries of each day in `days` days starting at `first`: `days + 1`
/// timestamps. Days are not assumed to last 24 hours (DST).
pub fn day_edges(first: Date, days: i64, tz: &TimeZone) -> Result<Vec<i64>> {
    (0..=days).map(|i| start_of_day(add_days(first, i)?, tz)).collect()
}

/// Boundaries of each clock hour of `date`. Normally 25 timestamps; one
/// fewer or more on days when the clocks change.
pub fn hour_edges(date: Date, tz: &TimeZone) -> Result<Vec<i64>> {
    const HOUR_MS: i64 = 3_600_000;
    let range = day_range(date, tz)?;
    let mut edges: Vec<i64> = (range.start_ms..range.end_ms).step_by(HOUR_MS as usize).collect();
    edges.push(range.end_ms);
    Ok(edges)
}

/// "2h 05m", "45m", "30s". Used everywhere a duration is shown.
pub fn format_duration(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    let (hours, minutes) = (secs / 3600, secs / 60 % 60);
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{secs}s")
    }
}

/// "Friday, November 20" / "viernes 20 de noviembre".
pub fn format_long_date(date: Date, lang: Lang) -> String {
    let weekday = weekday_name(date, lang);
    let month = month_name(date, lang);
    match lang {
        Lang::En => format!("{weekday}, {month} {}", date.day()),
        Lang::Es => format!("{weekday} {} de {month}", date.day()),
    }
}

/// "Nov 20" / "20 nov".
pub fn format_short_date(date: Date, lang: Lang) -> String {
    let month: String = month_name(date, lang).chars().take(3).collect();
    match lang {
        Lang::En => format!("{month} {}", date.day()),
        Lang::Es => format!("{} {month}", date.day()),
    }
}

pub fn weekday_name(date: Date, lang: Lang) -> &'static str {
    const EN: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
    const ES: [&str; 7] = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"];
    let index = date.weekday().to_monday_zero_offset() as usize;
    match lang {
        Lang::En => EN[index],
        Lang::Es => ES[index],
    }
}

pub fn month_name(date: Date, lang: Lang) -> &'static str {
    const EN: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    const ES: [&str; 12] = [
        "enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre",
        "octubre", "noviembre", "diciembre",
    ];
    let index = (date.month() - 1) as usize;
    match lang {
        Lang::En => EN[index],
        Lang::Es => ES[index],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> Date {
        parse_date(text).unwrap()
    }

    #[test]
    fn day_range_in_utc_is_24_hours() {
        let range = day_range(date("2026-11-20"), &TimeZone::UTC).unwrap();
        assert_eq!(range.start_ms, 1_795_132_800_000);
        assert_eq!(range.duration_ms(), 86_400_000);
    }

    #[test]
    fn day_range_respects_time_zone() {
        let tz = TimeZone::get("America/Argentina/Buenos_Aires").unwrap();
        let utc = day_range(date("2026-11-20"), &TimeZone::UTC).unwrap();
        let local = day_range(date("2026-11-20"), &tz).unwrap();
        // Buenos Aires is UTC-3: its midnight happens 3 hours later.
        assert_eq!(local.start_ms - utc.start_ms, 3 * 3_600_000);
        assert_eq!(date_of(local.start_ms, &tz), date("2026-11-20"));
        assert_eq!(date_of(local.start_ms - 1, &tz), date("2026-11-19"));
    }

    #[test]
    fn week_starts_on_monday() {
        // 2026-11-20 is a Friday.
        assert_eq!(week_start(date("2026-11-20")).unwrap(), date("2026-11-16"));
        assert_eq!(week_start(date("2026-11-16")).unwrap(), date("2026-11-16"));
        assert_eq!(week_start(date("2026-11-22")).unwrap(), date("2026-11-16"));
    }

    #[test]
    fn edges() {
        let days = day_edges(date("2026-11-16"), 7, &TimeZone::UTC).unwrap();
        assert_eq!(days.len(), 8);
        assert_eq!(days[7] - days[0], 7 * 86_400_000);

        let hours = hour_edges(date("2026-11-20"), &TimeZone::UTC).unwrap();
        assert_eq!(hours.len(), 25);
        assert_eq!(hours[1] - hours[0], 3_600_000);
    }

    #[test]
    fn dst_day_has_23_hours() {
        let tz = TimeZone::get("America/New_York").unwrap();
        let hours = hour_edges(date("2026-03-08"), &tz).unwrap();
        assert_eq!(hours.len(), 24);
        assert_eq!(hours.last().unwrap() - hours[0], 23 * 3_600_000);
    }

    #[test]
    fn durations() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(59_999), "59s");
        assert_eq!(format_duration(60_000), "1m");
        assert_eq!(format_duration(3_600_000 + 5 * 60_000), "1h 05m");
        assert_eq!(format_duration(-5), "0s");
    }

    #[test]
    fn date_formatting() {
        let d = date("2026-11-20");
        assert_eq!(format_long_date(d, Lang::En), "Friday, November 20");
        assert_eq!(format_long_date(d, Lang::Es), "viernes 20 de noviembre");
        assert_eq!(format_short_date(d, Lang::En), "Nov 20");
        assert_eq!(format_short_date(d, Lang::Es), "20 nov");
    }

    #[test]
    fn bad_dates_are_rejected() {
        assert!(parse_date("2026-13-01").is_err());
        assert!(parse_date("yesterday").is_err());
    }
}
