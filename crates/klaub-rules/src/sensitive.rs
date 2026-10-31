//! Recognising sensitive areas by name.
//!
//! A sensitive name raises *attention*, never certainty: the keyword list is intentionally a
//! small set of whole words, matched on identifier parts (`validateToken` has the parts
//! `validate` and `token`), so `authority` does not match `auth` (ADR 0007).

const SENSITIVE_WORDS: &[&str] = &[
    "auth",
    "authn",
    "authz",
    "authenticate",
    "authorize",
    "login",
    "logout",
    "session",
    "sessions",
    "password",
    "passwd",
    "credential",
    "credentials",
    "token",
    "tokens",
    "jwt",
    "oauth",
    "permission",
    "permissions",
    "acl",
    "rbac",
    "role",
    "roles",
    "payment",
    "payments",
    "billing",
    "charge",
    "invoice",
    "checkout",
    "crypto",
    "encrypt",
    "decrypt",
    "secret",
    "secrets",
    "migration",
    "migrations",
    "schema",
    "transaction",
    "transactions",
    "admin",
    "privilege",
    "privileges",
];

/// Splits `text` into lower-case words on non-alphanumerics and camel-case boundaries.
pub(crate) fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for c in text.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            previous = None;
            continue;
        }
        let boundary =
            previous.is_some_and(|p| (p.is_lowercase() || p.is_ascii_digit()) && c.is_uppercase());
        if boundary && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current.extend(c.to_lowercase());
        previous = Some(c);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The first sensitive word found in any of `texts`, if there is one.
pub(crate) fn sensitive_word<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<String> {
    texts
        .into_iter()
        .flat_map(words)
        .find(|w| SENSITIVE_WORDS.contains(&w.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_identifiers_into_words() {
        assert_eq!(words("validateToken"), vec!["validate", "token"]);
        assert_eq!(
            words("src/auth/session_store.py"),
            vec!["src", "auth", "session", "store", "py"]
        );
        assert_eq!(
            words("HTTPServer"),
            vec!["httpserver"],
            "acronyms stay together"
        );
        assert_eq!(words("get2Factor"), vec!["get2", "factor"]);
    }

    #[test]
    fn only_whole_words_count() {
        assert_eq!(
            sensitive_word(["src/auth/views.py"]).as_deref(),
            Some("auth")
        );
        assert_eq!(
            sensitive_word(["checkPassword"]).as_deref(),
            Some("password")
        );
        assert_eq!(
            sensitive_word(["authority", "tokenizer", "authorsList"]),
            None
        );
        assert_eq!(
            sensitive_word(["db/migrations/0001.sql"]).as_deref(),
            Some("migrations")
        );
    }
}
