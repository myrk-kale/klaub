//! Heuristics shared by the language profiles for structural risk markers.
//!
//! These are deliberately conservative *pointers for a human*, never verdicts: each marker
//! says "look here", with a short description that never contains the suspicious value itself.

use crate::domain::symbol::Receiver;

const SECRET_WORDS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "apikey",
    "privatekey",
    "credential",
    "accesskey",
    "authkey",
];
const NOT_A_SECRET_SUFFIXES: &[&str] = &[
    "path",
    "file",
    "name",
    "url",
    "uri",
    "type",
    "field",
    "length",
    "len",
    "count",
    "ttl",
    "expiry",
    "expires",
    "header",
    "prefix",
    "suffix",
    "endpoint",
    "env",
    "var",
    "keyid",
    "label",
    "hint",
    "regex",
    "pattern",
    "mask",
    "placeholder",
];
const PLACEHOLDER_WORDS: &[&str] = &[
    "example",
    "changeme",
    "placeholder",
    "dummy",
    "xxxx",
    "your",
    "fake",
    "todo",
    "sample",
    "redacted",
];

/// Whether an identifier looks like the name of a secret (`api_key`, `dbPassword`).
pub(crate) fn secret_name(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    SECRET_WORDS.iter().any(|word| normalized.contains(word))
        && !NOT_A_SECRET_SUFFIXES
            .iter()
            .any(|suffix| normalized.ends_with(suffix))
}

/// Whether a string literal could plausibly be a real secret rather than a placeholder.
pub(crate) fn plausible_secret(value: &str) -> bool {
    if value.chars().count() < 8 || value.chars().any(char::is_whitespace) {
        return false;
    }
    if value.starts_with(['$', '{', '<', '%', '#']) || value.contains("${") || value.contains("://")
    {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    if PLACEHOLDER_WORDS.iter().any(|word| lower.contains(word)) {
        return false;
    }
    let first = value.chars().next();
    !value.chars().all(|c| Some(c) == first)
}

/// Whether a call name executes SQL text.
pub(crate) fn sql_sink(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "execute"
            | "executemany"
            | "executescript"
            | "query"
            | "queryrow"
            | "queryraw"
            | "raw"
            | "exec"
            | "querycontext"
            | "execcontext"
    ) || name == "$queryRawUnsafe"
        || name == "$executeRawUnsafe"
}

/// The last identifier of a receiver path (`self.db.cursor` -> `cursor`).
pub(crate) fn receiver_last(receiver: &Receiver) -> Option<&str> {
    match receiver {
        Receiver::Path(path) => path.last().map(String::as_str),
        Receiver::None | Receiver::SelfRef | Receiver::Complex => None,
    }
}

/// Whether a call looks like network, file or database I/O.
pub(crate) fn io_call(receiver: &Receiver, name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    let name = lowered.as_str();
    let last = receiver_last(receiver).map(str::to_ascii_lowercase);
    let last = last.as_deref();
    match last {
        Some(
            "requests" | "httpx" | "urllib" | "aiohttp" | "axios" | "http" | "https" | "got"
            | "reqwest" | "ureq" | "surf",
        ) => true,
        Some(
            "cursor" | "conn" | "connection" | "db" | "session" | "engine" | "pool" | "client"
            | "tx",
        ) => {
            matches!(
                name,
                "execute"
                    | "executemany"
                    | "query"
                    | "commit"
                    | "fetchall"
                    | "fetchone"
                    | "fetch"
                    | "get"
                    | "post"
                    | "send"
                    | "exec"
                    | "queryrow"
                    | "find"
                    | "findone"
                    | "save"
            )
        }
        Some("fs" | "promises" | "os") => {
            name.starts_with("read")
                || name.starts_with("write")
                || matches!(name, "open" | "stat" | "readdir" | "mkdir")
        }
        Some("subprocess") => matches!(
            name,
            "run" | "check_output" | "check_call" | "popen" | "call"
        ),
        Some("command") => matches!(name, "output" | "status" | "spawn"),
        None => match receiver {
            Receiver::None => matches!(
                name,
                "open"
                    | "fetch"
                    | "urlopen"
                    | "readfile"
                    | "readfilesync"
                    | "writefile"
                    | "writefilesync"
            ),
            _ => false,
        },
        Some(_) => false,
    }
}

/// Whether a call invokes a shell.
pub(crate) fn shell_call(receiver: &Receiver, name: &str) -> bool {
    let last = receiver_last(receiver);
    matches!(
        (last, name),
        (Some("os"), "system" | "popen") | (Some("child_process" | "cp"), "exec" | "execSync")
    ) || (matches!(receiver, Receiver::None) && matches!(name, "execSync"))
}

/// Whether a call evaluates text as code.
pub(crate) fn dynamic_exec_call(receiver: &Receiver, name: &str) -> bool {
    matches!(receiver, Receiver::None) && matches!(name, "eval" | "exec")
}

/// The exception name from the text a `raise` or `throw` names, or `None` when that text is not a
/// plain (possibly dotted) identifier. A raised variable, an index or a call result is not a name
/// the contract can state, so it is left out instead of being shown as one.
pub(crate) fn exception_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    let plain = !name.is_empty()
        && name.len() <= 80
        && name.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        });
    plain.then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_names() {
        for yes in [
            "password",
            "DB_PASSWORD",
            "apiKey",
            "api_key",
            "client_secret",
            "authToken",
            "private_key",
            "AWS_ACCESS_KEY",
        ] {
            assert!(secret_name(yes), "{yes}");
        }
        for no in [
            "token_type",
            "password_field",
            "secret_path",
            "name",
            "max_tokens_count",
            "tokenizer_name",
            "api_key_header",
            "keyboard",
        ] {
            assert!(!secret_name(no), "{no}");
        }
    }

    #[test]
    fn plausible_secret_values() {
        assert!(plausible_secret("hunter2hunter2"));
        assert!(plausible_secret("s3cr3t-Value-9f8e"));
        for no in [
            "",
            "short",
            "your-api-key-here",
            "${API_KEY}",
            "<token>",
            "aaaaaaaaaaaa",
            "with space inside",
            "https://example.com",
            "changeme123",
        ] {
            assert!(!plausible_secret(no), "{no:?}");
        }
    }

    #[test]
    fn sinks_and_calls() {
        assert!(sql_sink("execute") && sql_sink("Query") && !sql_sink("select"));
        assert!(io_call(&Receiver::Path(vec!["requests".into()]), "get"));
        assert!(io_call(&Receiver::None, "open"));
        assert!(!io_call(&Receiver::Path(vec!["config".into()]), "get"));
        assert!(shell_call(&Receiver::Path(vec!["os".into()]), "system"));
        assert!(dynamic_exec_call(&Receiver::None, "eval"));
        assert!(!dynamic_exec_call(
            &Receiver::Path(vec!["re".into()]),
            "exec"
        ));
    }

    #[test]
    fn only_plain_names_count_as_raised_exceptions() {
        assert_eq!(exception_name("ValueError").as_deref(), Some("ValueError"));
        assert_eq!(
            exception_name("errors.NotFound").as_deref(),
            Some("errors.NotFound")
        );
        assert_eq!(exception_name("exc").as_deref(), Some("exc"));
        for not_a_name in ["", "make_error()", "errors[0]", "a b", "x\u{1b}[31m"] {
            assert_eq!(exception_name(not_a_name), None, "{not_a_name:?}");
        }
    }
}
