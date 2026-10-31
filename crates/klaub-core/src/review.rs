//! The persisted review: brief, comments, feedback and follow-up decisions.
//!
//! These types are the schema of `.klaub/reviews/<id>.json` (`schema_version: 1`).
//! Readers must ignore unknown fields and check `schema_version`; writers add
//! fields only as optional ones.

use serde::{Deserialize, Serialize};

use crate::finding::Priority;
use crate::graph::EdgeOrigin;
use crate::ids::{
    CommentId, CriterionId, FeedbackId, ReviewId, SessionId, SnapshotId, UnitId, WorktreeId,
};
use crate::path::RepoPath;
use crate::semantic::Confidence;
use crate::snapshot::Scope;
use crate::symbol::SymbolKind;
use crate::timestamp::Timestamp;

/// The current version of the review file schema.
pub const REVIEW_SCHEMA_VERSION: u32 = 1;

/// Where a review is stored, relative to the worktree (what the feedback prompt points at).
pub fn review_file(id: &ReviewId) -> String {
    format!("{}/reviews/{id}.json", crate::path::INTERNAL_DIR)
}

/// Which side of the comparison an anchor refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Version {
    /// The code before the change.
    Before,
    /// The code after the change.
    After,
}

/// Where in the code a comment is attached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    /// The file.
    pub file_path: RepoPath,
    /// The module the file belongs to.
    pub module_id: Option<String>,
    /// The review unit of the symbol.
    pub symbol_id: Option<UnitId>,
    /// The symbol's kind.
    pub symbol_kind: Option<SymbolKind>,
    /// The dotted qualified name.
    pub qualified_name: Option<String>,
    /// The declaration at the time of the comment.
    pub symbol_signature: Option<String>,
    /// Which side was being read.
    pub version: Version,
    /// Selected byte range in that side's text.
    pub byte_range: Option<(usize, usize)>,
    /// Selected line range (1-based, inclusive) in that side's text.
    pub line_range: Option<(u32, u32)>,
    /// Path of enclosing symbols from the file down to the anchor.
    pub ast_path: Vec<String>,
    /// Hash of the anchored text, normalised.
    pub fingerprint: String,
    /// A short excerpt kept so a later round can show what was commented.
    pub excerpt: Option<String>,
}

/// Lifecycle of a comment's delivery to the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentStatus {
    /// Being written; included in the draft.
    Draft,
    /// Included in a finished review whose instruction was prepared in the agent's input.
    Prepared,
    /// The human confirmed the instruction was submitted.
    Submitted,
    /// A later round concluded it was addressed.
    Addressed,
    /// A later round concluded it was not addressed.
    Unresolved,
    /// The anchor could not be relocated with confidence.
    Stale,
}

/// The reviewer's judgement of a comment in a later round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowupStatus {
    /// Not yet looked at.
    Pending,
    /// Fully resolved.
    Resolved,
    /// Partially resolved.
    Partial,
    /// Not resolved.
    Unresolved,
    /// The change made things worse or broke something else.
    Regression,
    /// The anchor no longer exists or could not be matched.
    Stale,
}

impl FollowupStatus {
    /// A short label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Resolved => "resolved",
            Self::Partial => "partial",
            Self::Unresolved => "unresolved",
            Self::Regression => "regression",
            Self::Stale => "stale",
        }
    }
}

/// One entry of a comment's follow-up history. Entries are only ever appended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FollowupEntry {
    /// The review round in which the decision was made.
    pub review_id: ReviewId,
    /// The snapshot that was inspected.
    pub snapshot_id: SnapshotId,
    /// The decision.
    pub status: FollowupStatus,
    /// Free-form note.
    pub note: Option<String>,
    /// Always `"human"`; a tool never decides.
    pub decided_by: String,
    /// When the decision was made.
    pub decided_at: Timestamp,
}

/// A review comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comment {
    /// Identity, stable across rounds.
    pub comment_id: CommentId,
    /// The wrapped session in which it was written.
    pub session_id: Option<SessionId>,
    /// The review that currently holds the comment.
    pub review_id: ReviewId,
    /// Creation time.
    pub created_at: Timestamp,
    /// Last edit time.
    pub updated_at: Timestamp,
    /// Repository identity.
    pub repository_id: String,
    /// Worktree identity.
    pub worktree_id: WorktreeId,
    /// Base commit.
    pub base_oid: String,
    /// The snapshot the anchor refers to.
    pub snapshot_id: SnapshotId,
    /// Where it is attached.
    pub anchor: Anchor,
    /// Optional priority of the comment itself.
    pub severity: Option<Priority>,
    /// The comment text (untrusted when read back).
    pub text: String,
    /// Flow steps related to the comment.
    #[serde(default)]
    pub related_flows: Vec<String>,
    /// Delivery state.
    pub status: CommentStatus,
    /// How sure the anchor still is, after relocation.
    pub anchor_confidence: Confidence,
    /// The review in which the comment was first written.
    pub origin_review_id: Option<ReviewId>,
    /// The feedback payload that carried it.
    pub feedback_id: Option<FeedbackId>,
    /// Latest follow-up decision.
    pub followup_status: FollowupStatus,
    /// Note attached to the latest decision.
    pub followup_note: Option<String>,
    /// Who decided; always `"human"` when set.
    pub followup_decided_by: Option<String>,
    /// All decisions so far.
    #[serde(default)]
    pub followups: Vec<FollowupEntry>,
}

/// Status of an acceptance criterion. `Verified` is a human statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionStatus {
    /// Not looked at yet.
    NotChecked,
    /// The human confirmed it.
    Verified,
    /// Looked at, still open.
    Pending,
    /// Does not apply to this change.
    NotApplicable,
}

/// An acceptance criterion written or imported by the reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    /// Identity within the review.
    pub id: CriterionId,
    /// The criterion text.
    pub text: String,
    /// Its status.
    pub status: CriterionStatus,
    /// Optional note.
    pub note: Option<String>,
    /// Symbols (unit ids) and comments (comment ids) it refers to.
    #[serde(default)]
    pub references: Vec<String>,
}

/// The reviewer's statement of intent. Never inferred from the agent's history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brief {
    /// What the change is meant to do.
    pub intent_summary: Option<String>,
    /// A reference to a task or ticket (free text).
    pub task_ref: Option<String>,
    /// Acceptance criteria.
    #[serde(default)]
    pub acceptance_criteria: Vec<Criterion>,
    /// A verbatim copy of the brief file passed with `--brief`, if any.
    pub source_copy: Option<String>,
}

/// Overall state of a review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Being written.
    Draft,
    /// Finished: feedback was produced and frozen.
    Finished,
    /// The instruction was prepared in the agent's input.
    Prepared,
    /// The human confirmed the instruction was submitted.
    Submitted,
}

/// How the feedback instruction reached the reviewer's hands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Only saved to the file.
    None,
    /// Pasted into the agent's input without `Enter`.
    PreparedInAgent,
    /// Copied to the clipboard.
    Copied,
    /// The file path was shown.
    Shown,
}

/// The payload produced by finishing a review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackRecord {
    /// Identity of the payload.
    pub feedback_id: FeedbackId,
    /// When the review was finished.
    pub created_at: Timestamp,
    /// The comments it froze.
    pub comment_ids: Vec<CommentId>,
    /// The instruction text (what `Prepare in agent` pastes).
    pub prompt: String,
    /// How it was delivered so far.
    pub delivery: Delivery,
    /// When it was delivered.
    pub delivered_at: Option<Timestamp>,
}

/// A review: the aggregate persisted as one JSON document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Review {
    /// Schema version of this document.
    pub schema_version: u32,
    /// Identity.
    pub review_id: ReviewId,
    /// The review of the previous round this one continues, if any.
    pub previous_review_id: Option<ReviewId>,
    /// Wrapped session, when there is one.
    pub session_id: Option<SessionId>,
    /// Absolute repository path at the time of creation (informational).
    pub repository: String,
    /// Repository identity.
    pub repository_id: String,
    /// Worktree identity.
    pub worktree_id: WorktreeId,
    /// The requested base.
    pub base: String,
    /// The base commit.
    pub base_oid: String,
    /// The comparison that was made.
    pub scope: Scope,
    /// The snapshot being reviewed.
    pub snapshot_id: SnapshotId,
    /// Creation time.
    pub created_at: Timestamp,
    /// Last write time.
    pub updated_at: Timestamp,
    /// State.
    pub status: ReviewStatus,
    /// Intent and acceptance criteria.
    #[serde(default)]
    pub brief: Brief,
    /// Comments, oldest first.
    #[serde(default)]
    pub comments: Vec<Comment>,
    /// Units the human marked as reviewed.
    #[serde(default)]
    pub reviewed_units: Vec<UnitId>,
    /// The unit the reader was on when the review was last saved.
    ///
    /// Absent from reviews written before the field existed, and cleared when a new round starts
    /// because it named a unit of the previous snapshot.
    #[serde(default)]
    pub last_unit: Option<UnitId>,
    /// The finished feedback, once there is one.
    pub feedback: Option<FeedbackRecord>,
    /// Status of the flow diagram when the review was last saved.
    pub flow: Option<FlowRef>,
}

/// Reference to the flow diagram of a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowRef {
    /// `current`, `missing`, `invalid` or `stale`.
    pub status: String,
    /// Relative path of the flow file, if any.
    pub file: Option<String>,
    /// Schema version of the flow file.
    pub schema_version: Option<u32>,
    /// The weakest confidence claimed by the flow, if known.
    pub weakest_origin: Option<EdgeOrigin>,
}

impl Review {
    /// Comments that belong in the next feedback payload (not already addressed).
    pub fn open_comments(&self) -> impl Iterator<Item = &Comment> {
        self.comments
            .iter()
            .filter(|c| !matches!(c.status, CommentStatus::Addressed))
    }

    /// Looks up a comment by id.
    pub fn comment(&self, id: &CommentId) -> Option<&Comment> {
        self.comments.iter().find(|c| &c.comment_id == id)
    }

    /// Whether a unit was marked reviewed.
    pub fn is_reviewed(&self, unit: &UnitId) -> bool {
        self.reviewed_units.contains(unit)
    }
}

/// A one-line description of a stored review, for listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSummary {
    /// Identity.
    pub review_id: ReviewId,
    /// State.
    pub status: ReviewStatus,
    /// Last write time.
    pub updated_at: Timestamp,
    /// Number of comments.
    pub comment_count: usize,
    /// The snapshot reviewed.
    pub snapshot_id: SnapshotId,
    /// Worktree identity.
    pub worktree_id: WorktreeId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_serialise_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&CommentStatus::Draft).unwrap(),
            "\"draft\""
        );
        assert_eq!(
            serde_json::to_string(&FollowupStatus::Regression).unwrap(),
            "\"regression\""
        );
        assert_eq!(
            serde_json::to_string(&CriterionStatus::NotChecked).unwrap(),
            "\"not_checked\""
        );
        assert_eq!(
            serde_json::to_string(&Delivery::PreparedInAgent).unwrap(),
            "\"prepared_in_agent\""
        );
    }

    #[test]
    fn brief_defaults_are_empty() {
        let brief = Brief::default();
        assert!(brief.intent_summary.is_none());
        assert_eq!(brief.acceptance_criteria, Vec::<Criterion>::new());
    }
}
