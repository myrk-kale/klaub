//! Strongly typed identifiers.
//!
//! Every identifier is a validated string with a short prefix that makes it
//! recognisable in JSON files, logs and prompts (`snap_`, `rev_`, ...). Using a
//! distinct type per kind prevents mixing, say, a snapshot id with a review id.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::error::{Error, Result};

const MAX_ID_LEN: usize = 128;

fn validate(kind: &'static str, prefix: &str, value: &str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_ID_LEN {
        return Err(Error::invalid(
            kind,
            format!("length must be 1..={MAX_ID_LEN}"),
        ));
    }
    if !value.starts_with(prefix) {
        return Err(Error::invalid(kind, format!("must start with `{prefix}`")));
    }
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))
    {
        return Err(Error::invalid(
            kind,
            "only ASCII letters, digits and `_-.:` are allowed",
        ));
    }
    Ok(())
}

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// The prefix every value of this type starts with.
            pub const PREFIX: &'static str = $prefix;

            /// Validates and wraps an existing identifier.
            pub fn parse(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                validate(stringify!($name), $prefix, &value)?;
                Ok(Self(value))
            }

            /// Builds an identifier from the leading characters of a hexadecimal digest.
            ///
            /// Digests shorter than 12 characters are used whole.
            pub fn from_digest(hex: &str) -> Self {
                let take = hex.len().min(12);
                Self(format!("{}{}", $prefix, &hex[..take]))
            }

            /// The identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = Error;
            fn try_from(value: String) -> Result<Self> {
                Self::parse(value)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }
    };
}

define_id!(
    /// Identifies an immutable snapshot of the changes under review.
    SnapshotId, "snap_"
);
define_id!(
    /// Identifies a review (a revision of the feedback for a worktree).
    ReviewId, "rev_"
);
define_id!(
    /// Identifies a Git worktree (derived from its canonical path).
    WorktreeId, "wt_"
);
define_id!(
    /// Identifies a wrapped agent session.
    SessionId, "ses_"
);
define_id!(
    /// Identifies one review comment.
    CommentId, "cmt_"
);
define_id!(
    /// Identifies one feedback payload produced by finishing a review.
    FeedbackId, "fb_"
);
define_id!(
    /// Identifies a symbol or file inside one snapshot.
    UnitId, "unit_"
);
define_id!(
    /// Identifies an acceptance criterion of the review brief.
    CriterionId, "ac_"
);

/// Identifies a test case: `path::name`, as the test framework reports it.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TestId(String);

impl TestId {
    /// Validates and wraps a test identifier.
    ///
    /// Test ids are later passed to a test runner as an argument, so they may
    /// not start with `-` (option injection) and may not contain control characters.
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() || value.len() > 512 {
            return Err(Error::invalid("test id", "length must be 1..=512"));
        }
        if value.starts_with('-') {
            return Err(Error::invalid("test id", "must not start with `-`"));
        }
        if value.chars().any(char::is_control) {
            return Err(Error::invalid(
                "test id",
                "must not contain control characters",
            ));
        }
        Ok(Self(value))
    }

    /// The identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for TestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TestId({})", self.0)
    }
}

impl TryFrom<String> for TestId {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::parse(value)
    }
}

impl From<TestId> for String {
    fn from(id: TestId) -> String {
        id.0
    }
}

/// Hashes a sequence of byte slices into a lowercase hexadecimal BLAKE3 digest.
///
/// Parts are length-prefixed so `["ab", "c"]` and `["a", "bc"]` differ.
pub fn digest_parts<'a>(parts: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_requires_prefix_and_safe_characters() {
        assert!(SnapshotId::parse("snap_abc123").is_ok());
        assert!(SnapshotId::parse("rev_abc123").is_err());
        assert!(SnapshotId::parse("snap_../etc").is_err());
        assert!(SnapshotId::parse("snap_a b").is_err());
        assert!(SnapshotId::parse("").is_err());
    }

    #[test]
    fn from_digest_truncates() {
        let id = ReviewId::from_digest("0123456789abcdef0123");
        assert_eq!(id.as_str(), "rev_0123456789ab");
        assert_eq!(ReviewId::from_digest("ab").as_str(), "rev_ab");
    }

    #[test]
    fn serde_round_trip_validates() {
        let id = CommentId::parse("cmt_1").unwrap();
        let json = serde_json_like(&id);
        assert_eq!(json, "\"cmt_1\"");
    }

    fn serde_json_like<T: Serialize>(value: &T) -> String {
        serde_json::to_string(value).unwrap()
    }

    #[test]
    fn deserializing_a_wrong_prefix_fails() {
        assert!(serde_json::from_str::<SnapshotId>("\"rev_1\"").is_err());
        assert!(serde_json::from_str::<SnapshotId>("\"snap_1\"").is_ok());
    }

    #[test]
    fn test_ids_cannot_inject_options() {
        assert!(TestId::parse("tests/test_a.py::test_x").is_ok());
        assert!(TestId::parse("--rootdir=/").is_err());
        assert!(TestId::parse("a\nb").is_err());
    }

    #[test]
    fn digest_parts_is_length_prefixed() {
        assert_ne!(
            digest_parts([b"ab".as_slice(), b"c".as_slice()]),
            digest_parts([b"a".as_slice(), b"bc".as_slice()])
        );
    }
}
