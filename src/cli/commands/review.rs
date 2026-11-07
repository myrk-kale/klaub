//! `klaub review`: opens the semantic reader over the current worktree.
//!
//! This is the composition root for a review. It wires the Git, tree-sitter and storage adapters to
//! the ports, opens or resumes a [`ReviewSession`](crate::app::ReviewSession) and hands it to the
//! reader. Inside `klaub wrap` the agent's pane is available, so feedback can be prepared there;
//! run on its own, the reader can only copy or show it. Nothing is ever submitted.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use crate::adapters::git::GitSnapshotProvider;
use crate::adapters::parse::TreeSitterParser;
use crate::adapters::storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};
use crate::app::{OpenOptions, Ports, ReviewSession};
use crate::domain::i18n::{Locale, cli as t};
use crate::domain::ids::SessionId;
use crate::domain::ports::ConfigSource;
use crate::domain::snapshot::{Scope, SnapshotRequest};
use crate::tui::App;
use anyhow::{Context as _, Result};

use crate::adapters::{ProcessTestRunner, RandomIds, SystemClock};
use crate::cli::args::{Format, ReviewArgs};
use crate::cli::context::Context;
use crate::cli::{env, logging, output};

pub(super) fn run(ctx: &Context, args: &ReviewArgs) -> Result<ExitCode> {
    logging::init_file(&ctx.dirs.logs_dir().join("review.log"));

    // Resolved before the terminal check, so a bad `--session` is reported as itself even when the
    // command is run without a terminal (as the integration tests do).
    let target = resolve_target(ctx, args)?;

    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        anyhow::bail!("{}", t::review_needs_terminal(ctx.locale));
    }
    let config_source = FsConfigSource::new(Some(ctx.dirs.user_config_file()));
    let mut config = config_source.load(&target.worktree).with_context(|| {
        t::cannot_load_configuration(ctx.locale, &target.worktree.display().to_string())
    })?;

    // The project may name a language of its own; otherwise the process-wide choice stands. The
    // reader only ever sees a concrete language, never `auto`.
    let locale = config.ui.language.locale().unwrap_or(ctx.locale);
    config.ui.language = locale.into();

    crate::tui::Keymap::from_overrides(&config.keys)
        .map_err(|error| anyhow::anyhow!("{error}"))
        .context(t::keys_table_invalid(locale))?;

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
            crate::cli::args::ReviewScope::Worktree => Scope::Worktree,
            crate::cli::args::ReviewScope::Staged => Scope::Staged,
            crate::cli::args::ReviewScope::Unstaged => Scope::Unstaged,
            crate::cli::args::ReviewScope::Branch => Scope::Branch,
        };
    }
    let options = OpenOptions {
        session_id: target.session_id,
        brief_file: args.brief.clone(),
    };
    let first_review_here = !target.worktree.join(".klaub").exists();
    let session = ReviewSession::open(ports, request, &config, &options)
        .with_context(|| t::cannot_open_review(locale, &target.worktree.display().to_string()))?;

    let mut app = App::new(session, parser);
    if first_review_here {
        app = app.with_storage_notice();
    }
    let app = crate::tui::run(app)?;
    if args.wait {
        return report_wait(&app.session, args.format, locale);
    }
    Ok(ExitCode::SUCCESS)
}

/// What `--wait` prints once the human has closed the reader, and the exit code that says how.
///
/// A finished review exits 0 and a draft exits 3, so a caller can tell "the human gave feedback"
/// from "the human closed the reader" without reading the output. With `--format json` standard
/// output holds the review export and nothing else; the explanation goes to standard error.
fn report_wait(session: &ReviewSession, format: Format, locale: Locale) -> Result<ExitCode> {
    let outcome = session.outcome();
    let code = if outcome.finished() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    };
    match format {
        Format::Json => output::json(&session.export_json())?,
        Format::Text if outcome.finished() => output::line(&t::review_finished(
            locale,
            outcome.review_id.as_str(),
            outcome.comments,
            &outcome.file,
        )),
        Format::Text => {}
    }
    if !outcome.finished() {
        output::error_line(&t::review_closed_unfinished(
            locale,
            outcome.comments,
            &outcome.file,
        ));
    }
    Ok(code)
}

/// Where the review happens and, inside a wrapped session, the session it belongs to.
struct ReviewTarget {
    session_id: Option<SessionId>,
    worktree: PathBuf,
}

fn resolve_target(ctx: &Context, args: &ReviewArgs) -> Result<ReviewTarget> {
    let Some(raw) = args.session.as_deref() else {
        let cwd = std::env::current_dir()?;
        return Ok(ReviewTarget {
            session_id: None,
            worktree: env::find_worktree_root(&cwd).unwrap_or(cwd),
        });
    };
    let id = SessionId::parse(raw).context(t::invalid_session_value(ctx.locale))?;
    let record = ctx
        .registry()
        .load(&id)
        .with_context(|| t::cannot_load_session(ctx.locale, id.as_str()))?;
    Ok(ReviewTarget {
        session_id: Some(id),
        worktree: record.worktree,
    })
}
