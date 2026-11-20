use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::glob;

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
}

impl Default for Config {
    fn default() -> Self {
        Config {
            poll_interval_secs: 1,
            afk_threshold_secs: 180,
            exclude: Exclude::default(),
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
        Ok(config)
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

    pub fn is_excluded(&self, app: &str, title: &str) -> bool {
        self.exclude.apps.iter().any(|p| glob::matches(p, app))
            || self.exclude.titles.iter().any(|p| glob::matches(p, title))
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
    fn exclusion_by_app_or_title() {
        let config = Config::parse(
            r#"
            [exclude]
            apps = ["keepass*"]
            titles = ["*banco*"]
            "#,
        )
        .unwrap();
        assert!(config.is_excluded("KeePassXC.exe", "Passwords"));
        assert!(config.is_excluded("chrome.exe", "Mi Banco - Google Chrome"));
        assert!(!config.is_excluded("chrome.exe", "Rust docs - Google Chrome"));
    }
}
