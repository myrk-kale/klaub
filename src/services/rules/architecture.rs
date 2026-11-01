//! Architecture rules: forbidden dependencies, new cycles and sudden coupling.

use std::collections::{HashMap, HashSet};

use crate::domain::config::Config;
use crate::domain::finding::{Evidence, Finding, Temporal};
use crate::domain::path::RepoPath;
use crate::domain::semantic::Confidence;
use crate::domain::snapshot::ChangeStatus;
use crate::domain::symbol::ParsedFile;
use crate::services::graph::{ModuleMap, ProjectGraph};

/// New project imports in one file above which coupling is worth a look.
const COUPLING_THRESHOLD: usize = 5;

/// A changed file with the parses of both versions.
#[derive(Debug, Clone, Copy)]
pub struct ArchInput<'a> {
    /// Path after the change.
    pub path: &'a RepoPath,
    /// Path before the change, when it differs.
    pub old_path: Option<&'a RepoPath>,
    /// How Git classified the change.
    pub status: ChangeStatus,
    /// The file is a test.
    pub is_test: bool,
    /// Parse of the previous version.
    pub before: Option<&'a ParsedFile>,
    /// Parse of the new version.
    pub after: Option<&'a ParsedFile>,
}

fn finding(
    rule_id: &str,
    title: &str,
    detail: String,
    file: &RepoPath,
    temporal: Temporal,
    key: &str,
) -> Finding {
    Finding {
        rule_id: rule_id.to_owned(),
        title: title.to_owned(),
        evidence: vec![Evidence {
            file: file.clone(),
            span: None,
            detail: detail.clone(),
        }],
        detail,
        confidence: Confidence::new(0.8),
        temporal,
        fingerprint: format!("{rule_id}|{key}"),
        symbol: None,
    }
}

/// Evaluates architecture rules for the changed files.
pub(crate) fn evaluate(
    config: &Config,
    graph: &ProjectGraph,
    modules: &ModuleMap,
    inputs: &[ArchInput<'_>],
) -> HashMap<RepoPath, Vec<Finding>> {
    let mut out: HashMap<RepoPath, Vec<Finding>> = HashMap::new();
    let cycles = graph.module_cycles(modules);
    for input in inputs.iter().filter(|i| !i.is_test) {
        let Some(after) = input.after else { continue };
        let after_imports: HashSet<RepoPath> = graph
            .imported_files(input.path, after)
            .into_iter()
            .collect();
        let before_imports: Option<HashSet<RepoPath>> = input.before.map(|b| {
            graph
                .imported_files(input.old_path.unwrap_or(input.path), b)
                .into_iter()
                .collect()
        });
        let is_new = |target: &RepoPath| {
            before_imports
                .as_ref()
                .is_none_or(|before| !before.contains(target))
        };

        // Forbidden dependencies, judged against the previous imports of the same file.
        for rule in &config.architecture.forbidden_dependencies {
            if !input.path.is_within(&rule.from) {
                continue;
            }
            for target in after_imports.iter().filter(|t| t.is_within(&rule.to)) {
                let temporal = match (&before_imports, is_new(target)) {
                    (Some(_), false) => Temporal::Preexisting,
                    (Some(_), true) => Temporal::Introduced,
                    (None, _) if input.status == ChangeStatus::Added => Temporal::Introduced,
                    (None, _) => Temporal::Unknown,
                };
                out.entry(input.path.clone()).or_default().push(finding(
                    "arch.forbidden_dependency",
                    "A forbidden dependency",
                    format!(
                        "`{}` must not depend on `{}`, but imports {target}.",
                        rule.from, rule.to
                    ),
                    input.path,
                    temporal,
                    &format!("{}->{}:{target}", rule.from, rule.to),
                ));
            }
        }

        // A cycle is introduced when this file adds an import that closes one.
        let own_module = modules.module_of(input.path);
        for cycle in cycles.iter().filter(|c| c.contains(&own_module)) {
            let closing: Vec<&RepoPath> = after_imports
                .iter()
                .filter(|t| is_new(t))
                .filter(|t| {
                    let module = modules.module_of(t);
                    module != own_module && cycle.contains(&module)
                })
                .collect();
            if let Some(first) = closing.first() {
                out.entry(input.path.clone()).or_default().push(finding(
                    "arch.module_cycle",
                    "A dependency cycle between modules",
                    format!("`{}` and the modules it imports now depend on each other: {} (new import: {first}).", own_module, cycle.join(" ↔ ")),
                    input.path,
                    Temporal::Introduced,
                    &cycle.join(","),
                ));
            }
        }

        // Many new dependencies at once.
        let added: Vec<&RepoPath> = after_imports.iter().filter(|t| is_new(t)).collect();
        if before_imports.is_some() && added.len() >= COUPLING_THRESHOLD {
            out.entry(input.path.clone()).or_default().push(finding(
                "arch.coupling",
                "Many new dependencies",
                format!(
                    "{} new project imports were added to one file.",
                    added.len()
                ),
                input.path,
                Temporal::Introduced,
                &added.len().to_string(),
            ));
        }
    }
    for findings in out.values_mut() {
        findings.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
        findings.dedup_by(|a, b| a.fingerprint == b.fingerprint);
    }
    out
}
