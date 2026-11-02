//! Writing to the terminal. Results go to standard output, diagnostics to standard error.

use std::io::{self, Write};

use crate::domain::i18n::{Locale, cli as t};

/// Prints one line of result to standard output. A closed pipe is not an error.
pub(crate) fn line(text: &str) {
    let _ = writeln!(io::stdout().lock(), "{text}");
}

/// Prints one diagnostic line to standard error.
pub(crate) fn error_line(text: &str) {
    let _ = writeln!(io::stderr().lock(), "error: {text}");
}

/// Prints a warning to standard error.
pub(crate) fn warn_line(text: &str) {
    let _ = writeln!(io::stderr().lock(), "warning: {text}");
}

/// Writes `value` as pretty JSON to standard output and nothing else.
pub(crate) fn json(value: &serde_json::Value) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    line(&text);
    Ok(())
}

/// Whether an answer to a yes/no question agrees. `y` and `yes` always do; the language adds the
/// words people of that language actually type.
pub(crate) fn is_yes(locale: Locale, answer: &str) -> bool {
    let answer = answer.trim().to_lowercase();
    ["y yes", t::yes_answers(locale)]
        .iter()
        .flat_map(|words| words.split_whitespace())
        .any(|word| word == answer)
}

/// Replaces the home directory prefix with `~` so diagnostics do not leak the user name.
pub(crate) fn tilde(path: &std::path::Path, home: Option<&std::path::Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn yes_is_recognised_in_every_language() {
        for locale in Locale::ALL {
            for answer in ["y", "Y", "yes", " YES\n"] {
                assert!(is_yes(locale, answer), "{locale:?} {answer:?}");
            }
            for answer in ["", "n", "no", "maybe", "ye"] {
                assert!(!is_yes(locale, answer), "{locale:?} {answer:?}");
            }
        }
    }

    #[test]
    fn home_is_abbreviated() {
        let home = Some(Path::new("/home/ana"));
        assert_eq!(tilde(Path::new("/home/ana/dev/x"), home), "~/dev/x");
        assert_eq!(tilde(Path::new("/home/ana"), home), "~");
        assert_eq!(tilde(Path::new("/srv/x"), home), "/srv/x");
        assert_eq!(tilde(Path::new("/home/anabel/x"), home), "/home/anabel/x");
        assert_eq!(tilde(Path::new("/home/ana/x"), None), "/home/ana/x");
    }
}
