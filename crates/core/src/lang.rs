/// Language of user-facing messages. English unless the system is in Spanish.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    En,
    Es,
}

impl Lang {
    /// Uses the `ZMAN_LANG` environment variable if set (e.g. `es`, `en`),
    /// otherwise the operating system's locale.
    pub fn detect() -> Lang {
        let tag = std::env::var("ZMAN_LANG").ok().or_else(sys_locale::get_locale);
        tag.map_or(Lang::En, |tag| Lang::from_tag(&tag))
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
    use super::Lang;

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
