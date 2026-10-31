//! The current worktree, loaded the same way for every non-interactive command.
//!
//! These commands never touch a terminal and never talk to an agent. They read the repository the
//! process was started in and, when they need a full analysis, open a review session over it.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use klaub_app::{OpenOptions, Ports, ReviewSession};
use klaub_core::config::Config;
use klaub_core::ports::ConfigSource;
use klaub_core::snapshot::SnapshotRequest;
use klaub_git::GitSnapshotProvider;
use klaub_parse::TreeSitterParser;
use klaub_storage::{
    FsConfigSource, FsFileReader, FsFlowRepository, JsonReviewRepository, SqliteCache,
};

use crate::adapters::{ProcessTestRunner, RandomIds, SystemClock};
use crate::context::Context;
use crate::env;

/// A worktree and the configuration that applies to it.
pub(super) struct Worktree {
    /// The repository root.
    pub(super) root: PathBuf,
    /// The user configuration merged with the project's `.klaub.toml`.
    pub(super) config: Config,
}

/// Locates the worktree and loads its configuration.
pub(super) fn load(ctx: &Context) -> Result<Worktree> {
    let cwd = std::env::current_dir().context("cannot read the current directory")?;
    let root = env::find_worktree_root(&cwd).unwrap_or(cwd);
    let config = FsConfigSource::new(Some(ctx.dirs.user_config_file()))
        .load(&root)
        .with_context(|| format!("cannot load configuration for {}", root.display()))?;
    Ok(Worktree { root, config })
}

/// Opens a review session over the current worktree, creating the analysis the command reads.
pub(super) fn open_session(ctx: &Context) -> Result<ReviewSession> {
    let loaded = load(ctx)?;
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
    let request = SnapshotRequest::worktree(&loaded.root);
    let options = OpenOptions {
        session_id: None,
        brief_file: None,
    };
    ReviewSession::open(ports, request, &loaded.config, &options)
        .with_context(|| format!("cannot open a review of {}", loaded.root.display()))
}
