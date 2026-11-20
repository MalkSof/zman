use serde::{Deserialize, Serialize};

/// A span of time during which one window was active.
///
/// Events are append-only. The only mutation ever applied to a stored event
/// is changing `duration_ms` (heartbeat extension or AFK trim), and every
/// such change bumps `updated_at_ms` so a future sync can pick it up with a
/// cursor on that column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// ULID: globally unique and sortable by creation time.
    pub id: String,
    /// The installation that recorded the event.
    pub device_id: String,
    /// Start, in milliseconds since the Unix epoch (UTC).
    pub start_ms: i64,
    pub duration_ms: i64,
    /// Application name as reported by the platform (e.g. `chrome.exe`).
    pub app: String,
    /// Window title.
    pub title: String,
    /// Free-form extras (a JSON object). Browser events carry `url` here.
    pub data: serde_json::Value,
    pub updated_at_ms: i64,
}

impl Event {
    pub fn end_ms(&self) -> i64 {
        self.start_ms + self.duration_ms
    }

    /// The page URL, for events enriched by the browser extension.
    pub fn url(&self) -> Option<&str> {
        self.data.get("url")?.as_str()
    }

    /// Host of [`Event::url`], lowercased and without `www.`.
    pub fn domain(&self) -> Option<String> {
        domain_of(self.url()?)
    }
}

/// Extracts the host from a URL: `https://www.GitHub.com:443/x` -> `github.com`.
pub fn domain_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let host = host.split(':').next()?.trim().to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host.is_empty() { None } else { Some(host.to_string()) }
}

/// Reduces a page URL to what zman is willing to store: http(s) only, no
/// credentials, query string or fragment; and only the site when
/// `full` is false. Returns `None` for anything else (`chrome://`, files...).
pub fn clean_url(url: &str, full: bool) -> Option<String> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let rest = rest.split(['?', '#']).next()?;
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, path),
        None => (rest, ""),
    };
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host).to_lowercase();
    if host.is_empty() {
        return None;
    }
    let path = if full { path } else { "" };
    Some(format!("{scheme}://{host}/{path}"))
}

/// App name for display: `chrome.exe` -> `chrome`.
pub fn display_app(app: &str) -> &str {
    let lower_ends = |suffix: &str| {
        app.len() > suffix.len() && app[app.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    };
    if app.is_char_boundary(app.len().saturating_sub(4)) && lower_ends(".exe") {
        &app[..app.len() - 4]
    } else {
        app
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_extraction() {
        assert_eq!(domain_of("https://www.GitHub.com:443/a/b?q=1").as_deref(), Some("github.com"));
        assert_eq!(domain_of("http://localhost:3000/").as_deref(), Some("localhost"));
        assert_eq!(domain_of("https://user:pw@mail.google.com/mail").as_deref(), Some("mail.google.com"));
        assert_eq!(domain_of("docs.rs/serde").as_deref(), Some("docs.rs"));
        assert_eq!(domain_of("https:///nohost"), None);
        assert_eq!(domain_of(""), None);
    }

    #[test]
    fn url_cleaning() {
        let url = "https://user:secret@GitHub.com/MalkSof/zman?tab=readme&token=abc#top";
        assert_eq!(clean_url(url, true).as_deref(), Some("https://github.com/MalkSof/zman"));
        assert_eq!(clean_url(url, false).as_deref(), Some("https://github.com/"));
        assert_eq!(clean_url("http://localhost:3000", true).as_deref(), Some("http://localhost:3000/"));
        assert_eq!(clean_url("chrome://settings/passwords", true), None);
        assert_eq!(clean_url("file:///C:/secret.pdf", true), None);
        assert_eq!(clean_url("about:blank", true), None);
        assert_eq!(clean_url("https:///x", true), None);
    }

    #[test]
    fn display_names() {
        assert_eq!(display_app("chrome.exe"), "chrome");
        assert_eq!(display_app("Code.EXE"), "Code");
        assert_eq!(display_app("Finder"), "Finder");
        assert_eq!(display_app(".exe"), ".exe");
        assert_eq!(display_app("añó"), "añó");
    }
}
