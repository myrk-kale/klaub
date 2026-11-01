//! Reading files the user names (briefs, reports) without leaving the worktree.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::ports::FileReader;
use crate::domain::timestamp::Timestamp;
use crate::domain::{Error, Result};

use crate::adapters::storage::fsutil::read_bounded;

/// A [`FileReader`] that resolves paths and refuses to leave the worktree.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsFileReader;

fn resolve(worktree_root: &Path, path: &Path) -> Result<PathBuf> {
    let root = worktree_root
        .canonicalize()
        .map_err(|e| Error::io(worktree_root.display(), e))?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    // Canonicalising resolves every symbolic link, so a link that points outside is caught here.
    let real = candidate
        .canonicalize()
        .map_err(|e| Error::io(candidate.display(), e))?;
    if real.starts_with(&root) {
        Ok(real)
    } else {
        Err(Error::invalid(
            "path",
            format!("{} is outside the worktree", path.display()),
        ))
    }
}

impl FileReader for FsFileReader {
    fn read_bounded(&self, worktree_root: &Path, path: &Path, max_bytes: u64) -> Result<Vec<u8>> {
        read_bounded(&resolve(worktree_root, path)?, max_bytes)
    }

    fn modified(&self, worktree_root: &Path, path: &Path) -> Result<Option<Timestamp>> {
        let real = resolve(worktree_root, path)?;
        let modified = fs::metadata(&real)
            .and_then(|m| m.modified())
            .map_err(|e| Error::io(real.display(), e))?;
        let seconds = modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_secs()).ok());
        Ok(seconds.and_then(|s| Timestamp::from_unix(s).ok()))
    }
}
