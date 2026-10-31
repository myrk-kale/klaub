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
    /// List and export stored reviews.
    #[command(subcommand)]
    Reviews(ReviewsCommand),
    /// Show the feedback produced by a finished review.
    Feedback(FeedbackArgs),
    /// Inspect and run the tests associated with a review.
    #[command(subcommand)]
    Tests(TestsCommand),
    /// Inspect the flow diagram of a change.
    #[command(subcommand)]
    Flow(FlowCommand),
    /// Install a packaged skill into a coding agent's configuration.
    #[command(subcommand)]
    Skills(SkillsCommand),
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

    /// The revision to compare against (default: the configured base, usually HEAD).
    #[arg(long, value_name = "REV")]
    pub(crate) base: Option<String>,

    /// What to compare: worktree, staged, unstaged or branch.
    #[arg(long, value_enum)]
    pub(crate) scope: Option<ReviewScope>,

    /// A local brief file to import into the review. Read once, never fetched.
    #[arg(long, value_name = "FILE")]
    pub(crate) brief: Option<PathBuf>,
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

/// What `klaub review` compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ReviewScope {
    /// The base revision against the working tree, including untracked files.
    Worktree,
    /// The base revision against the index.
    Staged,
    /// The index against the working tree.
    Unstaged,
    /// The merge base of the base revision and HEAD against HEAD.
    Branch,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ReviewsCommand {
    /// List the reviews stored in this worktree.
    List(FormatArgs),
    /// Print one review.
    Show {
        /// Review id (`rev_...`).
        id: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Write the review as JSON or Markdown.
    Export {
        /// Review id (`rev_...`).
        id: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
}

#[derive(Debug, Args)]
pub(crate) struct FeedbackArgs {
    /// Review id (`rev_...`).
    pub(crate) id: String,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub(crate) format: Format,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TestsCommand {
    /// List the test associations of a review. Lists only; it runs nothing.
    List {
        /// Review id (`rev_...`).
        #[arg(long, value_name = "ID")]
        review: String,
    },
    /// Run the configured test command for a review. Shows the command and asks first.
    Run {
        /// Review id (`rev_...`).
        #[arg(long, value_name = "ID")]
        review: String,
        /// Restrict the run to these test ids.
        #[arg(long, value_name = "TEST")]
        select: Vec<String>,
        /// Do not ask for confirmation.
        #[arg(long, short)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum FlowCommand {
    /// Report whether the current snapshot has a flow diagram.
    Status,
    /// Print the snapshot context a flow diagram must describe, as JSON.
    Context,
    /// Validate a flow document.
    Validate {
        /// The flow document to check.
        file: PathBuf,
    },
    /// Print the instruction that asks an agent for a flow diagram. Prints only; it sends nothing.
    Prompt,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SkillsCommand {
    /// Install a packaged skill into an agent's configuration directory.
    Install {
        /// Which skill to install.
        skill: SkillName,
        /// The agent whose configuration receives the skill.
        #[arg(long, value_enum)]
        harness: Harness,
        /// Replace a skill the user already customised.
        #[arg(long)]
        force: bool,
    },
}

/// Skills shipped with Klaub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum SkillName {
    /// The skill that writes a flow diagram of a change.
    Flow,
    /// The skill that turns review comments into agent instructions.
    Feedback,
}

/// Agents a skill can be installed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Harness {
    /// Codex (`$CODEX_HOME/skills`, or `~/.codex/skills`).
    Codex,
    /// Claude Code (`~/.claude/skills`).
    Claude,
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
