//! The current worktree, loaded the same way for every non-interactive command.
//!
//! These commands never touch a terminal and never talk to an agent. They read the repository the
//! process was started in and, when they need a full analysis, open a review session over it.

use std::path::PathBuf;
use std::sync::Arc;

use crate::adapters::git::GitSnapshotProvider;
use crate::adapters::parse::TreeSitterParser;
use crate::adapters::storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};
use crate::app::{OpenOptions, Ports, ReviewSession};
use crate::domain::config::Config;
use crate::domain::i18n::cli as t;
use crate::domain::ids::ReviewId;
use crate::domain::ports::{ConfigSource, ReviewRepository, SnapshotProvider};
use crate::domain::snapshot::SnapshotRequest;
use anyhow::{Context as _, Result, bail};

use crate::adapters::{ProcessTestRunner, RandomIds, SystemClock};
use crate::cli::context::Context;
use crate::cli::env;

/// A worktree and the configuration that applies to it.
pub(super) struct Worktree {
    /// The repository root.
    pub(super) root: PathBuf,
    /// The user configuration merged with the project's `.klaub.toml`.
    pub(super) config: Config,
}

/// Locates the worktree and loads its configuration.
pub(super) fn load(ctx: &Context) -> Result<Worktree> {
    let cwd = std::env::current_dir().context(t::cannot_read_cwd(ctx.locale))?;
    let root = env::find_worktree_root(&cwd).unwrap_or(cwd);
    let config = FsConfigSource::new(Some(ctx.dirs.user_config_file()))
        .load(&root)
        .with_context(|| t::cannot_load_configuration(ctx.locale, &root.display().to_string()))?;
    Ok(Worktree { root, config })
}

/// Opens a review session over the current worktree, creating the analysis the command reads.
pub(super) fn open_session(ctx: &Context) -> Result<ReviewSession> {
    let loaded = load(ctx)?;
    let request = SnapshotRequest::worktree(&loaded.root);
    open_with(ctx, &loaded, request)
}

/// Opens the session of one stored review.
///
/// The review is loaded by its id, so an id that does not exist is an error. It is then compared
/// with the worktree as it is now: a review belongs to one snapshot, and if the files have changed
/// since, its associations would describe a state that is gone. That is refused instead of
/// answering for different code, and it is checked before any session is opened, so nothing is
/// written for the state that was not asked about.
pub(super) fn open_session_of_review(ctx: &Context, id: &str) -> Result<ReviewSession> {
    let l = ctx.locale;
    let review_id = ReviewId::parse(id).context(t::invalid_review_id(l))?;
    let loaded = load(ctx)?;
    let stored = JsonReviewRepository::new(&loaded.root)
        .load(&review_id)
        .with_context(|| t::cannot_load_review(l, review_id.as_str()))?;
    let request = SnapshotRequest {
        scope: stored.scope.clone(),
        base: stored.base.clone(),
        ..SnapshotRequest::worktree(&loaded.root)
    };
    let current = GitSnapshotProvider::new(Arc::new(SystemClock))
        .capture(&request)
        .with_context(|| t::cannot_read_worktree_to_compare(l, review_id.as_str()))?;
    if current.id != stored.snapshot_id {
        bail!("{}", t::review_made_for_earlier(l, review_id.as_str()));
    }
    open_with(ctx, &loaded, request)
}

fn open_with(ctx: &Context, loaded: &Worktree, request: SnapshotRequest) -> Result<ReviewSession> {
    let config_source = FsConfigSource::new(Some(ctx.dirs.user_config_file()));

    let clock = Arc::new(SystemClock);
    let ports = Ports {
        snapshots: Arc::new(GitSnapshotProvider::new(clock.clone())),
        parser: Arc::new(TreeSitterParser::new()),
        reviews: Arc::new(JsonReviewRepository::new(&loaded.root)),
        flows: Arc::new(FsFlowRepository::new(
            &loaded.root,
            &loaded.config.flows.directory,
        )?),
        cache: Arc::new(SqliteCache::open(&ctx.dirs.cache_db())?),
        files: Arc::new(FsFileReader),
        config: Arc::new(config_source),
        clock,
        ids: Arc::new(RandomIds),
        runner: Some(Arc::new(ProcessTestRunner)),
    };
    let options = OpenOptions {
        session_id: None,
        brief_file: None,
    };
    ReviewSession::open(ports, request, &loaded.config, &options)
        .with_context(|| t::cannot_open_review(ctx.locale, &loaded.root.display().to_string()))
}
