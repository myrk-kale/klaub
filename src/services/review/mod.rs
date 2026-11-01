//! A pure domain service that manages the lifecycle of a review: comments and their anchors,
//! the reviewed set, finishing a review into a feedback payload, the human-confirmed delivery
//! states, relocation of anchors in a later snapshot, and the follow-up decisions of the next round.
//!
//! Everything that happens here is an explicit action of the human; no function infers that a
//! comment was submitted, resolved or addressed.

mod anchor;
mod export;
mod feedback;

use crate::domain::finding::Priority;
use crate::domain::ids::{
    CommentId, FeedbackId, ReviewId, SessionId, SnapshotId, UnitId, WorktreeId,
};
use crate::domain::review::{
    Anchor, Brief, Comment, CommentStatus, Delivery, FeedbackRecord, FollowupEntry, FollowupStatus,
    REVIEW_SCHEMA_VERSION, Review, ReviewStatus,
};
use crate::domain::semantic::Confidence;
use crate::domain::snapshot::Scope;
use crate::domain::text::sanitize_terminal_text;
use crate::domain::timestamp::Timestamp;
use crate::domain::{Error, Result};

pub use anchor::{Candidate, Relocation, build_anchor_fingerprint, relocate};
pub use export::{ExportContext, export_json, export_markdown, export_sarif};
pub use feedback::feedback_prompt;

/// Longest accepted comment text, in characters.
pub const MAX_COMMENT_CHARS: usize = 10_000;

/// Identity of the repository and snapshot a review belongs to.
#[derive(Debug, Clone)]
pub struct ReviewTarget {
    /// Absolute repository path (informational).
    pub repository: String,
    /// Repository identity.
    pub repository_id: String,
    /// Worktree identity.
    pub worktree_id: WorktreeId,
    /// Requested base.
    pub base: String,
    /// Base commit.
    pub base_oid: String,
    /// The comparison.
    pub scope: Scope,
    /// The snapshot under review.
    pub snapshot_id: SnapshotId,
    /// The wrapped session, if any.
    pub session_id: Option<SessionId>,
}

/// Creates a new, empty review. `hex` must be random lowercase hexadecimal (at least 12 characters).
pub fn new_review(target: &ReviewTarget, hex: &str, now: Timestamp) -> Review {
    Review {
        schema_version: REVIEW_SCHEMA_VERSION,
        review_id: ReviewId::from_digest(hex),
        previous_review_id: None,
        session_id: target.session_id.clone(),
        repository: target.repository.clone(),
        repository_id: target.repository_id.clone(),
        worktree_id: target.worktree_id.clone(),
        base: target.base.clone(),
        base_oid: target.base_oid.clone(),
        scope: target.scope.clone(),
        snapshot_id: target.snapshot_id.clone(),
        created_at: now,
        updated_at: now,
        status: ReviewStatus::Draft,
        brief: Brief::default(),
        comments: Vec::new(),
        reviewed_units: Vec::new(),
        last_unit: None,
        muted_rules: Vec::new(),
        feedback: None,
        flow: None,
    }
}

fn clean_text(text: &str) -> Result<String> {
    let cleaned = sanitize_terminal_text(text).replace('\r', "");
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return Err(Error::invalid("comment", "the text is empty"));
    }
    if trimmed.chars().count() > MAX_COMMENT_CHARS {
        return Err(Error::invalid(
            "comment",
            format!("longer than {MAX_COMMENT_CHARS} characters"),
        ));
    }
    Ok(trimmed.to_owned())
}

/// Adds a draft comment and returns its id.
pub fn add_comment(
    review: &mut Review,
    hex: &str,
    now: Timestamp,
    anchor: Anchor,
    text: &str,
    severity: Option<Priority>,
) -> Result<CommentId> {
    let text = clean_text(text)?;
    let id = CommentId::from_digest(hex);
    if review.comment(&id).is_some() {
        return Err(Error::Conflict(format!("comment id {id} already exists")));
    }
    review.comments.push(Comment {
        comment_id: id.clone(),
        session_id: review.session_id.clone(),
        review_id: review.review_id.clone(),
        created_at: now,
        updated_at: now,
        repository_id: review.repository_id.clone(),
        worktree_id: review.worktree_id.clone(),
        base_oid: review.base_oid.clone(),
        snapshot_id: review.snapshot_id.clone(),
        anchor,
        severity,
        text,
        related_flows: Vec::new(),
        status: CommentStatus::Draft,
        anchor_confidence: Confidence::CERTAIN,
        origin_review_id: Some(review.review_id.clone()),
        feedback_id: None,
        followup_status: FollowupStatus::Pending,
        followup_note: None,
        followup_decided_by: None,
        followups: Vec::new(),
    });
    touch(review, now);
    reopen_if_finished(review);
    Ok(id)
}

fn comment_mut<'a>(review: &'a mut Review, id: &CommentId) -> Result<&'a mut Comment> {
    review
        .comments
        .iter_mut()
        .find(|c| &c.comment_id == id)
        .ok_or_else(|| Error::not_found("comment", id))
}

fn touch(review: &mut Review, now: Timestamp) {
    review.updated_at = now;
}

/// A change after finishing starts a new draft; the old feedback is kept until the next finish.
fn reopen_if_finished(review: &mut Review) {
    if review.status != ReviewStatus::Draft {
        review.status = ReviewStatus::Draft;
    }
}

/// Edits the text (and optionally the severity) of a comment that has not been handed over.
pub fn edit_comment(
    review: &mut Review,
    id: &CommentId,
    now: Timestamp,
    text: &str,
    severity: Option<Priority>,
) -> Result<()> {
    let text = clean_text(text)?;
    let comment = comment_mut(review, id)?;
    if matches!(
        comment.status,
        CommentStatus::Submitted | CommentStatus::Addressed
    ) {
        return Err(Error::Conflict(
            "a comment that was already submitted cannot be edited; add a new one".to_owned(),
        ));
    }
    comment.text = text;
    comment.severity = severity;
    comment.updated_at = now;
    comment.status = CommentStatus::Draft;
    touch(review, now);
    reopen_if_finished(review);
    Ok(())
}

/// Deletes a draft comment ("the user can discard a draft without sending it").
pub fn delete_comment(review: &mut Review, id: &CommentId, now: Timestamp) -> Result<()> {
    let comment = review
        .comment(id)
        .ok_or_else(|| Error::not_found("comment", id))?;
    if comment.status != CommentStatus::Draft {
        return Err(Error::Conflict(
            "only draft comments can be deleted".to_owned(),
        ));
    }
    review.comments.retain(|c| &c.comment_id != id);
    touch(review, now);
    Ok(())
}

/// Marks a unit reviewed, or clears the mark. Only an explicit human action counts. Returns the new state.
pub fn toggle_reviewed(review: &mut Review, unit: &UnitId, now: Timestamp) -> bool {
    let reviewed = if let Some(position) = review.reviewed_units.iter().position(|u| u == unit) {
        review.reviewed_units.remove(position);
        false
    } else {
        review.reviewed_units.push(unit.clone());
        true
    };
    touch(review, now);
    reviewed
}

/// Finishes the review: freezes the open comments into a feedback payload.
///
/// Finishing is idempotent: when nothing changed since the last finish, the existing payload is
/// returned instead of creating another one that could be pasted twice.
pub fn finish(
    review: &mut Review,
    hex: &str,
    now: Timestamp,
    comments_file: &str,
) -> Result<FeedbackRecord> {
    if review.status != ReviewStatus::Draft
        && let Some(existing) = &review.feedback
    {
        return Ok(existing.clone());
    }
    let ids: Vec<CommentId> = review
        .open_comments()
        .map(|c| c.comment_id.clone())
        .collect();
    if ids.is_empty() {
        return Err(Error::invalid(
            "review",
            "there are no comments to hand over",
        ));
    }
    let feedback_id = FeedbackId::from_digest(hex);
    for comment in review
        .comments
        .iter_mut()
        .filter(|c| ids.contains(&c.comment_id))
    {
        comment.feedback_id = Some(feedback_id.clone());
    }
    let record = FeedbackRecord {
        feedback_id,
        created_at: now,
        prompt: feedback_prompt(ids.len(), comments_file),
        comment_ids: ids,
        delivery: Delivery::None,
        delivered_at: None,
    };
    review.feedback = Some(record.clone());
    review.status = ReviewStatus::Finished;
    touch(review, now);
    Ok(record)
}

/// Records how the feedback instruction reached the human's hands.
///
/// `PreparedInAgent` moves the comments to `prepared` ("the input was filled"), which is *not*
/// the same as submitted.
pub fn record_delivery(review: &mut Review, delivery: Delivery, now: Timestamp) -> Result<()> {
    let feedback = review
        .feedback
        .as_mut()
        .ok_or_else(|| Error::invalid("review", "finish the review first"))?;
    feedback.delivery = delivery;
    feedback.delivered_at = Some(now);
    if delivery == Delivery::PreparedInAgent {
        review.status = ReviewStatus::Prepared;
        let ids = feedback.comment_ids.clone();
        for comment in review
            .comments
            .iter_mut()
            .filter(|c| ids.contains(&c.comment_id) && c.status == CommentStatus::Draft)
        {
            comment.status = CommentStatus::Prepared;
        }
    }
    touch(review, now);
    Ok(())
}

/// The human confirms that they pressed `Enter`: only they can say so.
pub fn confirm_submitted(review: &mut Review, now: Timestamp) -> Result<()> {
    let feedback = review
        .feedback
        .as_ref()
        .ok_or_else(|| Error::invalid("review", "finish the review first"))?;
    let ids = feedback.comment_ids.clone();
    for comment in review
        .comments
        .iter_mut()
        .filter(|c| ids.contains(&c.comment_id))
    {
        if matches!(
            comment.status,
            CommentStatus::Draft | CommentStatus::Prepared
        ) {
            comment.status = CommentStatus::Submitted;
        }
    }
    review.status = ReviewStatus::Submitted;
    touch(review, now);
    Ok(())
}

/// Opens the next round: keeps every comment (with its identity and history), clears the reviewed
/// set (it referred to the old snapshot) and asks for a decision on each comment.
pub fn next_round(previous: &Review, hex: &str, now: Timestamp, target: &ReviewTarget) -> Review {
    let mut review = new_review(target, hex, now);
    review.previous_review_id = Some(previous.review_id.clone());
    review.brief = previous.brief.clone();
    review.muted_rules.clone_from(&previous.muted_rules);
    review.comments = previous
        .comments
        .iter()
        .map(|c| Comment {
            review_id: review.review_id.clone(),
            origin_review_id: c
                .origin_review_id
                .clone()
                .or_else(|| Some(previous.review_id.clone())),
            followup_status: FollowupStatus::Pending,
            ..c.clone()
        })
        .collect();
    review
}

/// Records the human's judgement of a comment in the current round, keeping the history.
///
/// Neither a passing test nor a vanished anchor does this for them.
pub fn decide_followup(
    review: &mut Review,
    id: &CommentId,
    status: FollowupStatus,
    note: Option<String>,
    now: Timestamp,
) -> Result<()> {
    let review_id = review.review_id.clone();
    let snapshot_id = review.snapshot_id.clone();
    let comment = comment_mut(review, id)?;
    let note = note
        .map(|n| sanitize_terminal_text(&n).trim().to_owned())
        .filter(|n| !n.is_empty());
    comment.followups.push(FollowupEntry {
        review_id,
        snapshot_id,
        status,
        note: note.clone(),
        decided_by: "human".to_owned(),
        decided_at: now,
    });
    comment.followup_status = status;
    comment.followup_note = note;
    comment.followup_decided_by = Some("human".to_owned());
    comment.updated_at = now;
    comment.status = match status {
        FollowupStatus::Resolved => CommentStatus::Addressed,
        FollowupStatus::Unresolved | FollowupStatus::Regression => CommentStatus::Unresolved,
        FollowupStatus::Stale => CommentStatus::Stale,
        FollowupStatus::Partial | FollowupStatus::Pending => comment.status,
    };
    touch(review, now);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::{SnapshotId, WorktreeId};
    use crate::domain::path::RepoPath;
    use crate::domain::review::Version;

    fn target() -> ReviewTarget {
        ReviewTarget {
            repository: "/work/repo".into(),
            repository_id: "repo123".into(),
            worktree_id: WorktreeId::parse("wt_abc").unwrap(),
            base: "HEAD".into(),
            base_oid: "deadbeef".into(),
            scope: Scope::Worktree,
            snapshot_id: SnapshotId::parse("snap_one").unwrap(),
            session_id: None,
        }
    }

    pub(crate) fn anchor(file: &str) -> Anchor {
        Anchor {
            file_path: RepoPath::new(file).unwrap(),
            module_id: None,
            symbol_id: None,
            symbol_kind: None,
            qualified_name: Some("f".into()),
            symbol_signature: Some("def f()".into()),
            version: Version::After,
            byte_range: None,
            line_range: Some((3, 4)),
            ast_path: vec!["f".into()],
            fingerprint: "fp".into(),
            excerpt: Some("return 1".into()),
        }
    }

    fn ts(n: i64) -> Timestamp {
        Timestamp::from_unix(1_700_000_000 + n).unwrap()
    }

    #[test]
    fn comments_are_sanitised_trimmed_and_bounded() {
        let mut review = new_review(&target(), "aaaaaaaaaaaa", ts(0));
        let id = add_comment(
            &mut review,
            "bbbbbbbbbbbb",
            ts(1),
            anchor("a.py"),
            "  check this\x1b[31m now\x1b]52;c;ZXZpbA==\x07  ",
            Some(Priority::P1),
        )
        .unwrap();
        assert_eq!(review.comment(&id).unwrap().text, "check this now");
        assert!(
            add_comment(
                &mut review,
                "cccccccccccc",
                ts(2),
                anchor("a.py"),
                "   \x1b[0m ",
                None
            )
            .is_err()
        );
        assert!(
            add_comment(
                &mut review,
                "dddddddddddd",
                ts(2),
                anchor("a.py"),
                &"x".repeat(MAX_COMMENT_CHARS + 1),
                None
            )
            .is_err()
        );
        assert!(
            add_comment(
                &mut review,
                "bbbbbbbbbbbb",
                ts(2),
                anchor("a.py"),
                "dup",
                None
            )
            .is_err(),
            "ids are unique"
        );
        assert_eq!(review.comments.len(), 1);
    }

    #[test]
    fn draft_comments_can_be_edited_and_deleted_but_submitted_ones_cannot() {
        let mut review = new_review(&target(), "aaaaaaaaaaaa", ts(0));
        let id = add_comment(
            &mut review,
            "bbbbbbbbbbbb",
            ts(1),
            anchor("a.py"),
            "first",
            None,
        )
        .unwrap();
        edit_comment(&mut review, &id, ts(2), "second", Some(Priority::P2)).unwrap();
        assert_eq!(review.comment(&id).unwrap().text, "second");
        finish(&mut review, "ffffffffffff", ts(3), ".klaub/reviews/x.json").unwrap();
        confirm_submitted(&mut review, ts(4)).unwrap();
        assert!(edit_comment(&mut review, &id, ts(5), "third", None).is_err());
        assert!(delete_comment(&mut review, &id, ts(5)).is_err());
        let second = add_comment(
            &mut review,
            "eeeeeeeeeeee",
            ts(6),
            anchor("a.py"),
            "another",
            None,
        )
        .unwrap();
        delete_comment(&mut review, &second, ts(7)).unwrap();
    }

    #[test]
    fn finishing_is_idempotent_and_needs_comments() {
        let mut review = new_review(&target(), "aaaaaaaaaaaa", ts(0));
        assert!(finish(&mut review, "ffffffffffff", ts(1), "f.json").is_err());
        add_comment(
            &mut review,
            "bbbbbbbbbbbb",
            ts(2),
            anchor("a.py"),
            "one",
            None,
        )
        .unwrap();
        let first = finish(&mut review, "111111111111", ts(3), ".klaub/reviews/r.json").unwrap();
        let again = finish(&mut review, "222222222222", ts(4), ".klaub/reviews/r.json").unwrap();
        assert_eq!(
            first.feedback_id, again.feedback_id,
            "no second payload to paste twice"
        );
        assert_eq!(review.status, ReviewStatus::Finished);
        assert!(
            first.prompt.contains("1 reviewer comment "),
            "{}",
            first.prompt
        );
        assert!(!first.prompt.contains('\n'), "a single line");

        add_comment(
            &mut review,
            "cccccccccccc",
            ts(5),
            anchor("b.py"),
            "two",
            None,
        )
        .unwrap();
        assert_eq!(
            review.status,
            ReviewStatus::Draft,
            "a new comment reopens the review"
        );
        let next = finish(&mut review, "333333333333", ts(6), ".klaub/reviews/r.json").unwrap();
        assert_ne!(next.feedback_id, first.feedback_id);
        assert_eq!(next.comment_ids.len(), 2);
    }

    #[test]
    fn delivery_never_implies_submission() {
        let mut review = new_review(&target(), "aaaaaaaaaaaa", ts(0));
        let id = add_comment(
            &mut review,
            "bbbbbbbbbbbb",
            ts(1),
            anchor("a.py"),
            "one",
            None,
        )
        .unwrap();
        assert!(
            record_delivery(&mut review, Delivery::Copied, ts(2)).is_err(),
            "finish first"
        );
        finish(&mut review, "111111111111", ts(3), "f.json").unwrap();
        record_delivery(&mut review, Delivery::PreparedInAgent, ts(4)).unwrap();
        assert_eq!(review.comment(&id).unwrap().status, CommentStatus::Prepared);
        assert_eq!(review.status, ReviewStatus::Prepared);
        confirm_submitted(&mut review, ts(5)).unwrap();
        assert_eq!(
            review.comment(&id).unwrap().status,
            CommentStatus::Submitted
        );
        assert_eq!(review.status, ReviewStatus::Submitted);
    }

    #[test]
    fn copying_or_showing_the_file_leaves_comments_as_drafts() {
        let mut review = new_review(&target(), "aaaaaaaaaaaa", ts(0));
        let id = add_comment(
            &mut review,
            "bbbbbbbbbbbb",
            ts(1),
            anchor("a.py"),
            "one",
            None,
        )
        .unwrap();
        finish(&mut review, "111111111111", ts(2), "f.json").unwrap();
        record_delivery(&mut review, Delivery::Copied, ts(3)).unwrap();
        assert_eq!(review.comment(&id).unwrap().status, CommentStatus::Draft);
        assert_eq!(review.feedback.as_ref().unwrap().delivery, Delivery::Copied);
    }

    #[test]
    fn reviewed_marks_toggle() {
        let mut review = new_review(&target(), "aaaaaaaaaaaa", ts(0));
        let unit = UnitId::parse("unit_abc").unwrap();
        assert!(toggle_reviewed(&mut review, &unit, ts(1)));
        assert!(review.is_reviewed(&unit));
        assert!(!toggle_reviewed(&mut review, &unit, ts(2)));
        assert!(!review.is_reviewed(&unit));
    }

    #[test]
    fn the_next_round_keeps_comments_and_asks_for_decisions() {
        let mut first = new_review(&target(), "aaaaaaaaaaaa", ts(0));
        let id = add_comment(
            &mut first,
            "bbbbbbbbbbbb",
            ts(1),
            anchor("a.py"),
            "fix the check",
            None,
        )
        .unwrap();
        toggle_reviewed(&mut first, &UnitId::parse("unit_x").unwrap(), ts(2));
        let mut second_target = target();
        second_target.snapshot_id = SnapshotId::parse("snap_two").unwrap();
        let mut second = next_round(&first, "cccccccccccc", ts(10), &second_target);
        assert_eq!(second.previous_review_id.as_ref(), Some(&first.review_id));
        assert!(
            second.reviewed_units.is_empty(),
            "marks referred to the old snapshot"
        );
        let carried = second.comment(&id).unwrap();
        assert_eq!(carried.origin_review_id.as_ref(), Some(&first.review_id));
        assert_eq!(carried.followup_status, FollowupStatus::Pending);

        decide_followup(
            &mut second,
            &id,
            FollowupStatus::Partial,
            Some("still misses the empty case".into()),
            ts(11),
        )
        .unwrap();
        decide_followup(&mut second, &id, FollowupStatus::Resolved, None, ts(12)).unwrap();
        let comment = second.comment(&id).unwrap();
        assert_eq!(
            comment.followups.len(),
            2,
            "history is appended, never overwritten"
        );
        assert_eq!(comment.followup_status, FollowupStatus::Resolved);
        assert_eq!(comment.status, CommentStatus::Addressed);
        assert_eq!(comment.followup_decided_by.as_deref(), Some("human"));
        assert_eq!(
            comment.followups[0].note.as_deref(),
            Some("still misses the empty case")
        );
    }
}
