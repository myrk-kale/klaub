//! The tmux configuration Klaub applies to its private server.
//!
//! The server starts with an empty configuration file (`-f /dev/null`), so the
//! user's own `.tmux.conf` and key bindings never apply. Everything below is
//! applied through argument arrays; commands that must run a shell command are
//! stored in tmux *user options* first, so no quoting of paths ever crosses
//! tmux's own command parser.

use std::path::Path;

use klaub_core::ids::SessionId;
use klaub_core::{Error, Result};

use crate::quote::shell_join;

/// What the configuration needs to know about the session.
#[derive(Debug, Clone)]
pub struct SessionConfig<'a> {
    /// The Klaub session id (baked into the shell commands the key bindings run).
    pub session_id: &'a SessionId,
    /// This executable; key bindings call back into it.
    pub klaub_exe: &'a Path,
    /// The prefix key in tmux notation (`C-b`).
    pub prefix: &'a str,
}

/// The help shown by `Prefix ?`.
pub const HELP_TEXT: &str = "klaub  Prefix+R review  Prefix+A agent  Prefix+I impact  Prefix+C comments  \
Prefix+T tests  Prefix+B brief  Prefix+L flow  Prefix+d detach  (Prefix is Ctrl+B unless configured)";

/// Checks a tmux key name such as `C-b`, `C-Space` or `M-a`.
pub fn validate_prefix(prefix: &str) -> Result<()> {
    let key = prefix.rsplit('-').next().unwrap_or(prefix);
    let modifiers = &prefix[..prefix.len().saturating_sub(key.len())];
    let valid_modifiers = modifiers.is_empty()
        || modifiers
            .trim_end_matches('-')
            .split('-')
            .all(|m| matches!(m, "C" | "M" | "S"));
    let valid_key = (key.len() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()))
        || matches!(key, "Space" | "Tab" | "Escape" | "Enter");
    if !prefix.is_empty() && valid_modifiers && valid_key {
        Ok(())
    } else {
        Err(Error::invalid(
            "tmux prefix",
            format!("{prefix:?} is not a plain tmux key such as C-b, C-a or C-Space"),
        ))
    }
}

fn command(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}

/// The shell command that runs `klaub <args>`.
fn klaub_shell(exe: &Path, args: &[&str]) -> String {
    let exe = exe.to_string_lossy();
    shell_join(std::iter::once(exe.as_ref()).chain(args.iter().copied()))
}

/// Builds the commands that configure a freshly started server.
pub fn configuration(spec: &SessionConfig<'_>) -> Result<Vec<Vec<String>>> {
    validate_prefix(spec.prefix)?;
    let id = spec.session_id.as_str();
    let focus =
        |target: &str| klaub_shell(spec.klaub_exe, &["session", "focus", "--id", id, target]);
    let reap = klaub_shell(spec.klaub_exe, &["session", "reap", "--id", id]);

    let mut commands = vec![
        command(&["set-option", "-g", "escape-time", "10"]),
        command(&["set-option", "-g", "focus-events", "on"]),
        command(&["set-option", "-g", "set-clipboard", "on"]),
        command(&["set-option", "-g", "mouse", "off"]),
        command(&["set-option", "-g", "history-limit", "50000"]),
        command(&["set-option", "-g", "allow-rename", "off"]),
        command(&["set-option", "-g", "automatic-rename", "off"]),
        command(&["set-option", "-g", "remain-on-exit", "failed"]),
        command(&["set-option", "-g", "status", "on"]),
        command(&[
            "set-option",
            "-g",
            "status-style",
            "bg=colour236,fg=colour252",
        ]),
        command(&["set-option", "-g", "status-left", " klaub "]),
        command(&["set-option", "-g", "status-left-length", "12"]),
        command(&[
            "set-option",
            "-g",
            "status-left-style",
            "bold,bg=colour63,fg=colour255",
        ]),
        command(&[
            "set-option",
            "-g",
            "status-right",
            " Prefix+R review  Prefix+A agent  Prefix+? help ",
        ]),
        command(&["set-option", "-g", "status-right-length", "64"]),
        command(&[
            "set-option",
            "-g",
            "window-status-current-style",
            "bold,underscore",
        ]),
        command(&["set-option", "-sa", "terminal-features", ",*:RGB"]),
    ];

    if spec.prefix != "C-b" {
        commands.push(command(&["unbind-key", "C-b"]));
        commands.push(command(&["set-option", "-g", "prefix", spec.prefix]));
        commands.push(command(&["bind-key", spec.prefix, "send-prefix"]));
    }

    // Shell commands live in user options and are expanded by `run-shell` at use time.
    commands.push(vec![
        "set-option".into(),
        "-g".into(),
        "@klaub-focus-review".into(),
        focus("review"),
    ]);
    commands.push(vec![
        "set-option".into(),
        "-g".into(),
        "@klaub-focus-agent".into(),
        focus("agent"),
    ]);
    commands.push(vec![
        "set-option".into(),
        "-g".into(),
        "@klaub-reap".into(),
        reap,
    ]);
    commands.push(command(&[
        "bind-key",
        "-T",
        "prefix",
        "R",
        "run-shell",
        "-b",
        "#{@klaub-focus-review}",
    ]));
    commands.push(command(&[
        "bind-key",
        "-T",
        "prefix",
        "A",
        "run-shell",
        "-b",
        "#{@klaub-focus-agent}",
    ]));
    commands.push(command(&[
        "bind-key",
        "-T",
        "prefix",
        "?",
        "display-message",
        "-d",
        "10000",
        HELP_TEXT,
    ]));
    // These open the reviewer on a specific mode. The review window is selected first, then the
    // function key is delivered to its pane, where the reader maps it to a mode.
    for (key, function_key) in [
        ("I", "F5"),
        ("C", "F6"),
        ("T", "F7"),
        ("B", "F8"),
        ("L", "F9"),
    ] {
        commands.push(command(&[
            "bind-key",
            "-T",
            "prefix",
            key,
            "run-shell",
            "-b",
            "#{@klaub-focus-review}",
        ]));
        commands.push(command(&[
            "bind-key",
            "-T",
            "prefix",
            key,
            "send-keys",
            "-t",
            "review",
            function_key,
        ]));
    }
    commands.push(command(&[
        "set-hook",
        "-g",
        "pane-exited",
        "run-shell -b \"#{@klaub-reap}\"",
    ]));
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn spec<'a>(id: &'a SessionId, exe: &'a Path, prefix: &'a str) -> SessionConfig<'a> {
        SessionConfig {
            session_id: id,
            klaub_exe: exe,
            prefix,
        }
    }

    #[test]
    fn prefix_validation() {
        for ok in ["C-b", "C-a", "C-Space", "M-a", "C-M-x", "a"] {
            assert!(validate_prefix(ok).is_ok(), "{ok}");
        }
        for bad in ["", "C-", "ctrl-b", "C-b; kill-server", "C-bb", "C-$(x)"] {
            assert!(validate_prefix(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn default_configuration_binds_review_and_agent() {
        let id = SessionId::parse("ses_abc123").unwrap();
        let commands = configuration(&spec(&id, Path::new("/usr/bin/klaub"), "C-b")).unwrap();
        let flat: Vec<String> = commands.iter().map(|c| c.join(" ")).collect();
        assert!(
            flat.iter()
                .any(|c| c == "bind-key -T prefix R run-shell -b #{@klaub-focus-review}")
        );
        assert!(
            flat.iter()
                .any(|c| c == "bind-key -T prefix A run-shell -b #{@klaub-focus-agent}")
        );
        assert!(flat.iter().any(|c| c == "set-option -g @klaub-focus-review /usr/bin/klaub session focus --id ses_abc123 review"));
        assert!(
            flat.iter()
                .any(|c| c == "bind-key -T prefix I send-keys -t review F5"),
            "Prefix+I opens the impact view"
        );
        assert!(
            flat.iter()
                .any(|c| c == "bind-key -T prefix L send-keys -t review F9")
        );
        assert!(
            !flat.iter().any(|c| c.contains("unbind-key")),
            "default prefix is left alone"
        );
    }

    #[test]
    fn custom_prefix_replaces_the_default() {
        let id = SessionId::parse("ses_abc123").unwrap();
        let commands = configuration(&spec(&id, Path::new("/usr/bin/klaub"), "C-a")).unwrap();
        let flat: Vec<String> = commands.iter().map(|c| c.join(" ")).collect();
        assert!(flat.contains(&"unbind-key C-b".to_owned()));
        assert!(flat.contains(&"set-option -g prefix C-a".to_owned()));
        assert!(flat.contains(&"bind-key C-a send-prefix".to_owned()));
    }

    #[test]
    fn hostile_executable_paths_are_quoted_for_the_shell() {
        let id = SessionId::parse("ses_abc123").unwrap();
        let exe = PathBuf::from("/tmp/it's here/$(touch pwned)/klaub");
        let commands = configuration(&spec(&id, &exe, "C-b")).unwrap();
        let focus = commands
            .iter()
            .find(|c| c.get(2).is_some_and(|k| k == "@klaub-focus-review"))
            .unwrap();
        assert_eq!(
            focus[3],
            "'/tmp/it'\\''s here/$(touch pwned)/klaub' session focus --id ses_abc123 review"
        );
    }

    #[test]
    fn no_argument_can_be_read_as_a_separator() {
        let id = SessionId::parse("ses_abc123").unwrap();
        let commands = configuration(&spec(&id, Path::new("/k;"), "C-b")).unwrap();
        // The unquoted `;` inside an argument that ends with `;` would be a tmux separator.
        let offenders: Vec<_> = commands
            .iter()
            .flatten()
            .filter(|a| a.ends_with(';'))
            .collect();
        assert!(offenders.is_empty(), "{offenders:?}");
    }
}
