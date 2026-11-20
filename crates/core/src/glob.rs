/// Case-insensitive wildcard match. `*` matches any run of characters
/// (including none); everything else must match literally.
pub fn matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    // Position of the last `*` seen and the text position it was tried at,
    // so we can backtrack and let it swallow one more character.
    let mut star: Option<(usize, usize)> = None;

    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some((star_pi, star_ti)) = star {
            pi = star_pi + 1;
            ti = star_ti + 1;
            star = Some((star_pi, star_ti + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn literal_is_exact_and_case_insensitive() {
        assert!(matches("KeePassXC.exe", "keepassxc.exe"));
        assert!(!matches("keepass", "keepassxc.exe"));
        assert!(!matches("keepassxc.exe", "keepass"));
    }

    #[test]
    fn star_matches_any_run() {
        assert!(matches("*banco*", "Home Banking - Banco Galicia"));
        assert!(matches("*banco*", "banco"));
        assert!(matches("keepass*", "KeePassXC.exe"));
        assert!(matches("* - Private Browsing", "Example - Private Browsing"));
        assert!(!matches("*banco*", "Inbox - Gmail"));
        assert!(!matches("*.exe", "chrome.exe.bak"));
    }

    #[test]
    fn star_backtracks() {
        assert!(matches("a*b*c", "a-b-b-c"));
        assert!(matches("*ab", "aab"));
        assert!(!matches("a*b*c", "a-c-b"));
    }

    #[test]
    fn empty_cases() {
        assert!(matches("", ""));
        assert!(matches("*", ""));
        assert!(matches("*", "anything"));
        assert!(!matches("", "x"));
    }

    #[test]
    fn non_ascii() {
        assert!(matches("*navegación privada*", "Sitio — Navegación Privada"));
    }
}
