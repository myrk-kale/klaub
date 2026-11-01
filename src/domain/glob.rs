//! A small glob matcher for repository paths.
//!
//! Supports `*` (anything within one path component), `**` (any number of components,
//! including none), and `?` (one character within a component). Matching is case-sensitive and
//! always against the whole path. It is used for the test-file patterns of the configuration.

/// Whether `path` (`/`-separated, relative) matches `pattern`.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    match_components(&pattern, &path)
}

fn match_components(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| match_components(rest, &path[skip..])),
        Some((component, rest)) => path.split_first().is_some_and(|(head, tail)| {
            match_component(component, head) && match_components(rest, tail)
        }),
    }
}

fn match_component(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    // Classic iterative wildcard matching with backtracking to the last `*`.
    let (mut p, mut t) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) && pattern[p] != '*' {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_star_stays_within_a_component() {
        assert!(matches("tests/test_*.py", "tests/test_auth.py"));
        assert!(!matches("tests/test_*.py", "tests/unit/test_auth.py"));
        assert!(!matches("tests/test_*.py", "tests/test_auth.pyc"));
        assert!(matches("*.md", "README.md"));
        assert!(!matches("*.md", "docs/README.md"));
    }

    #[test]
    fn double_star_spans_components() {
        assert!(matches("**/test_*.py", "test_a.py"));
        assert!(matches("**/test_*.py", "a/b/c/test_a.py"));
        assert!(matches("src/**/*.rs", "src/a/b/lib.rs"));
        assert!(matches("src/**/*.rs", "src/lib.rs"));
        assert!(!matches("src/**/*.rs", "tests/lib.rs"));
        assert!(matches("**", "any/thing"));
    }

    #[test]
    fn question_mark_and_literals() {
        assert!(matches("a?c.txt", "abc.txt"));
        assert!(!matches("a?c.txt", "ac.txt"));
        assert!(matches("exact/path.rs", "exact/path.rs"));
        assert!(!matches("exact/path.rs", "exact/path.rss"));
        assert!(matches("**/*.test.ts", "web/app.test.ts"));
        assert!(!matches("**/*.test.ts", "web/app.ts"));
    }

    #[test]
    fn stars_can_match_nothing_and_backtrack() {
        assert!(matches("a*b*c", "abc"));
        assert!(matches("a*b*c", "axxbyyc"));
        assert!(!matches("a*b*c", "axxbyy"));
    }
}
