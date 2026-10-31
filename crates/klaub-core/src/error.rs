//! The error type shared by every crate in the workspace.

use std::fmt;

/// Convenient result alias used across the workspace.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong while producing or recording a review.
///
/// Adapters translate their library errors into one of these variants so the
/// application layer can handle failures without knowing which adapter ran.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A value failed validation.
    #[error("invalid {what}: {reason}")]
    Invalid {
        /// The kind of value that was rejected (for example `"repository path"`).
        what: &'static str,
        /// Why it was rejected.
        reason: String,
    },

    /// A referenced entity does not exist.
    #[error("{what} not found: {id}")]
    NotFound {
        /// The kind of entity.
        what: &'static str,
        /// Its identifier or path.
        id: String,
    },

    /// A file system operation failed.
    #[error("i/o error on {path}: {source}")]
    Io {
        /// The path involved.
        path: String,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },

    /// Git failed or produced output Klaub could not interpret.
    #[error("git: {0}")]
    Git(String),

    /// A source file could not be analysed.
    #[error("parse: {0}")]
    Parse(String),

    /// A configuration file is invalid.
    #[error("configuration error at {location}: {message}")]
    Config {
        /// `file:line:column` or a key path.
        location: String,
        /// What is wrong.
        message: String,
    },

    /// Reading or writing persisted data failed.
    #[error("storage: {0}")]
    Storage(String),

    /// tmux failed or behaved unexpectedly.
    #[error("tmux: {0}")]
    Tmux(String),

    /// The operation is not available in this context (for example pasting without an agent).
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// The operation conflicts with the current state (stale snapshot, concurrent writer).
    #[error("conflict: {0}")]
    Conflict(String),

    /// The operation was cancelled by the user.
    #[error("cancelled")]
    Cancelled,
}

impl Error {
    /// Builds an [`Error::Invalid`].
    pub fn invalid(what: &'static str, reason: impl fmt::Display) -> Self {
        Self::Invalid {
            what,
            reason: reason.to_string(),
        }
    }

    /// Builds an [`Error::NotFound`].
    pub fn not_found(what: &'static str, id: impl fmt::Display) -> Self {
        Self::NotFound {
            what,
            id: id.to_string(),
        }
    }

    /// Builds an [`Error::Io`] from a path and an I/O error.
    pub fn io(path: impl fmt::Display, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_string(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_human_readable() {
        assert_eq!(
            Error::invalid("repository path", "contains `..`").to_string(),
            "invalid repository path: contains `..`"
        );
        assert_eq!(
            Error::not_found("review", "rev_1").to_string(),
            "review not found: rev_1"
        );
    }
}
