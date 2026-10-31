//! Short words for enums that the reader shows. Every state has a word, so meaning never depends
//! on colour alone.

use klaub_core::graph::{EdgeKind, EdgeOrigin, EntrypointKind, EntrypointScope};
use klaub_core::review::{CommentStatus, CriterionStatus, ReviewStatus};
use klaub_core::testing::TestOutcome;

pub(crate) fn origin(origin: EdgeOrigin) -> &'static str {
    match origin {
        EdgeOrigin::Resolved => "resolved",
        EdgeOrigin::Heuristic => "heuristic",
        EdgeOrigin::Unresolved => "unresolved",
    }
}

/// A one-character certainty marker: solid for resolved, hollow for guesses.
pub(crate) fn origin_mark(origin: EdgeOrigin) -> char {
    match origin {
        EdgeOrigin::Resolved => '●',
        EdgeOrigin::Heuristic => '◐',
        EdgeOrigin::Unresolved => '○',
    }
}

pub(crate) fn entrypoint_kind(kind: &EntrypointKind) -> String {
    match kind {
        EntrypointKind::Http { method, route } => {
            klaub_core::text::sanitize_single_line(&format!("{method} {route}"))
        }
        EntrypointKind::Cli => "cli".to_owned(),
        EntrypointKind::Main => "main".to_owned(),
        EntrypointKind::Worker => "worker".to_owned(),
        EntrypointKind::PublicExport => "export".to_owned(),
    }
}

pub(crate) fn entrypoint_scope(scope: EntrypointScope) -> &'static str {
    match scope {
        EntrypointScope::External => "external",
        EntrypointScope::Internal => "internal",
    }
}

pub(crate) fn edge_kind(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Calls => "calls",
        EdgeKind::Imports => "imports",
        EdgeKind::Exports => "exports",
        EdgeKind::Implements => "implements",
        EdgeKind::Extends => "extends",
        EdgeKind::References => "references",
    }
}

pub(crate) fn comment_status(status: CommentStatus) -> &'static str {
    match status {
        CommentStatus::Draft => "draft",
        CommentStatus::Prepared => "prepared",
        CommentStatus::Submitted => "submitted",
        CommentStatus::Addressed => "addressed",
        CommentStatus::Unresolved => "unresolved",
        CommentStatus::Stale => "stale",
    }
}

pub(crate) fn criterion_status(status: CriterionStatus) -> &'static str {
    match status {
        CriterionStatus::NotChecked => "not checked",
        CriterionStatus::Verified => "verified",
        CriterionStatus::Pending => "pending",
        CriterionStatus::NotApplicable => "n/a",
    }
}

pub(crate) fn criterion_mark(status: CriterionStatus) -> &'static str {
    match status {
        CriterionStatus::NotChecked => "[ ]",
        CriterionStatus::Verified => "[x]",
        CriterionStatus::Pending => "[~]",
        CriterionStatus::NotApplicable => "[-]",
    }
}

pub(crate) fn outcome(outcome: TestOutcome) -> &'static str {
    match outcome {
        TestOutcome::Passed => "passed",
        TestOutcome::Failed => "FAILED",
        TestOutcome::Skipped => "skipped",
        TestOutcome::NotRun => "not run",
    }
}

pub(crate) fn review_status(status: ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Draft => "draft",
        ReviewStatus::Finished => "finished",
        ReviewStatus::Prepared => "prepared",
        ReviewStatus::Submitted => "submitted",
    }
}
