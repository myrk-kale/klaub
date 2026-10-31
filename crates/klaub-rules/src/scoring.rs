//! The scoring model: itemised points with reasons, and the priority they add up to.

use klaub_core::config::PriorityConfig;
use klaub_core::finding::{Finding, Priority, QueueTag, ScoreBreakdown, ScoreComponent, Temporal};
use klaub_core::graph::{EntrypointScope, ImpactReport, ImpactScope};
use klaub_core::path::RepoPath;
use klaub_core::semantic::ChangeKind;
use klaub_core::symbol::{Symbol, SymbolKind};

use crate::sensitive::sensitive_word;
use crate::{CRITICAL_RULES, FileContext};
use klaub_core::semantic::SymbolChange;

pub(crate) struct SymbolFacts<'a> {
    pub(crate) config: &'a PriorityConfig,
    pub(crate) change: &'a SymbolChange,
    pub(crate) before: Option<&'a Symbol>,
    pub(crate) after: Option<&'a Symbol>,
    pub(crate) file: FileContext<'a>,
    pub(crate) findings: &'a [Finding],
    pub(crate) impact: Option<&'a ImpactReport>,
    pub(crate) linked_tests: usize,
}

pub(crate) struct Scored {
    pub(crate) breakdown: ScoreBreakdown,
    pub(crate) priority: Priority,
    pub(crate) tags: Vec<QueueTag>,
}

/// Turns a total into a priority. `P0` also needs critical evidence.
pub(crate) fn priority(score: i32, critical: bool, weights: &PriorityConfig) -> Priority {
    if score >= weights.p0_threshold && critical {
        Priority::P0
    } else if score >= weights.p1_threshold {
        Priority::P1
    } else if score >= weights.p2_threshold {
        Priority::P2
    } else {
        Priority::P3
    }
}

/// Rules that depend only on a file's name: dependency manifests and migrations.
pub(crate) fn file_rule(path: &RepoPath) -> Option<(&'static str, &'static str, String, i32)> {
    let name = path.file_name();
    let manifest = matches!(
        name,
        "package.json"
            | "pyproject.toml"
            | "Pipfile"
            | "Cargo.toml"
            | "go.mod"
            | "Gemfile"
            | "pom.xml"
            | "build.gradle"
            | "composer.json"
    ) || (name.starts_with("requirements")
        && std::path::Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("txt")));
    if manifest {
        return Some((
            "risk.dependency_manifest",
            "Dependencies changed",
            format!(
                "{name} declares what the project depends on; new or upgraded packages are code you did not read."
            ),
            15,
        ));
    }
    let lower = path.as_str().to_ascii_lowercase();
    if lower.contains("migrations/")
        || lower.contains("alembic/versions/")
        || std::path::Path::new(&lower)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("sql"))
    {
        return Some((
            "risk.migration",
            "Schema or data migration",
            "Migrations change data that cannot be reverted by reverting the code.".to_owned(),
            20,
        ));
    }
    None
}

fn contract(facts: &SymbolFacts<'_>) -> Option<(i32, String)> {
    let weight = facts.config.contract_change;
    let (before, after) = (facts.before, facts.after);
    if after.or(before).is_some_and(|s| s.features.is_test) {
        return None;
    }
    match facts.change.kind {
        ChangeKind::SignatureChanged => {
            let public =
                after.is_some_and(Symbol::is_public) || before.is_some_and(Symbol::is_public);
            Some(if public {
                (
                    weight,
                    "the declaration of a public symbol changed".to_owned(),
                )
            } else {
                (weight / 2, "the declaration changed".to_owned())
            })
        }
        ChangeKind::Removed
            if before.is_some_and(Symbol::is_public)
                && before.is_some_and(|s| s.kind != SymbolKind::Module) =>
        {
            Some((weight, "a public symbol was removed".to_owned()))
        }
        ChangeKind::Added
            if after.is_some_and(Symbol::is_public)
                && after.is_some_and(|s| s.parent == Some(0) && s.kind != SymbolKind::Module) =>
        {
            Some((weight / 2, "a new public symbol extends the API".to_owned()))
        }
        ChangeKind::Moved | ChangeKind::Renamed if after.is_some_and(Symbol::is_public) => Some((
            weight / 2,
            "a public symbol moved or was renamed".to_owned(),
        )),
        _ => None,
    }
}

pub(crate) fn score_symbol(facts: &SymbolFacts<'_>) -> Scored {
    let weights = facts.config;
    let kind = facts.change.kind;
    let symbol = facts.after.or(facts.before);
    let mut breakdown = ScoreBreakdown::default();
    let mut tags = Vec::new();
    let mut critical = false;

    let contract = contract(facts);
    if let Some((points, reason)) = &contract {
        breakdown.add(
            ScoreComponent::Contract,
            *points,
            reason.clone(),
            Some("contract.change"),
        );
        tags.push(QueueTag::Contract);
    }

    // Reach through the call graph.
    let mut external = false;
    if let Some(report) = facts.impact {
        external = report
            .entrypoints
            .iter()
            .any(|e| e.entrypoint.scope == EntrypointScope::External);
        if external {
            breakdown.add(
                ScoreComponent::Reach,
                weights.entrypoint_external,
                "an external entrypoint (route, command, main) can reach it",
                Some("reach.entrypoint"),
            );
        } else if !report.entrypoints.is_empty() {
            breakdown.add(
                ScoreComponent::Reach,
                weights.entrypoint_internal,
                "an exported symbol can reach it",
                Some("reach.entrypoint"),
            );
        }
        if u32::try_from(report.direct_callers()).unwrap_or(u32::MAX) >= weights.fan_in_threshold {
            breakdown.add(
                ScoreComponent::Reach,
                weights.fan_in,
                format!("{} direct callers", report.direct_callers()),
                Some("reach.fan_in"),
            );
        }
        match report.scope {
            ImpactScope::CrossModule => breakdown.add(
                ScoreComponent::Reach,
                weights.cross_module,
                "callers live in other modules",
                Some("reach.cross_module"),
            ),
            ImpactScope::Global => breakdown.add(
                ScoreComponent::Reach,
                weights.global_scope,
                "it reaches several modules or important entrypoints",
                Some("reach.global"),
            ),
            ImpactScope::Local | ImpactScope::Module => {}
        }
    }

    // Sensitive area: attention, never certainty.
    let decorators: Vec<&str> = symbol
        .map(|s| s.decorators.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let sensitive = sensitive_word(
        std::iter::once(facts.file.path.as_str())
            .chain(symbol.map(|s| s.qualified_name.as_str()))
            .chain(decorators),
    );
    if let Some(word) = &sensitive
        && kind != ChangeKind::FormatOnly
    {
        breakdown.add(
            ScoreComponent::Criticality,
            weights.criticality,
            format!("touches a sensitive area (`{word}`)"),
            Some("criticality.sensitive_area"),
        );
    }

    // Complexity.
    if let Some(after) = facts.after {
        let now = after.features.complexity;
        if now >= weights.complexity_threshold {
            breakdown.add(
                ScoreComponent::Complexity,
                weights.complexity,
                format!("cyclomatic complexity {now}"),
                Some("complexity.high"),
            );
        }
        if let Some(before) = facts.before {
            let was = before.features.complexity;
            if now >= was + 5 && now * 10 >= was * 13 {
                breakdown.add(
                    ScoreComponent::Complexity,
                    weights.complexity_growth,
                    format!("complexity grew from {was} to {now}"),
                    Some("complexity.growth"),
                );
            }
        }
    }

    // Technical risk: every finding on the target version counts (introduced ones sort first in
    // the queue, but a preexisting danger is not discounted). Critical rules weigh double.
    let mut risk_so_far = 0;
    for finding in facts
        .findings
        .iter()
        .filter(|f| f.temporal != Temporal::Removed)
    {
        let is_critical = CRITICAL_RULES.contains(&finding.rule_id.as_str());
        let wanted = if is_critical {
            weights.risk_critical
        } else if finding.rule_id.starts_with("risk.") {
            weights.risk_per_finding
        } else if finding.rule_id.starts_with("perf.") {
            weights.risk_per_finding / 2
        } else {
            0
        };
        let granted = wanted.min((weights.risk_cap - risk_so_far).max(0));
        risk_so_far += granted;
        breakdown.add(
            ScoreComponent::TechnicalRisk,
            granted,
            format!("{} ({})", finding.title, finding.temporal.label()),
            Some(&finding.rule_id),
        );
        critical |= is_critical;
    }

    // Tests: the absence of an association is a signal, not a fact about the code.
    let callable = symbol.is_some_and(|s| s.kind.is_callable());
    if callable
        && facts.linked_tests == 0
        && kind != ChangeKind::FormatOnly
        && kind != ChangeKind::Removed
        && !symbol.is_some_and(|s| s.features.is_test)
        && !facts.file.is_test
    {
        breakdown.add(
            ScoreComponent::TestSignals,
            weights.no_tests,
            "no associated test was found (that does not prove there is none)",
            Some("tests.no_association"),
        );
        tags.push(QueueTag::NoTests);
    }

    // Mitigations.
    match kind {
        ChangeKind::FormatOnly => {
            breakdown.add(
                ScoreComponent::Mitigation,
                weights.format_only,
                "only whitespace or comments changed",
                Some("mitigation.format_only"),
            );
            tags.push(QueueTag::FormatOnly);
        }
        ChangeKind::Moved | ChangeKind::Renamed => {
            tags.push(QueueTag::Moved);
            let unchanged = facts
                .before
                .zip(facts.after)
                .is_some_and(|(b, a)| b.fingerprint.token_hash == a.fingerprint.token_hash);
            if unchanged {
                breakdown.add(
                    ScoreComponent::Mitigation,
                    weights.moved,
                    "moved or renamed without changing the code",
                    Some("mitigation.moved"),
                );
            }
        }
        ChangeKind::Rewritten => tags.push(QueueTag::Rewrite),
        _ => {}
    }
    match facts.file.status {
        klaub_core::snapshot::ChangeStatus::Added => tags.push(QueueTag::NewFile),
        klaub_core::snapshot::ChangeStatus::Deleted => tags.push(QueueTag::RemovedFile),
        _ => {}
    }
    if facts.file.is_test {
        breakdown.add(
            ScoreComponent::Mitigation,
            weights.test_file,
            "the file is a test",
            Some("mitigation.test_file"),
        );
    }
    if facts.file.generated {
        breakdown.add(
            ScoreComponent::Mitigation,
            weights.generated,
            "the file looks generated",
            Some("mitigation.generated"),
        );
        tags.push(QueueTag::Generated);
    }

    // Critical evidence beyond findings.
    if contract.is_some() && external {
        critical = true;
    }
    if contract.is_some() && sensitive.is_some() {
        critical = true;
    }
    if kind == ChangeKind::FormatOnly {
        critical = false;
    }
    let priority = priority(breakdown.total(), critical, weights);
    Scored {
        breakdown,
        priority,
        tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p0_needs_critical_evidence() {
        let weights = PriorityConfig::default();
        assert_eq!(
            priority(95, false, &weights),
            Priority::P1,
            "a high score alone is capped at P1"
        );
        assert_eq!(priority(95, true, &weights), Priority::P0);
        assert_eq!(priority(45, false, &weights), Priority::P1);
        assert_eq!(priority(20, false, &weights), Priority::P2);
        assert_eq!(priority(19, true, &weights), Priority::P3);
    }

    #[test]
    fn file_rules_look_at_names() {
        let rule = |p: &str| file_rule(&RepoPath::new(p).unwrap()).map(|r| r.0);
        assert_eq!(rule("web/package.json"), Some("risk.dependency_manifest"));
        assert_eq!(
            rule("requirements-dev.txt"),
            Some("risk.dependency_manifest")
        );
        assert_eq!(
            rule("db/migrations/0042_add_users.py"),
            Some("risk.migration")
        );
        assert_eq!(rule("schema.sql"), Some("risk.migration"));
        assert_eq!(rule("src/main.rs"), None);
    }
}
