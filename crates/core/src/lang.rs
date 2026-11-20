use std::path::{Path, PathBuf};

/// Language of user-facing messages. English unless the system is in Spanish.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    En,
    Es,
}

/// File holding the language chosen in the app, next to the database.
fn preference_file() -> Option<PathBuf> {
    crate::Paths::resolve().ok().map(|paths| paths.data_dir.join("language"))
}

fn read_preference(path: &Path) -> Option<Lang> {
    match std::fs::read_to_string(path).ok()?.trim() {
        "en" => Some(Lang::En),
        "es" => Some(Lang::Es),
        _ => None,
    }
}

fn write_preference(path: &Path, lang: Option<Lang>) -> std::io::Result<()> {
    match lang {
        Some(lang) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, lang.code())
        }
        None => match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    }
}

impl Lang {
    /// The language to use, in order of priority: the `ZMAN_LANG`
    /// environment variable (`es`, `en`), the language chosen in the app,
    /// and finally the operating system's.
    pub fn detect() -> Lang {
        if let Ok(tag) = std::env::var("ZMAN_LANG") {
            return Lang::from_tag(&tag);
        }
        Lang::saved().unwrap_or_else(|| sys_locale::get_locale().map_or(Lang::En, |tag| Lang::from_tag(&tag)))
    }

    /// The language the user picked explicitly, if any.
    pub fn saved() -> Option<Lang> {
        read_preference(&preference_file()?)
    }

    /// Remembers the user's choice for every zman program; `None` goes back
    /// to following the system.
    pub fn save(lang: Option<Lang>) -> std::io::Result<()> {
        match preference_file() {
            Some(path) => write_preference(&path, lang),
            None => Ok(()),
        }
    }

    /// "en" or "es".
    pub fn code(self) -> &'static str {
        self.pick("en", "es")
    }

    /// Maps a locale tag such as `es-AR` or `es_ES.UTF-8` to a language.
    pub fn from_tag(tag: &str) -> Lang {
        let primary = tag.split(['-', '_', '.']).next().unwrap_or("");
        if primary.eq_ignore_ascii_case("es") { Lang::Es } else { Lang::En }
    }

    /// Picks the text for this language.
    pub fn pick<'a>(self, en: &'a str, es: &'a str) -> &'a str {
        match self {
            Lang::En => en,
            Lang::Es => es,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Lang, read_preference, write_preference};

    #[test]
    fn preference_round_trip() {
        let dir = std::env::temp_dir().join(format!("zman-lang-test-{}", std::process::id()));
        let path = dir.join("nested").join("language");
        assert_eq!(read_preference(&path), None);
        write_preference(&path, Some(Lang::Es)).unwrap();
        assert_eq!(read_preference(&path), Some(Lang::Es));
        write_preference(&path, Some(Lang::En)).unwrap();
        assert_eq!(read_preference(&path), Some(Lang::En));
        write_preference(&path, None).unwrap();
        assert_eq!(read_preference(&path), None);
        // Clearing twice, or reading garbage, is harmless.
        write_preference(&path, None).unwrap();
        std::fs::write(&path, "klingon").unwrap();
        assert_eq!(read_preference(&path), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn spanish_locales() {
        for tag in ["es", "es-AR", "es_ES.UTF-8", "ES-mx"] {
            assert_eq!(Lang::from_tag(tag), Lang::Es, "{tag}");
        }
    }

    #[test]
    fn everything_else_is_english() {
        for tag in ["en-US", "pt-BR", "et-EE", "", "C"] {
            assert_eq!(Lang::from_tag(tag), Lang::En, "{tag}");
        }
    }

    #[test]
    fn pick_returns_matching_text() {
        assert_eq!(Lang::En.pick("hello", "hola"), "hello");
        assert_eq!(Lang::Es.pick("hello", "hola"), "hola");
    }
}
