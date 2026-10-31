//! The wiring shared by commands: directories, the tmux choice, clock and id source.

use std::path::PathBuf;

use anyhow::Result;
use klaub_core::dirs::AppDirs;
use klaub_core::ports::{Clock, IdGenerator};
use klaub_tmux::binary::{self, ResolvedTmux};
use klaub_tmux::registry::SessionRegistry;

use crate::adapters::{RandomIds, SystemClock};
use crate::env;

/// Everything a command needs from its surroundings.
pub(crate) struct Context {
    pub(crate) dirs: AppDirs,
    pub(crate) home: Option<PathBuf>,
    tmux_path: Option<PathBuf>,
    pub(crate) clock: Box<dyn Clock>,
    pub(crate) ids: Box<dyn IdGenerator>,
}

impl Context {
    /// Builds the context from the process environment.
    pub(crate) fn from_process(tmux_path: Option<PathBuf>) -> Result<Self> {
        let get = |key: &str| std::env::var_os(key);
        Ok(Self {
            dirs: env::resolve_dirs(&get)?,
            home: env::home(&get),
            tmux_path,
            clock: Box::new(SystemClock),
            ids: Box::new(RandomIds),
        })
    }

    /// The session registry.
    pub(crate) fn registry(&self) -> SessionRegistry {
        SessionRegistry::new(self.dirs.sessions_dir())
    }

    /// Chooses and, for the embedded binary, verifies the tmux executable.
    pub(crate) fn resolve_tmux(&self) -> Result<ResolvedTmux> {
        Ok(binary::resolve(
            self.tmux_path.as_deref(),
            &self.dirs.bin_dir(),
        )?)
    }

    /// The explicit tmux path the user gave, if any.
    pub(crate) fn explicit_tmux(&self) -> Option<&std::path::Path> {
        self.tmux_path.as_deref()
    }
}
