use serde::Deserialize;

use crate::error::{Error, Result};
use crate::event::Event;
use crate::glob;
use crate::lang::Lang;

/// One `[[categories]]` entry of the config file.
///
/// A rule matches an event when *every* kind of pattern it defines matches:
/// with only `apps`, the app must match one of them; with `apps` and
/// `titles`, both the app and the title must match. To express "this OR
/// that", write two rules with the same `name`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryRule {
    pub name: String,
    /// Productivity score from -2 (very distracting) to 2 (very productive).
    #[serde(default)]
    pub score: i8,
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub titles: Vec<String>,
    /// Matched against the site of browser events, e.g. `github.com`
    /// (subdomains included).
    #[serde(default)]
    pub domains: Vec<String>,
}

impl CategoryRule {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::Config("a category has an empty name".into()));
        }
        if !(-2..=2).contains(&self.score) {
            return Err(Error::Config(format!(
                "category \"{}\": score must be between -2 and 2",
                self.name
            )));
        }
        if self.apps.is_empty() && self.titles.is_empty() && self.domains.is_empty() {
            return Err(Error::Config(format!(
                "category \"{}\" needs at least one of apps, titles or domains",
                self.name
            )));
        }
        Ok(())
    }

    fn matches(&self, event: &Event) -> bool {
        let any = |patterns: &[String], text: &str| patterns.iter().any(|p| glob::matches(p, text));
        if !self.apps.is_empty() && !any(&self.apps, &event.app) {
            return false;
        }
        if !self.titles.is_empty() && !any(&self.titles, &event.title) {
            return false;
        }
        if !self.domains.is_empty() {
            let Some(domain) = event.domain() else {
                return false;
            };
            if !self.domains.iter().any(|d| domain_matches(d, &domain)) {
                return false;
            }
        }
        true
    }
}

/// `github.com` matches `github.com` and `gist.github.com`, not `notgithub.com`.
fn domain_matches(pattern: &str, domain: &str) -> bool {
    let pattern = pattern.trim().trim_start_matches("*.").to_lowercase();
    let domain = domain.to_lowercase();
    domain == pattern || domain.ends_with(&format!(".{pattern}"))
}

/// An ordered list of rules; the first one that matches wins.
#[derive(Debug, Clone, PartialEq)]
pub struct Categories {
    rules: Vec<CategoryRule>,
    uncategorized: String,
}

impl Categories {
    pub fn new(rules: Vec<CategoryRule>, lang: Lang) -> Categories {
        Categories {
            rules,
            uncategorized: lang.pick("Uncategorized", "Sin categoría").to_string(),
        }
    }

    /// Category name and productivity score for an event. Events no rule
    /// matches are "Uncategorized" with a neutral score.
    pub fn classify(&self, event: &Event) -> (&str, i8) {
        self.rules
            .iter()
            .find(|rule| rule.matches(event))
            .map_or((self.uncategorized.as_str(), 0), |rule| (rule.name.as_str(), rule.score))
    }

    /// Built-in rules used when the config defines no categories.
    pub fn defaults(lang: Lang) -> Categories {
        let dev = lang.pick("Development", "Desarrollo");
        let work = lang.pick("Work", "Trabajo");
        let comm = lang.pick("Communication", "Comunicación");
        let fun = lang.pick("Entertainment", "Entretenimiento");
        let browsing = lang.pick("Browsing", "Navegación");
        let system = lang.pick("System", "Sistema");

        const BROWSERS: &[&str] = &[
            "chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "vivaldi.exe",
            "arc.exe", "zen.exe",
        ];
        let rule = |name: &str, score: i8, apps: &[&str], titles: &[&str], domains: &[&str]| {
            let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            CategoryRule {
                name: name.to_string(),
                score,
                apps: owned(apps),
                titles: owned(titles),
                domains: owned(domains),
            }
        };

        let rules = vec![
            // Sites by domain: only apply once the browser extension reports URLs.
            rule(fun, -2, &[], &[], &[
                "youtube.com", "netflix.com", "twitch.tv", "tiktok.com", "instagram.com",
                "facebook.com", "reddit.com", "x.com", "twitter.com", "primevideo.com",
                "disneyplus.com", "9gag.com", "pinterest.com",
            ]),
            rule(comm, 0, &[], &[], &[
                "mail.google.com", "outlook.live.com", "outlook.office.com", "web.whatsapp.com",
                "slack.com", "web.telegram.org", "discord.com", "meet.google.com", "zoom.us",
                "teams.microsoft.com", "linkedin.com",
            ]),
            rule(dev, 2, &[], &[], &[
                "github.com", "gitlab.com", "bitbucket.org", "stackoverflow.com", "docs.rs",
                "crates.io", "developer.mozilla.org", "localhost", "127.0.0.1", "npmjs.com",
                "vercel.com", "cloudflare.com",
            ]),
            rule(work, 2, &[], &[], &[
                "docs.google.com", "drive.google.com", "notion.so", "figma.com", "atlassian.net",
                "trello.com", "linear.app", "claude.ai", "chatgpt.com", "calendar.google.com",
            ]),
            // Sites by window title, for browsers without the extension.
            rule(fun, -2, BROWSERS, &[
                "*youtube*", "*netflix*", "*twitch*", "*tiktok*", "*instagram*", "*facebook*",
                "*reddit*", "* / x*", "*twitter*", "*prime video*", "*disney+*", "*pinterest*",
            ], &[]),
            rule(comm, 0, BROWSERS, &[
                "*gmail*", "*outlook*", "*whatsapp*", "*slack*", "*telegram*", "*discord*",
                "*google meet*", "*zoom*", "*microsoft teams*", "*linkedin*",
            ], &[]),
            rule(dev, 2, BROWSERS, &[
                "*github*", "*gitlab*", "*bitbucket*", "*stack overflow*", "*docs.rs*",
                "*crates.io*", "*mdn web docs*", "*localhost*", "*127.0.0.1*",
            ], &[]),
            rule(work, 2, BROWSERS, &[
                "*google docs*", "*google sheets*", "*google slides*", "*google drive*",
                "*notion*", "*figma*", "*jira*", "*confluence*", "*trello*", "*linear*",
                "*claude*", "*chatgpt*", "*google calendar*",
            ], &[]),
            // Desktop apps.
            rule(dev, 2, &[
                "code.exe", "cursor.exe", "devenv.exe", "idea64.exe", "pycharm64.exe",
                "webstorm64.exe", "rustrover64.exe", "phpstorm64.exe", "goland64.exe",
                "clion64.exe", "datagrip64.exe", "zed.exe", "sublime_text.exe", "notepad++.exe",
                "windowsterminal.exe", "powershell.exe", "pwsh.exe", "cmd.exe", "wezterm-gui.exe",
                "alacritty.exe", "mintty.exe", "gitkraken.exe", "githubdesktop.exe",
                "docker desktop.exe", "postman.exe", "insomnia.exe", "dbeaver.exe",
            ], &[], &[]),
            rule(comm, 0, &[
                "slack.exe", "teams.exe", "ms-teams.exe", "outlook.exe", "olk.exe", "discord.exe",
                "telegram.exe", "whatsapp.exe", "whatsapp.root.exe", "zoom.exe", "thunderbird.exe",
                "signal.exe", "skype.exe",
            ], &[], &[]),
            rule(work, 2, &[
                "winword.exe", "excel.exe", "powerpnt.exe", "onenote.exe", "notion.exe",
                "obsidian.exe", "figma.exe", "acrobat.exe", "acrord32.exe", "sumatrapdf.exe",
                "claude.exe", "chatgpt.exe", "marktext.exe", "photoshop.exe", "illustrator.exe",
            ], &[], &[]),
            rule(fun, -2, &[
                "spotify.exe", "steam.exe", "steamwebhelper.exe", "vlc.exe", "netflix.exe",
                "epicgameslauncher.exe", "wmplayer.exe",
            ], &[], &[]),
            rule(browsing, 0, BROWSERS, &[], &[]),
            rule(system, 0, &[
                "explorer.exe", "searchhost.exe", "shellexperiencehost.exe",
                "startmenuexperiencehost.exe", "applicationframehost.exe", "lockapp.exe",
                "taskmgr.exe", "systemsettings.exe", "snippingtool.exe",
            ], &[], &[]),
        ];
        Categories::new(rules, lang)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(app: &str, title: &str, url: Option<&str>) -> Event {
        Event {
            id: "E".into(),
            device_id: "dev".into(),
            start_ms: 0,
            duration_ms: 1000,
            app: app.into(),
            title: title.into(),
            data: match url {
                Some(url) => serde_json::json!({ "url": url }),
                None => serde_json::json!({}),
            },
            updated_at_ms: 0,
        }
    }

    fn rule(name: &str, score: i8, apps: &[&str], titles: &[&str], domains: &[&str]) -> CategoryRule {
        let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        CategoryRule {
            name: name.into(),
            score,
            apps: owned(apps),
            titles: owned(titles),
            domains: owned(domains),
        }
    }

    #[test]
    fn first_matching_rule_wins() {
        let cats = Categories::new(
            vec![
                rule("Video", -2, &["chrome.exe"], &["*youtube*"], &[]),
                rule("Browsing", 0, &["chrome.exe"], &[], &[]),
            ],
            Lang::En,
        );
        assert_eq!(cats.classify(&event("chrome.exe", "Cats - YouTube", None)), ("Video", -2));
        assert_eq!(cats.classify(&event("chrome.exe", "Rust docs", None)), ("Browsing", 0));
    }

    #[test]
    fn app_and_title_must_both_match() {
        let cats = Categories::new(vec![rule("Video", -2, &["chrome.exe"], &["*youtube*"], &[])], Lang::En);
        // A file called youtube.rs in the editor is not a video.
        assert_eq!(cats.classify(&event("code.exe", "youtube.rs", None)), ("Uncategorized", 0));
    }

    #[test]
    fn unmatched_is_uncategorized_in_the_chosen_language() {
        let cats = Categories::new(vec![], Lang::Es);
        assert_eq!(cats.classify(&event("x.exe", "y", None)), ("Sin categoría", 0));
    }

    #[test]
    fn domains_match_subdomains_only() {
        let cats = Categories::new(vec![rule("Dev", 2, &[], &[], &["github.com"])], Lang::En);
        let classify = |url| cats.classify(&event("chrome.exe", "t", Some(url))).0.to_string();
        assert_eq!(classify("https://github.com/MalkSof/zman"), "Dev");
        assert_eq!(classify("https://gist.github.com/x"), "Dev");
        assert_eq!(classify("https://notgithub.com/"), "Uncategorized");
        // No URL at all: a domain rule cannot match.
        assert_eq!(cats.classify(&event("chrome.exe", "GitHub", None)).0, "Uncategorized");
    }

    #[test]
    fn defaults_cover_common_cases() {
        let cats = Categories::defaults(Lang::En);
        assert_eq!(cats.classify(&event("Code.exe", "main.rs", None)), ("Development", 2));
        assert_eq!(cats.classify(&event("chrome.exe", "Cats - YouTube - Google Chrome", None)).0, "Entertainment");
        assert_eq!(cats.classify(&event("chrome.exe", "Inbox - Gmail - Google Chrome", None)).0, "Communication");
        assert_eq!(cats.classify(&event("chrome.exe", "Some blog - Google Chrome", None)).0, "Browsing");
        assert_eq!(cats.classify(&event("slack.exe", "general", None)).0, "Communication");
        // The URL wins over a misleading title.
        let e = event("chrome.exe", "How to grow on YouTube - Google Chrome", Some("https://github.com/a/b"));
        assert_eq!(cats.classify(&e).0, "Development");
    }

    #[test]
    fn validation() {
        assert!(rule("Ok", 2, &["a"], &[], &[]).validate().is_ok());
        assert!(rule("", 0, &["a"], &[], &[]).validate().is_err());
        assert!(rule("Bad", 3, &["a"], &[], &[]).validate().is_err());
        assert!(rule("Empty", 0, &[], &[], &[]).validate().is_err());
    }
}
