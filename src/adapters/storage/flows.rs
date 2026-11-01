//! Flow documents under `.klaub/flows/<snapshot-id>.json`.

use std::path::PathBuf;

use crate::domain::ids::SnapshotId;
use crate::domain::path::RepoPath;
use crate::domain::ports::{FlowFile, FlowRepository};
use crate::domain::{Error, Result};

use crate::adapters::storage::fsutil::{ensure_no_symlinks_below, read_bounded};

/// Reads flow documents from a directory inside the worktree.
#[derive(Debug, Clone)]
pub struct FsFlowRepository {
    root: PathBuf,
    directory: String,
}

impl FsFlowRepository {
    /// `directory` is relative to `root` (the `[flows] directory` setting, `.klaub/flows` by default).
    pub fn new(root: impl Into<PathBuf>, directory: &str) -> Result<Self> {
        RepoPath::new(directory)?;
        Ok(Self {
            root: root.into(),
            directory: directory.trim_end_matches('/').to_owned(),
        })
    }
}

impl FlowRepository for FsFlowRepository {
    fn expected_path(&self, snapshot: &SnapshotId) -> Result<RepoPath> {
        RepoPath::new(format!("{}/{snapshot}.json", self.directory))
    }

    fn read(&self, snapshot: &SnapshotId, max_bytes: u64) -> Result<Option<FlowFile>> {
        let path = self.expected_path(snapshot)?;
        let full = path.to_path(&self.root);
        ensure_no_symlinks_below(&self.root, &full)?;
        match std::fs::symlink_metadata(&full) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                return Err(Error::invalid(
                    "flow",
                    format!("{path} is not a regular file"),
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::io(full.display(), e)),
        }
        Ok(Some(FlowFile {
            bytes: read_bounded(&full, max_bytes)?,
            path,
        }))
    }
}
