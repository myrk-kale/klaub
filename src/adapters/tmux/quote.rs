//! Quoting for the two places a command string crosses a parser: `/bin/sh` and tmux formats.

/// Quotes `value` as one POSIX shell word.
pub(crate) fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '=' | ':' | ',')
        })
    {
        return value.to_owned();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Joins words into one shell command line.
pub(crate) fn shell_join<'a>(words: impl IntoIterator<Item = &'a str>) -> String {
    words
        .into_iter()
        .map(shell_quote)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_words_stay_plain() {
        assert_eq!(shell_quote("/usr/bin/klaub"), "/usr/bin/klaub");
        assert_eq!(shell_quote("ses_abc123"), "ses_abc123");
    }

    #[test]
    fn hostile_words_are_neutralised() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("$(rm -rf ~)"), "'$(rm -rf ~)'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_join(["a", "b c"]), "a 'b c'");
    }
}
