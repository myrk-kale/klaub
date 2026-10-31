//! Where Klaub keeps its own data outside the repository.
//!
//! Resolving these locations reads the environment, which is the composition
//! root's job. The domain only defines the shape, so adapters receive explicit
//! directories and tests can point them at temporary folders.

use std::path::{Path, PathBuf};

/// The user-level directories Klaub writes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    /// Derived data that can be deleted: extracted binaries, the parse cache.
    pub cache: PathBuf,
    /// Durable per-user state: session registry, logs.
    pub state: PathBuf,
    /// User configuration (`config.toml`).
    pub config: PathBuf,
}

impl AppDirs {
    /// Builds the set from explicit roots.
    pub fn new(
        cache: impl Into<PathBuf>,
        state: impl Into<PathBuf>,
        config: impl Into<PathBuf>,
    ) -> Self {
        Self {
            cache: cache.into(),
            state: state.into(),
            config: config.into(),
        }
    }

    /// All three directories below one root, as tests and sandboxes want them.
    pub fn under(root: &Path) -> Self {
        Self::new(root.join("cache"), root.join("state"), root.join("config"))
    }

    /// Where verified, extracted helper binaries live.
    pub fn bin_dir(&self) -> PathBuf {
        self.cache.join("bin")
    }

    /// Where the parse cache database lives.
    pub fn cache_db(&self) -> PathBuf {
        self.cache.join("analysis.sqlite3")
    }

    /// Where session records live.
    pub fn sessions_dir(&self) -> PathBuf {
        self.state.join("sessions")
    }

    /// Where log files live.
    pub fn logs_dir(&self) -> PathBuf {
        self.state.join("logs")
    }

    /// The user-level configuration file.
    pub fn user_config_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_locations_hang_off_the_roots() {
        let dirs = AppDirs::under(Path::new("/x"));
        assert_eq!(dirs.bin_dir(), Path::new("/x/cache/bin"));
        assert_eq!(dirs.sessions_dir(), Path::new("/x/state/sessions"));
        assert_eq!(dirs.user_config_file(), Path::new("/x/config/config.toml"));
    }
}
