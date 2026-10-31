//! Exports: the JSON schema of `klaub reviews show --format json` and a Markdown rendering.

use klaub_core::finding::{FindingDelta, Priority};
use klaub_core::review::{CommentStatus, CriterionStatus, FlowRef, Review};
use klaub_core::text::sanitize_terminal_text;
use serde_json::{Value, json};

/// Facts computed elsewhere that the export includes.
#[derive(Debug, Clone, Default)]
pub struct ExportContext {
    /// Relative path of the review file.
    pub comments_file: String,
    /// The worktree changed after the snapshot was opened.
    pub snapshot_changed_since_open: bool,
    /// `(reviewed, total, unknown)` units, when a coverage was computed.
    pub coverage: Option<(usize, usize, usize)>,
    /// Findings by temporal class, when computed.
    pub finding_delta: Option<FindingDelta>,
    /// `(linked, passed, failed, not_run)` test evidence, when computed.
    pub test_evidence: Option<(usize, usize, usize, usize)>,
    /// State of the flow diagram.
    pub flow: Option<FlowRef>,
}

fn criterion_status(status: CriterionStatus) -> &'static str {
    match status {
        CriterionStatus::NotChecked => "not_checked",
        CriterionStatus::Verified => "verified",
        CriterionStatus::Pending => "pending",
        CriterionStatus::NotApplicable => "not_applicable",
    }
}

/// The export document (`schema_version: 1`). Consumers must ignore unknown fields.
pub fn export_json(review: &Review, context: &ExportContext) -> Value {
    let status = serde_json::to_value(review.status).unwrap_or(Value::Null);
    json!({
        "schema_version": 1,
        "review_id": review.review_id,
        "status": status,
        "repository": review.repository,
        "base": review.base,
        "snapshot_id": review.snapshot_id,
        "comments_file": context.comments_file,
        "comment_count": review.comments.len(),
        "snapshot_changed_since_open": context.snapshot_changed_since_open,
        "intent_summary": review.brief.intent_summary,
        "task_ref": review.brief.task_ref,
        "acceptance_criteria": review.brief.acceptance_criteria.iter().map(|c| json!({
            "id": c.id, "text": c.text, "status": criterion_status(c.status), "note": c.note,
        })).collect::<Vec<_>>(),
        "review_coverage": context.coverage.map(|(reviewed, total, unknown)| json!({
            "reviewed_symbols": reviewed, "total_symbols": total, "unknown_units": unknown,
        })),
        "finding_delta": context.finding_delta,
        "test_evidence": context.test_evidence.map(|(linked, passed, failed, not_run)| json!({
            "linked": linked, "passed": passed, "failed": failed, "not_run": not_run,
        })),
        "flow": context.flow,
        "comments": review.comments,
    })
}

fn line(text: &str) -> String {
    sanitize_terminal_text(text).replace('\n', " ")
}

/// The review as a [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html)
/// log, so hosts that read SARIF (code scanning views, editors) can show the comments in place.
///
/// Every comment becomes one result of the single rule `klaub.review-comment`, anchored to its file
/// and, when it has one, its line range. SARIF levels describe code defects and a comment is not
/// one, so the mapping is a hint about attention, not a verdict: `P0` is `error`, `P1` is
/// `warning`, everything else (including a comment without a priority) is `note`. The exact
/// priority, the version the lines refer to (`before` or `after`), the follow-up decision and the
/// anchor's confidence travel in `properties`. A comment on the old version points at lines of the
/// old file; `properties.version` says so.
pub fn export_sarif(review: &Review) -> Value {
    let results: Vec<Value> = review
        .comments
        .iter()
        .map(|comment| {
            let level = match comment.severity {
                Some(Priority::P0) => "error",
                Some(Priority::P1) => "warning",
                _ => "note",
            };
            let mut physical = json!({
                "artifactLocation": { "uri": comment.anchor.file_path.as_str() },
            });
            if let Some((start, end)) = comment.anchor.line_range {
                physical["region"] = json!({ "startLine": start, "endLine": end.max(start) });
            }
            json!({
                "ruleId": SARIF_RULE,
                "level": level,
                "message": { "text": comment.text },
                "locations": [{ "physicalLocation": physical }],
                "partialFingerprints": { "klaubAnchor/v1": comment.anchor.fingerprint },
                "properties": {
                    "comment_id": comment.comment_id,
                    "priority": comment.severity.map(Priority::label),
                    "version": comment.anchor.version,
                    "symbol": comment.anchor.qualified_name,
                    "status": comment.status,
                    "followup_status": comment.followup_status,
                    "anchor_confidence": comment.anchor_confidence,
                },
            })
        })
        .collect();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": "Klaub",
                "version": env!("CARGO_PKG_VERSION"),
                "informationUri": "https://github.com/myrk-kale/klaub",
                "rules": [{
                    "id": SARIF_RULE,
                    "name": "ReviewComment",
                    "shortDescription": { "text": "A comment written by a human reviewer" },
                    "help": { "text": "Written by a person reading the code in Klaub. The level is a hint about attention, not a defect classification." },
                }],
            }},
            "automationDetails": { "id": review.review_id },
            "results": results,
        }],
    })
}

const SARIF_RULE: &str = "klaub.review-comment";

/// A Markdown rendering of the review, for pull request descriptions or issue trackers.
pub fn export_markdown(review: &Review) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Review {}\n\n", review.review_id));
    out.push_str(&format!(
        "- Snapshot: `{}` (base `{}`)\n- Comments: {}\n",
        review.snapshot_id,
        line(&review.base),
        review.comments.len()
    ));
    if let Some(intent) = &review.brief.intent_summary {
        out.push_str(&format!("\n## Intent\n\n{}\n", line(intent)));
    }
    if !review.brief.acceptance_criteria.is_empty() {
        out.push_str("\n## Acceptance criteria\n\n");
        for criterion in &review.brief.acceptance_criteria {
            let mark = match criterion.status {
                CriterionStatus::Verified => "x",
                _ => " ",
            };
            out.push_str(&format!(
                "- [{mark}] {} _({})_\n",
                line(&criterion.text),
                criterion_status(criterion.status)
            ));
        }
    }
    out.push_str("\n## Comments\n");
    let mut by_file: Vec<(&str, Vec<&klaub_core::review::Comment>)> = Vec::new();
    for comment in &review.comments {
        let file = comment.anchor.file_path.as_str();
        match by_file.iter_mut().find(|(f, _)| *f == file) {
            Some((_, list)) => list.push(comment),
            None => by_file.push((file, vec![comment])),
        }
    }
    by_file.sort_by_key(|(file, _)| *file);
    for (file, comments) in by_file {
        out.push_str(&format!("\n### `{file}`\n\n"));
        for comment in comments {
            let place = match (&comment.anchor.qualified_name, comment.anchor.line_range) {
                (Some(name), Some((start, end))) if start != end => {
                    format!("`{}` lines {start}–{end}", line(name))
                }
                (Some(name), Some((start, _))) => format!("`{}` line {start}", line(name)),
                (Some(name), None) => format!("`{}`", line(name)),
                (None, Some((start, _))) => format!("line {start}"),
                (None, None) => "file".to_owned(),
            };
            let severity = comment
                .severity
                .map_or(String::new(), |s| format!(" **{}**", s.label()));
            let state = match comment.status {
                CommentStatus::Draft => "",
                CommentStatus::Prepared => " (prepared)",
                CommentStatus::Submitted => " (submitted)",
                CommentStatus::Addressed => " (addressed)",
                CommentStatus::Unresolved => " (unresolved)",
                CommentStatus::Stale => " (stale)",
            };
            out.push_str(&format!(
                "- {place}{severity}{state}: {}\n",
                line(&comment.text)
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::anchor;
    use crate::{ReviewTarget, add_comment, new_review};
    use klaub_core::ids::{SnapshotId, WorktreeId};
    use klaub_core::snapshot::Scope;
    use klaub_core::timestamp::Timestamp;

    fn review() -> Review {
        let target = ReviewTarget {
            repository: "/r".into(),
            repository_id: "r".into(),
            worktree_id: WorktreeId::parse("wt_a").unwrap(),
            base: "HEAD".into(),
            base_oid: "abc".into(),
            scope: Scope::Worktree,
            snapshot_id: SnapshotId::parse("snap_a").unwrap(),
            session_id: None,
        };
        let mut review = new_review(&target, "aaaaaaaaaaaa", Timestamp::from_unix(1).unwrap());
        add_comment(
            &mut review,
            "bbbbbbbbbbbb",
            Timestamp::from_unix(2).unwrap(),
            anchor("src/a.py"),
            "handle the empty case",
            Some(Priority::P1),
        )
        .unwrap();
        add_comment(
            &mut review,
            "cccccccccccc",
            Timestamp::from_unix(3).unwrap(),
            anchor("src/a.py"),
            "rename this",
            None,
        )
        .unwrap();
        review
    }

    #[test]
    fn the_json_export_follows_the_documented_shape() {
        let review = review();
        let context = ExportContext {
            comments_file: ".klaub/reviews/r.json".into(),
            coverage: Some((12, 18, 2)),
            finding_delta: Some(FindingDelta {
                introduced: 2,
                preexisting: 3,
                removed: 1,
                unknown: 1,
            }),
            test_evidence: Some((5, 0, 0, 5)),
            ..ExportContext::default()
        };
        let value = export_json(&review, &context);
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["comment_count"], 2);
        assert_eq!(value["status"], "draft");
        assert_eq!(value["review_coverage"]["reviewed_symbols"], 12);
        assert_eq!(value["finding_delta"]["introduced"], 2);
        assert_eq!(value["test_evidence"]["not_run"], 5);
        assert_eq!(value["comments"].as_array().unwrap().len(), 2);
        assert_eq!(value["comments"][0]["severity"], "P1");
        assert!(value["flow"].is_null());
    }

    #[test]
    fn markdown_groups_by_file_and_sanitises() {
        let mut review = review();
        review.comments[1].text = "tricky \x1b[31mred\x1b[0m\nsecond line".into();
        let markdown = export_markdown(&review);
        assert!(markdown.contains("### `src/a.py`"));
        assert!(markdown.contains("**P1**: handle the empty case"));
        assert!(
            !markdown.contains('\x1b'),
            "terminal sequences never reach a document"
        );
        assert!(markdown.contains("tricky red second line"));
    }

    #[test]
    fn sarif_has_one_result_per_comment_with_a_mapped_level_and_the_exact_priority() {
        let log = export_sarif(&review());
        assert_eq!(log["version"], "2.1.0");
        let results = log["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);

        let first = &results[0];
        assert_eq!(first["ruleId"], "klaub.review-comment");
        assert_eq!(first["level"], "warning", "P1 maps to warning");
        assert_eq!(first["message"]["text"], "handle the empty case");
        assert_eq!(first["properties"]["priority"], "P1");
        assert_eq!(
            first["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/a.py"
        );
        assert!(first["partialFingerprints"]["klaubAnchor/v1"].is_string());

        let second = &results[1];
        assert_eq!(
            second["level"], "note",
            "no priority is a note, not a defect"
        );
        assert!(second["properties"]["priority"].is_null());
    }

    #[test]
    fn sarif_declares_its_one_rule_and_says_the_level_is_not_a_verdict() {
        let log = export_sarif(&review());
        let driver = &log["runs"][0]["tool"]["driver"];
        assert_eq!(driver["name"], "Klaub");
        assert_eq!(driver["rules"].as_array().unwrap().len(), 1);
        assert!(
            driver["rules"][0]["help"]["text"]
                .as_str()
                .unwrap()
                .contains("not a defect classification")
        );
    }
}
