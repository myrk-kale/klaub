//! # klaub-rules
//!
//! A pure domain service that turns matched changes into an *explainable review queue*.
//!
//! - **Findings** are observations with a stable `rule_id`, evidence and a confidence. They are
//!   computed on **both** versions of a symbol and reconciled into `introduced`, `preexisting`,
//!   `removed` or `unknown` (low-confidence matches and rewrites never claim `introduced`).
//! - **Signals** (contract change, reach through the call graph, sensitive area, complexity, missing
//!   tests, architecture) are *points with a reason*. Every point is itemised in the
//!   [`ScoreBreakdown`], so a reviewer can disagree with it.
//! - **Priority** orders review work. `P0` additionally requires a piece of *critical evidence*, so a
//!   sensitive word alone can never produce it.
//!
//! Nothing here approves, rejects or certifies code.

mod architecture;
mod scoring;
mod sensitive;
mod temporal;

use std::collections::HashMap;

use klaub_core::config::{Config, SortOrder};
use klaub_core::finding::{
    Evidence, Finding, QueueItem, QueueTag, ReviewUnit, ScoreBreakdown, ScoreComponent, Temporal,
    UnitKind,
};
use klaub_core::graph::SymbolKey;
use klaub_core::path::RepoPath;
use klaub_core::semantic::{ChangeKind, Confidence, MatchBand, SymbolChange, file_unit_id};
use klaub_core::snapshot::ChangeStatus;
use klaub_core::symbol::{MarkerKind, Span, Symbol, SymbolKind};
use klaub_core::testing::TestEvidence;
use klaub_graph::{ImpactOptions, ModuleMap, ProjectGraph};

pub use architecture::ArchInput;

/// Rules whose hits are *critical evidence* for `P0` when they are introduced or unknown.
const CRITICAL_RULES: &[&str] = &[
    "risk.sql_from_dynamic_text",
    "risk.secret_literal",
    "risk.dynamic_execution",
    "risk.shell_execution",
    "risk.verification_disabled",
    "arch.forbidden_dependency",
];

/// What a marker kind means as a rule.
struct MarkerRule {
    kind: MarkerKind,
    id: &'static str,
    title: &'static str,
    why: &'static str,
    confidence: f32,
}

const MARKER_RULES: &[MarkerRule] = &[
    MarkerRule {
        kind: MarkerKind::SwallowedError,
        id: "risk.swallowed_error",
        title: "An error is discarded",
        why: "An error path that does nothing hides failures from callers and from operators.",
        confidence: 0.7,
    },
    MarkerRule {
        kind: MarkerKind::RawSqlConcat,
        id: "risk.sql_from_dynamic_text",
        title: "A query is built from dynamic text",
        why: "Concatenated or interpolated SQL can allow injection; parameters keep data out of the statement.",
        confidence: 0.75,
    },
    MarkerRule {
        kind: MarkerKind::SecretLiteral,
        id: "risk.secret_literal",
        title: "A secret-looking literal is in the code",
        why: "Credentials in source are readable by everyone with access to the repository and its history.",
        confidence: 0.6,
    },
    MarkerRule {
        kind: MarkerKind::DynamicExecution,
        id: "risk.dynamic_execution",
        title: "Text is evaluated as code",
        why: "Evaluating text executes whatever it contains; check where the text comes from.",
        confidence: 0.8,
    },
    MarkerRule {
        kind: MarkerKind::ShellExecution,
        id: "risk.shell_execution",
        title: "A shell command is run",
        why: "Shell invocation with interpolated input allows command injection.",
        confidence: 0.75,
    },
    MarkerRule {
        kind: MarkerKind::UnsafeBlock,
        id: "risk.unsafe_block",
        title: "Unsafe code",
        why: "The compiler no longer checks memory safety inside this block; the author upholds the invariants.",
        confidence: 0.9,
    },
    MarkerRule {
        kind: MarkerKind::VerificationDisabled,
        id: "risk.verification_disabled",
        title: "Certificate verification is turned off",
        why: "Without verification a connection can be intercepted.",
        confidence: 0.85,
    },
    MarkerRule {
        kind: MarkerKind::CallInLoop,
        id: "perf.io_in_loop",
        title: "I/O inside a loop",
        why: "One network, file or database round trip per iteration usually scales badly.",
        confidence: 0.55,
    },
];

/// Facts about the file a symbol lives in.
#[derive(Debug, Clone, Copy)]
pub struct FileContext<'a> {
    /// Path after the change (or before, for deleted files).
    pub path: &'a RepoPath,
    /// How Git classified the change.
    pub status: ChangeStatus,
    /// The file is a test file by the configured patterns.
    pub is_test: bool,
    /// The file looks machine-generated.
    pub generated: bool,
    /// One of the versions had syntax errors, so the picture may be partial.
    pub parse_errors: bool,
}

/// A symbol-level change to rate.
#[derive(Debug, Clone, Copy)]
pub struct SymbolInput<'a> {
    /// The file.
    pub file: FileContext<'a>,
    /// The matched change.
    pub change: &'a SymbolChange,
    /// The symbol before.
    pub before: Option<&'a Symbol>,
    /// The symbol after.
    pub after: Option<&'a Symbol>,
}

/// Why a file is reviewed as a whole instead of by symbols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileReason {
    /// The language is not supported; the reason is shown.
    NoSemanticSupport(String),
    /// Binary, oversized or otherwise unreadable content.
    Opaque(String),
    /// The change has no symbol-level content (a rename, a mode change, an empty file).
    NoSymbolChanges,
}

/// A file-level unit.
#[derive(Debug, Clone)]
pub struct FileUnit<'a> {
    /// The file.
    pub file: FileContext<'a>,
    /// Why there are no symbols to review.
    pub reason: FileReason,
}

/// Everything the evaluator needs besides the inputs.
pub struct Evaluator<'a> {
    config: &'a Config,
    graph: Option<&'a ProjectGraph>,
    tests: &'a TestEvidence,
    modules: ModuleMap,
}

/// The output of an evaluation.
#[derive(Debug, Clone, Default)]
pub struct Evaluation {
    /// Queue items in review order.
    pub items: Vec<QueueItem>,
    /// Findings dropped because their rule is disabled in the configuration.
    pub silenced: u32,
}

/// A rule hit on one version of a symbol, before reconciliation with the other version.
struct Hit {
    finding: Finding,
}

impl<'a> Evaluator<'a> {
    /// Creates an evaluator. `graph` may be absent (no call graph, so no reach signals).
    pub fn new(
        config: &'a Config,
        graph: Option<&'a ProjectGraph>,
        tests: &'a TestEvidence,
    ) -> Self {
        Self {
            config,
            graph,
            tests,
            modules: ModuleMap::configured(&config.architecture.modules),
        }
    }

    /// The module assignment in use.
    pub fn modules(&self) -> &ModuleMap {
        &self.modules
    }

    fn enabled(&self, rule_id: &str) -> bool {
        !self.config.rules.disabled.iter().any(|d| d == rule_id)
    }

    /// Rule hits of one version of a symbol.
    fn hits(&self, symbol: &Symbol, file: &RepoPath, silenced: &mut u32) -> Vec<Hit> {
        let key = SymbolKey::new(file.clone(), symbol.qualified_name.clone());
        let mut hits = Vec::new();
        for marker in &symbol.features.markers {
            let Some(rule) = MARKER_RULES.iter().find(|r| r.kind == marker.kind) else {
                continue;
            };
            if !self.enabled(rule.id) {
                *silenced += 1;
                continue;
            }
            hits.push(Hit {
                finding: Finding {
                    rule_id: rule.id.to_owned(),
                    title: rule.title.to_owned(),
                    detail: format!("{} {}", marker.detail, rule.why),
                    evidence: vec![Evidence {
                        file: file.clone(),
                        span: Some(Span {
                            start_byte: 0,
                            end_byte: 0,
                            start_line: marker.line,
                            end_line: marker.line,
                        }),
                        detail: marker.detail.clone(),
                    }],
                    confidence: Confidence::new(rule.confidence),
                    temporal: Temporal::Unknown,
                    fingerprint: format!("{}|{}", rule.id, marker.detail),
                    symbol: Some(key.clone()),
                },
            });
        }
        let threshold = self.config.priority.complexity_threshold;
        if symbol.features.complexity >= threshold && symbol.kind != SymbolKind::Module {
            if self.enabled("complexity.high") {
                hits.push(Hit {
                    finding: Finding {
                        rule_id: "complexity.high".to_owned(),
                        title: "High cyclomatic complexity".to_owned(),
                        detail: format!(
                            "Cyclomatic complexity {} (threshold {threshold}): many independent paths are hard to review and to test.",
                            symbol.features.complexity
                        ),
                        evidence: vec![Evidence {
                            file: file.clone(),
                            span: Some(symbol.span),
                            detail: format!("complexity {}", symbol.features.complexity),
                        }],
                        confidence: Confidence::new(0.9),
                        temporal: Temporal::Unknown,
                        fingerprint: "complexity.high|".to_owned(),
                        symbol: Some(key),
                    },
                });
            } else {
                *silenced += 1;
            }
        }
        hits
    }

    /// Rates one symbol-level change.
    fn symbol_item(&self, input: &SymbolInput<'_>, silenced: &mut u32) -> QueueItem {
        let change = input.change;
        let path_after = change.after.as_ref().map(|r| &r.file);
        let path_before = change.before.as_ref().map(|r| &r.file);
        let before_hits = match (input.before, path_before) {
            (Some(symbol), Some(path)) => self.hits(symbol, path, silenced),
            _ => Vec::new(),
        };
        let after_hits = match (input.after, path_after) {
            (Some(symbol), Some(path)) => self.hits(symbol, path, silenced),
            _ => Vec::new(),
        };
        let thresholds = self.config.analysis.thresholds();
        let uncertain = change.kind == ChangeKind::Rewritten
            || input.file.parse_errors
            || change.confidence.band(thresholds) != MatchBand::Strong;
        let findings = temporal::reconcile(
            change.kind,
            uncertain,
            before_hits.into_iter().map(|h| h.finding),
            after_hits.into_iter().map(|h| h.finding),
        );

        let key = path_after
            .zip(input.after)
            .map(|(p, s)| SymbolKey::new(p.clone(), s.qualified_name.clone()));
        let impact = self.graph.zip(key.as_ref()).and_then(|(graph, key)| {
            let options = ImpactOptions {
                max_depth: self.config.analysis.max_call_depth,
                ..ImpactOptions::default()
            };
            graph.impact(key, &options, &self.modules)
        });
        let linked_tests = key.as_ref().map_or(0, |k| self.tests.links_for(k).count());

        let scored = scoring::score_symbol(&scoring::SymbolFacts {
            config: &self.config.priority,
            change,
            before: input.before,
            after: input.after,
            file: input.file,
            findings: &findings,
            impact: impact.as_ref(),
            linked_tests,
        });

        let unit = ReviewUnit {
            id: change.id.clone(),
            kind: UnitKind::Symbol,
            file: input.file.path.clone(),
            symbol: Some(
                key.clone()
                    .or_else(|| {
                        path_before
                            .zip(input.before)
                            .map(|(p, s)| SymbolKey::new(p.clone(), s.qualified_name.clone()))
                    })
                    .unwrap_or_else(|| {
                        SymbolKey::new(input.file.path.clone(), change.display_name())
                    }),
            ),
            symbol_kind: change
                .after
                .as_ref()
                .or(change.before.as_ref())
                .map(|r| r.kind),
            label: change.display_name().to_owned(),
            semantic: !input.file.parse_errors,
        };
        let mut tags = scored.tags;
        if input.file.parse_errors {
            tags.push(QueueTag::Unknown);
        }
        tags.sort();
        tags.dedup();
        QueueItem {
            unit,
            change: change.kind,
            priority: scored.priority,
            score: scored.breakdown,
            findings,
            tags,
            entrypoints: impact.as_ref().map_or(0, |r| {
                u32::try_from(r.entrypoints.len()).unwrap_or(u32::MAX)
            }),
            scope: impact.as_ref().map(|r| r.scope),
            entry_scope: impact
                .as_ref()
                .and_then(|r| r.entrypoints.iter().map(|e| e.entrypoint.scope).min()),
        }
    }

    /// Rates a file that is reviewed as a whole.
    fn file_item(&self, unit: &FileUnit<'_>, extra: Vec<Finding>, silenced: &mut u32) -> QueueItem {
        let weights = &self.config.priority;
        let path = unit.file.path;
        let mut breakdown = ScoreBreakdown::default();
        let mut tags = Vec::new();
        let mut findings = extra;

        match &unit.reason {
            FileReason::NoSemanticSupport(why) => {
                breakdown.add(
                    ScoreComponent::TechnicalRisk,
                    weights.p2_threshold,
                    format!("no semantic analysis available ({why}): read it as text"),
                    None,
                );
                tags.push(QueueTag::Unknown);
            }
            FileReason::Opaque(why) => {
                breakdown.add(
                    ScoreComponent::TechnicalRisk,
                    weights.p2_threshold,
                    format!("content cannot be shown ({why})"),
                    None,
                );
                tags.push(QueueTag::Unknown);
            }
            FileReason::NoSymbolChanges => {
                breakdown.add(
                    ScoreComponent::TechnicalRisk,
                    weights.p2_threshold / 2,
                    "the file changed but no symbol did (rename, mode, empty or top-level only)",
                    None,
                );
            }
        }
        if let Some((rule_id, title, detail, points)) = scoring::file_rule(path) {
            if self.enabled(rule_id) {
                breakdown.add(
                    ScoreComponent::TechnicalRisk,
                    points,
                    detail.clone(),
                    Some(rule_id),
                );
                findings.push(Finding {
                    rule_id: rule_id.to_owned(),
                    title: title.to_owned(),
                    detail,
                    evidence: vec![Evidence {
                        file: path.clone(),
                        span: None,
                        detail: path.to_string(),
                    }],
                    confidence: Confidence::new(0.8),
                    temporal: Temporal::Introduced,
                    fingerprint: format!("{rule_id}|"),
                    symbol: None,
                });
            } else {
                *silenced += 1;
            }
        }
        for finding in &findings {
            if finding.rule_id == "arch.forbidden_dependency"
                && finding.temporal != Temporal::Removed
            {
                breakdown.add(
                    ScoreComponent::Architecture,
                    weights.architecture_violation,
                    finding.title.clone(),
                    Some(&finding.rule_id),
                );
            } else if finding.rule_id.starts_with("arch.")
                && finding.temporal == Temporal::Introduced
            {
                breakdown.add(
                    ScoreComponent::Architecture,
                    weights.architecture_violation / 2,
                    finding.title.clone(),
                    Some(&finding.rule_id),
                );
            }
        }
        match unit.file.status {
            ChangeStatus::Added => tags.push(QueueTag::NewFile),
            ChangeStatus::Deleted => tags.push(QueueTag::RemovedFile),
            _ => {}
        }
        if unit.file.generated {
            breakdown.add(
                ScoreComponent::Mitigation,
                weights.generated,
                "the file looks generated",
                None,
            );
            tags.push(QueueTag::Generated);
        }
        if unit.file.is_test {
            breakdown.add(
                ScoreComponent::Mitigation,
                weights.test_file,
                "the file is a test",
                None,
            );
        }
        let critical = findings.iter().any(|f| {
            CRITICAL_RULES.contains(&f.rule_id.as_str()) && f.temporal != Temporal::Removed
        });
        let priority = scoring::priority(breakdown.total(), critical, weights);
        tags.sort();
        tags.dedup();
        QueueItem {
            unit: ReviewUnit {
                id: file_unit_id(path),
                kind: UnitKind::File,
                file: path.clone(),
                symbol: None,
                symbol_kind: None,
                label: path.to_string(),
                semantic: matches!(unit.reason, FileReason::NoSymbolChanges),
            },
            change: match unit.file.status {
                ChangeStatus::Added => ChangeKind::Added,
                ChangeStatus::Deleted => ChangeKind::Removed,
                ChangeStatus::Renamed | ChangeStatus::Copied => ChangeKind::Moved,
                ChangeStatus::Modified | ChangeStatus::TypeChanged => ChangeKind::Unknown,
            },
            priority,
            score: breakdown,
            findings,
            tags,
            entrypoints: 0,
            scope: None,
            entry_scope: None,
        }
    }

    /// Evaluates every unit and returns the queue in review order.
    pub fn evaluate(
        &self,
        symbols: &[SymbolInput<'_>],
        files: &[FileUnit<'_>],
        arch: &[ArchInput<'_>],
    ) -> Evaluation {
        let mut silenced = 0;
        let mut items: Vec<QueueItem> = symbols
            .iter()
            .map(|s| self.symbol_item(s, &mut silenced))
            .collect();

        let mut arch_findings: HashMap<RepoPath, Vec<Finding>> = HashMap::new();
        if let Some(graph) = self.graph {
            for (path, findings) in architecture::evaluate(self.config, graph, &self.modules, arch)
            {
                arch_findings
                    .entry(path)
                    .or_default()
                    .extend(findings.into_iter().filter(|f| {
                        let keep = self.enabled(&f.rule_id);
                        if !keep {
                            silenced += 1;
                        }
                        keep
                    }));
            }
        }
        for unit in files {
            let extra = arch_findings.remove(unit.file.path).unwrap_or_default();
            items.push(self.file_item(unit, extra, &mut silenced));
        }
        // Architecture findings of files that otherwise have only symbol units get a unit of their own.
        let mut remaining: Vec<(RepoPath, Vec<Finding>)> = arch_findings
            .into_iter()
            .filter(|(_, f)| !f.is_empty())
            .collect();
        remaining.sort_by(|a, b| a.0.cmp(&b.0));
        for (path, findings) in remaining {
            let Some(input) = arch.iter().find(|a| a.path == &path) else {
                continue;
            };
            let unit = FileUnit {
                file: FileContext {
                    path: input.path,
                    status: input.status,
                    is_test: input.is_test,
                    generated: false,
                    parse_errors: false,
                },
                reason: FileReason::NoSymbolChanges,
            };
            let mut item = self.file_item(&unit, findings, &mut silenced);
            // The file's symbols are rated on their own; this unit exists for its dependencies only.
            item.unit.label = format!("{path} (dependencies)");
            item.score
                .parts
                .retain(|p| p.component != ScoreComponent::TechnicalRisk);
            item.priority = scoring::priority(
                item.score.total(),
                item.findings
                    .iter()
                    .any(|f| CRITICAL_RULES.contains(&f.rule_id.as_str())),
                &self.config.priority,
            );
            items.push(item);
        }

        sort_queue(&mut items, self.config.review.sort, &self.modules);
        Evaluation { items, silenced }
    }
}

/// Orders the queue.
///
/// [`SortOrder::Priority`] puts the higher priority first, then units with more introduced
/// findings and a higher score. [`SortOrder::Execution`] puts a unit reachable from more
/// entrypoints first ([`QueueItem::entrypoints`]) and breaks the remaining ties the same way.
/// [`SortOrder::Module`] groups by module, then uses the priority ordering.
pub fn sort_queue(items: &mut [QueueItem], order: SortOrder, modules: &ModuleMap) {
    let introduced = |i: &QueueItem| {
        i.findings
            .iter()
            .filter(|f| f.temporal == Temporal::Introduced)
            .count()
    };
    items.sort_by(|a, b| {
        let by_priority = a
            .priority
            .cmp(&b.priority)
            .then(introduced(b).cmp(&introduced(a)))
            .then(b.score.total().cmp(&a.score.total()))
            .then(a.unit.file.cmp(&b.unit.file))
            .then(a.unit.label.cmp(&b.unit.label));
        match order {
            SortOrder::Priority => by_priority,
            SortOrder::Execution => b.entrypoints.cmp(&a.entrypoints).then(by_priority),
            SortOrder::Module => modules
                .module_of(&a.unit.file)
                .cmp(&modules.module_of(&b.unit.file))
                .then(by_priority),
        }
    });
}

/// Whether `path` is a test file by the configured patterns.
pub fn is_test_path(config: &Config, path: &RepoPath) -> bool {
    let tests = &config.tests;
    [
        &tests.python_patterns,
        &tests.js_patterns,
        &tests.rust_patterns,
        &tests.go_patterns,
        &tests.java_patterns,
    ]
    .into_iter()
    .flatten()
    .any(|pattern| klaub_core::glob::matches(pattern, path.as_str()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::sort_queue;
    use klaub_core::config::SortOrder;
    use klaub_core::finding::{Priority, QueueItem, ReviewUnit, ScoreBreakdown, UnitKind};
    use klaub_core::path::RepoPath;
    use klaub_core::semantic::{ChangeKind, file_unit_id};
    use klaub_graph::ModuleMap;

    /// A queue item whose only interesting facts are its priority and how many entrypoints reach it.
    fn item(path: &str, priority: Priority, entrypoints: u32) -> QueueItem {
        let file = RepoPath::new(path).unwrap();
        QueueItem {
            unit: ReviewUnit {
                id: file_unit_id(&file),
                kind: UnitKind::File,
                file: file.clone(),
                symbol: None,
                symbol_kind: None,
                label: file.to_string(),
                semantic: true,
            },
            change: ChangeKind::Modified,
            priority,
            score: ScoreBreakdown::default(),
            findings: Vec::new(),
            tags: Vec::new(),
            entrypoints,
            scope: None,
            entry_scope: None,
        }
    }

    #[test]
    fn execution_order_ranks_reach_before_priority() {
        let wide = item("src/wide.py", Priority::P2, 4);
        let urgent = item("src/urgent.py", Priority::P0, 1);
        let modules = ModuleMap::default();

        let mut by_priority = [wide.clone(), urgent.clone()];
        sort_queue(&mut by_priority, SortOrder::Priority, &modules);
        assert_eq!(by_priority[0].unit.label, "src/urgent.py");

        let mut by_execution = [wide, urgent];
        sort_queue(&mut by_execution, SortOrder::Execution, &modules);
        assert_eq!(
            by_execution[0].unit.label, "src/wide.py",
            "more entrypoints rank first, ahead of a higher priority"
        );
        assert_eq!(by_execution[1].unit.label, "src/urgent.py");
    }
}
