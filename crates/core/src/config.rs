use std::path::Path;

use serde::Deserialize;

use crate::category::{Categories, CategoryRule, domain_matches};
use crate::event::domain_of;
use crate::error::{Error, Result};
use crate::glob;
use crate::lang::Lang;

/// User configuration, read from `config.toml`. Every field is optional in
/// the file; missing ones take the defaults below.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// How often the daemon samples the active window.
    pub poll_interval_secs: u64,
    /// Seconds without keyboard/mouse input before the user counts as away.
    pub afk_threshold_secs: u64,
    pub exclude: Exclude,
    pub browser: Browser,
    /// Rules assigning apps and titles to categories, in priority order.
    /// When empty, built-in rules are used.
    pub categories: Vec<CategoryRule>,
}

/// Windows that must never be recorded. Patterns are case-insensitive and
/// support `*` as a wildcard.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Exclude {
    /// Matched against the app name (e.g. `keepassxc.exe`, `*vpn*`).
    pub apps: Vec<String>,
    /// Matched against the window title (e.g. `*banco*`).
    pub titles: Vec<String>,
    /// Sites never recorded, subdomains included (e.g. `mybank.com`).
    pub domains: Vec<String>,
}

/// Settings for the browser extension, which reports the active tab to a
/// local port so events can carry the page URL.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Browser {
    /// Local port the extension talks to. 0 turns the feature off.
    pub port: u16,
    /// Store the page path (`https://github.com/user/repo`). When false only
    /// the site is kept (`https://github.com/`). Query strings are never stored.
    pub full_urls: bool,
}

impl Default for Browser {
    fn default() -> Self {
        Browser { port: 27184, full_urls: true }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            poll_interval_secs: 1,
            afk_threshold_secs: 180,
            exclude: Exclude::default(),
            browser: Browser::default(),
            categories: Vec::new(),
        }
    }
}

impl Default for Exclude {
    fn default() -> Self {
        Exclude {
            apps: Vec::new(),
            // Private browsing windows of Edge and Firefox (en/es).
            titles: vec![
                "*InPrivate*".into(),
                "*Private Browsing*".into(),
                "*Navegación privada*".into(),
            ],
            domains: Vec::new(),
        }
    }
}

impl Config {
    /// Loads the config file, or returns the defaults if it does not exist.
    pub fn load(path: &Path) -> Result<Config> {
        match std::fs::read_to_string(path) {
            Ok(text) => Config::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn parse(text: &str) -> Result<Config> {
        let config: Config = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        if config.poll_interval_secs == 0 {
            return Err(Error::Config("poll_interval_secs must be at least 1".into()));
        }
        if config.afk_threshold_secs == 0 {
            return Err(Error::Config("afk_threshold_secs must be at least 1".into()));
        }
        for rule in &config.categories {
            rule.validate()?;
        }
        Ok(config)
    }

    /// The category rules to apply: the user's, or the built-in defaults.
    pub fn categories(&self, lang: Lang) -> Categories {
        if self.categories.is_empty() {
            Categories::defaults(lang)
        } else {
            Categories::new(self.categories.clone(), lang)
        }
    }

    pub fn poll_interval_ms(&self) -> i64 {
        self.poll_interval_secs as i64 * 1000
    }

    pub fn afk_threshold_ms(&self) -> i64 {
        self.afk_threshold_secs as i64 * 1000
    }

    /// Longest silence between two samples that still counts as continuous
    /// use. Anything longer (daemon stopped, machine suspended) breaks the
    /// event instead of being filled in.
    pub fn max_gap_ms(&self) -> i64 {
        (self.poll_interval_ms() * 3).max(5000)
    }

    pub fn is_excluded(&self, app: &str, title: &str, url: Option<&str>) -> bool {
        let domain = url.and_then(domain_of);
        self.exclude.apps.iter().any(|p| glob::matches(p, app))
            || self.exclude.titles.iter().any(|p| glob::matches(p, title))
            || domain.is_some_and(|d| self.exclude.domains.iter().any(|p| domain_matches(p, &d)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let config = Config::parse(
            r#"
            afk_threshold_secs = 60

            [exclude]
            apps = ["KeePassXC.exe"]
            "#,
        )
        .unwrap();
        assert_eq!(config.afk_threshold_secs, 60);
        assert_eq!(config.poll_interval_secs, 1);
        assert_eq!(config.exclude.apps, vec!["KeePassXC.exe"]);
        // Not mentioned in the file, so the default title patterns remain.
        assert_eq!(config.exclude.titles, Exclude::default().titles);
    }

    #[test]
    fn unknown_keys_and_zero_values_are_rejected() {
        assert!(Config::parse("afk_treshold_secs = 60").is_err());
        assert!(Config::parse("poll_interval_secs = 0").is_err());
        assert!(Config::parse("afk_threshold_secs = 0").is_err());
    }

    #[test]
    fn categories_from_file_replace_the_defaults() {
        let config = Config::parse(
            r#"
            [[categories]]
            name = "Deep work"
            score = 2
            apps = ["code.exe"]

            [[categories]]
            name = "Video"
            score = -2
            apps = ["chrome.exe"]
            titles = ["*youtube*"]
            "#,
        )
        .unwrap();
        assert_eq!(config.categories.len(), 2);
        assert_eq!(config.categories[1].titles, vec!["*youtube*"]);
        assert_ne!(config.categories(Lang::En), Categories::defaults(Lang::En));
        assert_eq!(Config::default().categories(Lang::En), Categories::defaults(Lang::En));
    }

    #[test]
    fn invalid_categories_are_rejected() {
        assert!(Config::parse("[[categories]]
name = \"X\"
score = 5
apps = [\"a\"]").is_err());
        assert!(Config::parse("[[categories]]
name = \"X\"").is_err());
    }

    #[test]
    fn exclusion_by_app_or_title() {
        let config = Config::parse(
            r#"
            [exclude]
            apps = ["keepass*"]
            titles = ["*banco*"]
            "#,
        )
        .unwrap();
        assert!(config.is_excluded("KeePassXC.exe", "Passwords", None));
        assert!(config.is_excluded("chrome.exe", "Mi Banco - Google Chrome", None));
        assert!(!config.is_excluded("chrome.exe", "Rust docs - Google Chrome", None));
    }

    #[test]
    fn exclusion_by_domain_and_browser_settings() {
        let config = Config::parse(
            r#"
            [exclude]
            domains = ["mybank.com"]

            [browser]
            port = 0
            full_urls = false
            "#,
        )
        .unwrap();
        assert!(config.is_excluded("chrome.exe", "Accounts", Some("https://www.mybank.com/home")));
        assert!(config.is_excluded("chrome.exe", "Accounts", Some("https://online.mybank.com/")));
        assert!(!config.is_excluded("chrome.exe", "Accounts", Some("https://notmybank.com/")));
        assert!(!config.is_excluded("chrome.exe", "Accounts", None));
        assert_eq!(config.browser, Browser { port: 0, full_urls: false });
        assert_eq!(Config::default().browser.port, 27184);
    }
}
