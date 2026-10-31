//! Loading configuration: the user's `config.toml`, then the project's `.klaub.toml`.

use std::fs;
use std::path::{Path, PathBuf};

use klaub_core::config::Config;
use klaub_core::ports::ConfigSource;
use klaub_core::{Error, Result};

use crate::fsutil::read_bounded;

const MAX_CONFIG_BYTES: u64 = 256 * 1024;
/// Name of the project configuration file.
pub const PROJECT_CONFIG: &str = ".klaub.toml";

/// Reads configuration files from disk.
#[derive(Debug, Clone)]
pub struct FsConfigSource {
    user_file: Option<PathBuf>,
}

impl FsConfigSource {
    /// `user_file` is the per-user `config.toml`, if the user directory is known.
    pub fn new(user_file: Option<PathBuf>) -> Self {
        Self { user_file }
    }

    fn read_layer(path: &Path) -> Result<Option<String>> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                return Err(Error::invalid(
                    "configuration",
                    format!("{} is not a regular file", path.display()),
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::io(path.display(), e)),
        }
        let bytes = read_bounded(path, MAX_CONFIG_BYTES)?;
        String::from_utf8(bytes).map(Some).map_err(|_| {
            Error::invalid(
                "configuration",
                format!("{} is not valid UTF-8", path.display()),
            )
        })
    }
}

impl ConfigSource for FsConfigSource {
    fn load(&self, worktree_root: &Path) -> Result<Config> {
        let project_file = worktree_root.join(PROJECT_CONFIG);
        let mut layers: Vec<(String, String)> = Vec::new();
        if let Some(user) = &self.user_file
            && let Some(text) = Self::read_layer(user)?
        {
            layers.push((user.display().to_string(), text));
        }
        if let Some(text) = Self::read_layer(&project_file)? {
            layers.push((PROJECT_CONFIG.to_owned(), text));
        }
        let refs: Vec<(&str, &str)> = layers
            .iter()
            .map(|(origin, text)| (origin.as_str(), text.as_str()))
            .collect();
        let config = Config::from_layers(&refs)?;
        config.validate()?;
        Ok(config)
    }
}
