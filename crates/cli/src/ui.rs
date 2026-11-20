//! Small terminal drawing helpers: colors, bars and sparklines.
//!
//! Output goes through `anstream`, which strips the colors when the output
//! is piped or `NO_COLOR` is set.

use anstyle::{AnsiColor, Style};

pub fn bold() -> Style {
    Style::new().bold()
}

pub fn dim() -> Style {
    Style::new().dimmed()
}

fn color(color: AnsiColor) -> Style {
    Style::new().fg_color(Some(color.into()))
}

/// Color for a category score (-2..=2): red for distracting, green for productive.
pub fn score_style(score: i8) -> Style {
    match score {
        2 => color(AnsiColor::Green),
        1 => color(AnsiColor::Cyan),
        0 => color(AnsiColor::Blue),
        -1 => color(AnsiColor::Yellow),
        _ => color(AnsiColor::Red),
    }
}

/// Color for a 0-100 productivity value.
pub fn productivity_style(value: u8) -> Style {
    match value {
        67.. => color(AnsiColor::Green),
        34..=66 => color(AnsiColor::Yellow),
        _ => color(AnsiColor::Red),
    }
}

/// Wraps `text` in a style.
pub fn paint(style: Style, text: &str) -> String {
    format!("{style}{text}{style:#}")
}

/// A horizontal bar `width` cells wide filled to `fraction` (0.0-1.0),
/// with eighth-of-a-cell precision. The unfilled part is padded with spaces.
pub fn bar(fraction: f64, width: usize) -> String {
    const PARTIAL: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    let eighths = (fraction.clamp(0.0, 1.0) * (width * 8) as f64).round() as usize;
    let (full, rest) = (eighths / 8, eighths % 8);
    let mut out = "█".repeat(full);
    if rest > 0 {
        out.push(PARTIAL[rest]);
    }
    let used = out.chars().count();
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

/// One block character whose height shows `fraction` (0.0-1.0).
pub fn spark(fraction: f64) -> char {
    const LEVELS: [char; 9] = ['·', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if fraction <= 0.0 {
        return LEVELS[0];
    }
    // Any activity at all shows at least the lowest block.
    let level = (fraction.clamp(0.0, 1.0) * 8.0).ceil() as usize;
    LEVELS[level.clamp(1, 8)]
}

/// Pads or truncates `text` to exactly `width` characters.
pub fn fit(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return format!("{text}{}", " ".repeat(width - count));
    }
    let mut short: String = text.chars().take(width.saturating_sub(1)).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_widths() {
        assert_eq!(bar(0.0, 4), "    ");
        assert_eq!(bar(1.0, 4), "████");
        assert_eq!(bar(0.5, 4), "██  ");
        assert_eq!(bar(0.125, 1), "▏");
        assert_eq!(bar(7.0, 2), "██");
        assert!((0..=100).all(|i| bar(i as f64 / 100.0, 10).chars().count() == 10));
    }

    #[test]
    fn spark_levels() {
        assert_eq!(spark(0.0), '·');
        assert_eq!(spark(0.01), '▁');
        assert_eq!(spark(0.5), '▄');
        assert_eq!(spark(1.0), '█');
        assert_eq!(spark(3.0), '█');
    }

    #[test]
    fn fit_pads_and_truncates() {
        assert_eq!(fit("abc", 5), "abc  ");
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("añó", 3), "añó");
    }
}
