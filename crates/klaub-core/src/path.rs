//! A validated, repository-relative path.
//!
//! File names come from Git and from agents, so they are untrusted. [`RepoPath`]
//! can only be built from a relative, normalised, `/`-separated path without `..`
//! components or control characters, which makes path traversal impossible by
//! construction wherever it is used.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const MAX_PATH_LEN: usize = 4096;
/// Name of the directory where Klaub keeps its own data inside a worktree.
pub const INTERNAL_DIR: &str = ".klaub";

/// A path relative to the worktree root, using `/` separators.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoPath(String);

impl RepoPath {
    /// Validates and normalises a repository-relative path.
    ///
    /// A leading `./` is removed. Absolute paths, `..` components, empty
    /// components, backslashes, drive prefixes, control characters and
    /// bidirectional override characters are rejected.
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        let raw = value.as_ref();
        let trimmed = raw.strip_prefix("./").unwrap_or(raw);
        let err = |reason: &str| Error::invalid("repository path", format!("{reason}: {raw:?}"));

        if trimmed.is_empty() {
            return Err(err("empty"));
        }
        if trimmed.len() > MAX_PATH_LEN {
            return Err(err("too long"));
        }
        if trimmed.starts_with('/') {
            return Err(err("absolute paths are not allowed"));
        }
        if trimmed.contains('\\') {
            return Err(err("backslashes are not allowed"));
        }
        let mut chars = trimmed.chars();
        if let (Some(first), Some(':')) = (chars.next(), chars.next())
            && first.is_ascii_alphabetic()
        {
            return Err(err("drive prefixes are not allowed"));
        }
        if trimmed
            .chars()
            .any(|c| c.is_control() || is_bidi_control(c))
        {
            return Err(err("control characters are not allowed"));
        }
        for component in trimmed.split('/') {
            match component {
                "" => return Err(err("empty path component")),
                "." => return Err(err("`.` component")),
                ".." => return Err(err("`..` component")),
                _ => {}
            }
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The last component.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// The directory part, or `None` for a file at the repository root.
    pub fn parent(&self) -> Option<&str> {
        self.0.rsplit_once('/').map(|(parent, _)| parent)
    }

    /// The file extension without the dot.
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => Some(ext),
            _ => None,
        }
    }

    /// The file name without its extension.
    pub fn stem(&self) -> &str {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem,
            _ => name,
        }
    }

    /// Whether the path is `dir` or lies inside it (`dir` is given without a trailing slash).
    pub fn is_within(&self, dir: &str) -> bool {
        let dir = dir.trim_end_matches('/');
        dir.is_empty()
            || self.0 == dir
            || self
                .0
                .strip_prefix(dir)
                .is_some_and(|rest| rest.starts_with('/'))
    }

    /// Whether the path belongs to Klaub's own data directory (`.klaub/`).
    pub fn is_internal(&self) -> bool {
        self.is_within(INTERNAL_DIR)
    }

    /// Joins the path to a worktree root.
    pub fn to_path(&self, root: &Path) -> PathBuf {
        let mut out = root.to_path_buf();
        out.extend(self.0.split('/'));
        out
    }
}

fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RepoPath({})", self.0)
    }
}

impl AsRef<str> for RepoPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RepoPath {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl From<RepoPath> for String {
    fn from(path: RepoPath) -> String {
        path.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_relative_paths() {
        let path = RepoPath::new("src/auth/session.py").unwrap();
        assert_eq!(path.file_name(), "session.py");
        assert_eq!(path.parent(), Some("src/auth"));
        assert_eq!(path.extension(), Some("py"));
        assert_eq!(path.stem(), "session");
        assert_eq!(RepoPath::new("./a.rs").unwrap().as_str(), "a.rs");
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "a/./b",
            "a\\b",
            "C:/x",
            "a/b/",
            "a\0b",
            "a\u{202E}b",
            "a\x1b[31m",
        ] {
            assert!(RepoPath::new(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn dotfiles_have_no_extension() {
        let path = RepoPath::new(".gitignore").unwrap();
        assert_eq!(path.extension(), None);
        assert_eq!(path.stem(), ".gitignore");
    }

    #[test]
    fn is_within_matches_whole_components() {
        let path = RepoPath::new("src/api/routes.py").unwrap();
        assert!(path.is_within("src"));
        assert!(path.is_within("src/api"));
        assert!(path.is_within("src/api/routes.py"));
        assert!(!path.is_within("src/ap"));
        assert!(path.is_within(""));
    }

    #[test]
    fn internal_directory_is_detected() {
        assert!(
            RepoPath::new(".klaub/reviews/rev_1.json")
                .unwrap()
                .is_internal()
        );
        assert!(!RepoPath::new(".klaub-notes.md").unwrap().is_internal());
    }

    #[test]
    fn joins_to_a_root() {
        let joined = RepoPath::new("a/b.txt")
            .unwrap()
            .to_path(Path::new("/repo"));
        assert_eq!(joined, Path::new("/repo/a/b.txt"));
    }
}
