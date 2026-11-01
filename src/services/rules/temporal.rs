//! Reconciling findings of the base and the target version of a symbol.

use std::collections::HashMap;

use crate::domain::finding::{Finding, Temporal};
use crate::domain::semantic::ChangeKind;

/// Classifies findings as introduced, preexisting, removed or unknown.
///
/// `uncertain` is true when the match between the two versions cannot be trusted (a rewrite, a
/// probable rather than strong match, or syntax errors): differences are then `Unknown`, because
/// calling something "introduced" on a shaky correspondence would be a guess presented as fact.
pub(crate) fn reconcile(
    kind: ChangeKind,
    uncertain: bool,
    before: impl IntoIterator<Item = Finding>,
    after: impl IntoIterator<Item = Finding>,
) -> Vec<Finding> {
    let tag = |mut finding: Finding, temporal| {
        finding.temporal = temporal;
        finding
    };
    match kind {
        ChangeKind::Added => {
            return after
                .into_iter()
                .map(|f| tag(f, Temporal::Introduced))
                .collect();
        }
        ChangeKind::Removed => {
            return before
                .into_iter()
                .map(|f| tag(f, Temporal::Removed))
                .collect();
        }
        _ => {}
    }

    let mut remaining: HashMap<String, Vec<Finding>> = HashMap::new();
    for finding in before {
        remaining
            .entry(finding.fingerprint.clone())
            .or_default()
            .push(finding);
    }
    let mut out = Vec::new();
    for finding in after {
        let seen_before = remaining
            .get_mut(&finding.fingerprint)
            .and_then(Vec::pop)
            .is_some();
        out.push(match (seen_before, uncertain) {
            (true, _) => tag(finding, Temporal::Preexisting),
            (false, true) => tag(finding, Temporal::Unknown),
            (false, false) => tag(finding, Temporal::Introduced),
        });
    }
    let mut gone: Vec<Finding> = remaining.into_values().flatten().collect();
    gone.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
    out.extend(gone.into_iter().map(|f| {
        tag(
            f,
            if uncertain {
                Temporal::Unknown
            } else {
                Temporal::Removed
            },
        )
    }));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::semantic::Confidence;

    fn finding(fingerprint: &str) -> Finding {
        Finding {
            rule_id: "r".into(),
            title: String::new(),
            detail: String::new(),
            evidence: vec![],
            confidence: Confidence::CERTAIN,
            temporal: Temporal::Unknown,
            fingerprint: fingerprint.into(),
            symbol: None,
        }
    }

    fn labels(findings: &[Finding]) -> Vec<(&str, Temporal)> {
        findings
            .iter()
            .map(|f| (f.fingerprint.as_str(), f.temporal))
            .collect()
    }

    #[test]
    fn introduced_preexisting_and_removed() {
        let out = reconcile(
            ChangeKind::Modified,
            false,
            [finding("old"), finding("kept")],
            [finding("kept"), finding("new")],
        );
        let mut got = labels(&out);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("kept", Temporal::Preexisting),
                ("new", Temporal::Introduced),
                ("old", Temporal::Removed)
            ]
        );
    }

    #[test]
    fn duplicates_are_matched_one_to_one() {
        let out = reconcile(
            ChangeKind::Modified,
            false,
            [finding("x")],
            [finding("x"), finding("x")],
        );
        let preexisting = out
            .iter()
            .filter(|f| f.temporal == Temporal::Preexisting)
            .count();
        let introduced = out
            .iter()
            .filter(|f| f.temporal == Temporal::Introduced)
            .count();
        assert_eq!((preexisting, introduced), (1, 1));
    }

    #[test]
    fn uncertain_matches_never_claim_introduced() {
        let out = reconcile(
            ChangeKind::Rewritten,
            true,
            [finding("old"), finding("kept")],
            [finding("kept"), finding("new")],
        );
        let mut got = labels(&out);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("kept", Temporal::Preexisting),
                ("new", Temporal::Unknown),
                ("old", Temporal::Unknown)
            ]
        );
    }

    #[test]
    fn new_and_deleted_symbols() {
        assert_eq!(
            labels(&reconcile(ChangeKind::Added, true, [], [finding("a")])),
            vec![("a", Temporal::Introduced)]
        );
        assert_eq!(
            labels(&reconcile(ChangeKind::Removed, false, [finding("a")], [])),
            vec![("a", Temporal::Removed)]
        );
    }
}
