//! Reviews as versioned JSON files.
//!
//! `.klaub/reviews/<review-id>.json` is the source of truth for comments. Writes are atomic and
//! serialised by an advisory lock, so a crash or a second Klaub process cannot corrupt a file;
//! readers check `schema_version` and ignore fields they do not know.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::ids::{ReviewId, WorktreeId};
use crate::domain::path::INTERNAL_DIR;
use crate::domain::ports::ReviewRepository;
use crate::domain::review::{REVIEW_SCHEMA_VERSION, Review, ReviewSummary};
use crate::domain::{Error, Result};

use crate::adapters::storage::fsutil::{
    ensure_no_symlinks_below, private_dir, read_bounded, write_atomic,
};

/// Largest review file that is read.
const MAX_REVIEW_BYTES: u64 = 16 * 1024 * 1024;

/// Stores the reviews of one worktree under `<root>/.klaub/reviews`.
#[derive(Debug, Clone)]
pub struct JsonReviewRepository {
    root: PathBuf,
}

impl JsonReviewRepository {
    /// A repository for the worktree at `root` (a canonical path).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn dir(&self) -> PathBuf {
        self.root.join(INTERNAL_DIR).join("reviews")
    }

    /// The path of a review file relative to the worktree (what feedback prompts refer to).
    pub fn relative_path(id: &ReviewId) -> String {
        crate::domain::review::review_file(id)
    }

    fn file(&self, id: &ReviewId) -> PathBuf {
        self.dir().join(format!("{id}.json"))
    }

    fn prepare(&self) -> Result<()> {
        let dir = self.dir();
        ensure_no_symlinks_below(&self.root, &dir)?;
        private_dir(&dir)
    }

    fn with_lock<T>(&self, action: impl FnOnce() -> Result<T>) -> Result<T> {
        let lock_path = self.dir().join(".lock");
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .map_err(|e| Error::io(lock_path.display(), e))?;
        lock.lock().map_err(|e| Error::io(lock_path.display(), e))?;
        let result = action();
        let _ = lock.unlock();
        result
    }

    fn read(path: &Path) -> Result<Review> {
        let bytes = read_bounded(path, MAX_REVIEW_BYTES)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| Error::Storage(format!("{} is not valid JSON: {e}", path.display())))?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64);
        if version != Some(u64::from(REVIEW_SCHEMA_VERSION)) {
            return Err(Error::Storage(format!(
                "{} has schema_version {:?}; this Klaub reads version {REVIEW_SCHEMA_VERSION}",
                path.display(),
                version
            )));
        }
        serde_json::from_value(value).map_err(|e| {
            Error::Storage(format!(
                "{} does not match the review schema: {e}",
                path.display()
            ))
        })
    }
}

impl ReviewRepository for JsonReviewRepository {
    fn save(&self, review: &Review) -> Result<()> {
        self.prepare()?;
        let mut bytes = serde_json::to_vec_pretty(review)
            .map_err(|e| Error::Storage(format!("cannot encode the review: {e}")))?;
        bytes.push(b'\n');
        self.with_lock(|| write_atomic(&self.file(&review.review_id), &bytes))
    }

    fn load(&self, id: &ReviewId) -> Result<Review> {
        ensure_no_symlinks_below(&self.root, &self.file(id))?;
        let path = self.file(id);
        if !path.exists() {
            return Err(Error::not_found("review", id));
        }
        Self::read(&path)
    }

    fn list(&self) -> Result<Vec<ReviewSummary>> {
        let dir = self.dir();
        ensure_no_symlinks_below(&self.root, &dir)?;
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(dir.display(), e)),
        };
        let mut summaries = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".json") else {
                continue;
            };
            let Ok(id) = ReviewId::parse(stem) else {
                continue;
            };
            match self.load(&id) {
                Ok(review) => summaries.push(ReviewSummary {
                    review_id: review.review_id,
                    status: review.status,
                    updated_at: review.updated_at,
                    comment_count: review.comments.len(),
                    snapshot_id: review.snapshot_id,
                    worktree_id: review.worktree_id,
                }),
                Err(error) => tracing::warn!(%error, file = %name, "skipping an unreadable review"),
            }
        }
        summaries.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.review_id.cmp(&a.review_id))
        });
        Ok(summaries)
    }

    fn latest(&self, worktree: &WorktreeId) -> Result<Option<ReviewId>> {
        Ok(self
            .list()?
            .into_iter()
            .find(|s| &s.worktree_id == worktree)
            .map(|s| s.review_id))
    }
}
