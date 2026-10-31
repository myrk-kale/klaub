//! The command line grammar.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

/// Klaub: a semantic code reviewer for humans that wraps any coding agent.
#[derive(Debug, Parser)]
#[command(
    name = "klaub",
    version,
    about,
    long_about = "Klaub runs your coding agent in a private tmux session and gives you a fullscreen \
semantic reviewer one key chord away. Review the real code, comment on it, and hand the feedback \
back to the agent. Klaub never approves, rejects or submits anything for you.",
    propagate_version = true,
    arg_required_else_help = true
)]
pub(crate) struct Cli {
    /// Use this tmux executable instead of the embedded one (diagnostics and restricted environments).
    #[arg(long, global = true, env = "KLAUB_TMUX_PATH", value_name = "PATH")]
    pub(crate) tmux_path: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Run a coding agent in a private tmux session with a reviewer window next to it.
    #[command(
        after_help = "Examples:\n  klaub wrap codex\n  klaub wrap -- claude --model <model>\n\nArguments after the command are passed to it untouched."
    )]
    Wrap(WrapArgs),
    /// Open the reviewer.
    Review(ReviewArgs),
    /// Inspect and control wrapped sessions.
    #[command(subcommand)]
    Session(SessionCommand),
    /// Check the environment: git, tmux, terminal and directories.
    Doctor(FormatArgs),
    /// Print version and build information.
    Version(FormatArgs),
    /// Print a shell completion script.
    Completions {
        /// The shell to generate completions for.
        shell: Shell,
    },
}

/// Output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    /// Human-readable text.
    Text,
    /// JSON on standard output, nothing else.
    Json,
}

#[derive(Debug, Args)]
pub(crate) struct FormatArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub(crate) format: Format,
}

#[derive(Debug, Args)]
pub(crate) struct WrapArgs {
    /// Start the session but do not attach to it.
    #[arg(long)]
    pub(crate) detach: bool,

    /// tmux prefix key for the session, in tmux notation.
    #[arg(
        long,
        env = "KLAUB_TMUX_PREFIX",
        default_value = "C-b",
        value_name = "KEY"
    )]
    pub(crate) prefix: String,

    /// The agent command and its arguments, run without a shell.
    #[arg(
        required = true,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "COMMAND"
    )]
    pub(crate) command: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ReviewArgs {
    /// The wrapped session this reviewer belongs to (set by `klaub wrap`).
    #[arg(long, env = "KLAUB_SESSION", value_name = "ID")]
    pub(crate) session: Option<String>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SessionCommand {
    /// List sessions.
    List(FormatArgs),
    /// Attach the terminal to a session.
    Attach {
        /// Session id; optional when exactly one session is running.
        id: Option<String>,
    },
    /// End a session and its agent.
    Kill {
        /// Session id.
        id: String,
        /// Do not ask for confirmation.
        #[arg(long, short)]
        yes: bool,
    },
    /// Bring a window to the front (used by the key bindings).
    #[command(hide = true)]
    Focus {
        /// Session id.
        #[arg(long)]
        id: String,
        /// Which window.
        target: FocusTarget,
    },
    /// End the session if its agent is gone (used by a tmux hook).
    #[command(hide = true)]
    Reap {
        /// Session id.
        #[arg(long)]
        id: String,
    },
}

/// Windows of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum FocusTarget {
    /// The agent window.
    Agent,
    /// The review window.
    Review,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_grammar_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn wrap_passes_everything_after_the_command_through() {
        let cli = Cli::try_parse_from(["klaub", "wrap", "claude", "--model", "x", "-p"]).unwrap();
        let Command::Wrap(args) = cli.command else {
            panic!("expected wrap")
        };
        assert_eq!(args.command, vec!["claude", "--model", "x", "-p"]);
        assert!(!args.detach);
    }

    #[test]
    fn wrap_options_come_before_the_command_or_after_a_double_dash() {
        let cli =
            Cli::try_parse_from(["klaub", "wrap", "--detach", "--", "codex", "--detach"]).unwrap();
        let Command::Wrap(args) = cli.command else {
            panic!("expected wrap")
        };
        assert!(args.detach);
        assert_eq!(args.command, vec!["codex", "--detach"]);
    }

    #[test]
    fn wrap_requires_a_command() {
        assert!(Cli::try_parse_from(["klaub", "wrap"]).is_err());
    }
}
