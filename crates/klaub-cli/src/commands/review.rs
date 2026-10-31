//! `klaub review`: opens the semantic reader over the current worktree.
//!
//! This is the composition root for a review. It wires the Git, tree-sitter and storage adapters to
//! the ports, opens or resumes a [`ReviewSession`](klaub_app::ReviewSession) and hands it to the
//! reader. Inside `klaub wrap` the agent's pane is available, so feedback can be prepared there;
//! run on its own, the reader can only copy or show it. Nothing is ever submitted.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use klaub_app::{OpenOptions, Ports, ReviewSession};
use klaub_core::ids::SessionId;
use klaub_core::ports::{AgentChannel, Clipboard, ConfigSource};
use klaub_core::snapshot::{Scope, SnapshotRequest};
use klaub_git::GitSnapshotProvider;
use klaub_parse::TreeSitterParser;
use klaub_storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};
use klaub_tmux::channel::TmuxAgentChannel;
use klaub_tui::App;

use crate::adapters::{ClipboardCopy, ProcessTestRunner, RandomIds, SystemClock};
use crate::cli::{Format, ReviewArgs};
use crate::context::Context;
use crate::{env, logging, output};

pub(super) fn run(ctx: &Context, args: &ReviewArgs) -> Result<ExitCode> {
    logging::init_file(&ctx.dirs.logs_dir().join("review.log"));

    // Resolved before the terminal check, so a bad `--session` is reported as itself even when the
    // command is run without a terminal (as the integration tests do).
    let target = resolve_target(ctx, args)?;

    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "the reviewer is interactive and needs a terminal; run it in your terminal or through `klaub wrap`"
        );
    }
    let config_source = FsConfigSource::new(Some(ctx.dirs.user_config_file()));
    let config = config_source.load(&target.worktree).with_context(|| {
        format!(
            "cannot load configuration for {}",
            target.worktree.display()
        )
    })?;

    let clock = Arc::new(SystemClock);
    let ports = Ports {
        snapshots: Arc::new(GitSnapshotProvider::new(clock.clone())),
        parser: Arc::new(TreeSitterParser::new()),
        reviews: Arc::new(JsonReviewRepository::new(&target.worktree)),
        flows: Arc::new(FsFlowRepository::new(
            &target.worktree,
            &config.flows.directory,
        )?),
        cache: Arc::new(SqliteCache::open(&ctx.dirs.cache_db())?),
        files: Arc::new(FsFileReader),
        config: Arc::new(config_source),
        clock,
        ids: Arc::new(RandomIds),
        runner: Some(Arc::new(ProcessTestRunner)),
    };
    let parser = Arc::clone(&ports.parser);

    let mut request = SnapshotRequest::worktree(&target.worktree);
    if let Some(base) = &args.base {
        request.base.clone_from(base);
    } else if !config.review.base.is_empty() {
        request.base.clone_from(&config.review.base);
    }
    if let Some(scope) = args.scope {
        request.scope = match scope {
            crate::cli::ReviewScope::Worktree => Scope::Worktree,
            crate::cli::ReviewScope::Staged => Scope::Staged,
            crate::cli::ReviewScope::Unstaged => Scope::Unstaged,
            crate::cli::ReviewScope::Branch => Scope::Branch,
        };
    }
    let options = OpenOptions {
        session_id: target.session_id,
        brief_file: args.brief.clone(),
    };
    let session = ReviewSession::open(ports, request, &config, &options)
        .with_context(|| format!("cannot open a review of {}", target.worktree.display()))?;

    let clipboard: Option<Arc<dyn Clipboard>> = Some(Arc::new(ClipboardCopy));
    let app = klaub_tui::run(App::new(session, parser, target.agent, clipboard))?;
    if args.wait {
        return report_wait(&app.session, args.format);
    }
    Ok(ExitCode::SUCCESS)
}

/// What `--wait` prints once the human has closed the reader, and the exit code that says how.
///
/// A finished review exits 0 and a draft exits 3, so a caller can tell "the human gave feedback"
/// from "the human closed the reader" without reading the output. With `--format json` standard
/// output holds the review export and nothing else; the explanation goes to standard error.
fn report_wait(session: &ReviewSession, format: Format) -> Result<ExitCode> {
    let outcome = session.outcome();
    let code = if outcome.finished() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    };
    match format {
        Format::Json => output::json(&session.export_json())?,
        Format::Text if outcome.finished() => output::line(&format!(
            "review {} finished: {} comment(s). Feedback: {}",
            outcome.review_id, outcome.comments, outcome.file
        )),
        Format::Text => {}
    }
    if !outcome.finished() {
        output::error_line(&format!(
            "the review was closed without being finished; the draft ({} comment(s)) is in {}",
            outcome.comments, outcome.file
        ));
    }
    Ok(code)
}

/// Where the review happens and, inside a wrapped session, the pane the feedback can be prepared in.
struct ReviewTarget {
    session_id: Option<SessionId>,
    agent: Option<Arc<dyn AgentChannel>>,
    worktree: PathBuf,
}

fn resolve_target(ctx: &Context, args: &ReviewArgs) -> Result<ReviewTarget> {
    let Some(raw) = args.session.as_deref() else {
        let cwd = std::env::current_dir()?;
        return Ok(ReviewTarget {
            session_id: None,
            agent: None,
            worktree: env::find_worktree_root(&cwd).unwrap_or(cwd),
        });
    };
    let id = SessionId::parse(raw).context("invalid --session value")?;
    let record = ctx
        .registry()
        .load(&id)
        .with_context(|| format!("cannot load session {id}"))?;
    let channel: Arc<dyn AgentChannel> = Arc::new(TmuxAgentChannel::new(record.clone()));
    Ok(ReviewTarget {
        session_id: Some(id),
        agent: Some(channel),
        worktree: record.worktree,
    })
}
