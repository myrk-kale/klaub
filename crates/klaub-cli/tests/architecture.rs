//! Executable architecture rules (see `docs/architecture.md` and ADR 0002).
//!
//! The hexagonal layout is only worth having if it is enforced. These tests read
//! the workspace's dependency graph with `cargo metadata` and scan sources of
//! the I/O-free crates, so a violation fails CI instead of surviving review.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;

const CORE: &str = "klaub-core";
const DOMAIN_SERVICES: &[&str] = &[
    "klaub-match",
    "klaub-graph",
    "klaub-rules",
    "klaub-tests",
    "klaub-context",
    "klaub-flow",
    "klaub-review",
];
const APPLICATION: &str = "klaub-app";
const DRIVEN_ADAPTERS: &[&str] = &["klaub-git", "klaub-parse", "klaub-storage", "klaub-tmux"];
const TUI: &str = "klaub-tui";
const CLI: &str = "klaub-cli";

/// APIs that reach outside the process. They belong in adapters, behind ports.
const FORBIDDEN_IO_APIS: &[&str] = &[
    "std::fs",
    "std::process",
    "std::net",
    "std::env",
    "std::thread::sleep",
    "std::time::SystemTime",
    "std::time::Instant",
    "OffsetDateTime::now",
    "tokio::",
    "reqwest",
    "libc::",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// Workspace package name -> names of the workspace crates it depends on (normal dependencies).
fn workspace_graph() -> BTreeMap<String, BTreeSet<String>> {
    let metadata = MetadataCommand::new()
        .manifest_path(workspace_root().join("Cargo.toml"))
        .no_deps()
        .exec()
        .expect("cargo metadata");
    let members: BTreeSet<String> = metadata
        .packages
        .iter()
        .map(|p| p.name.to_string())
        .collect();
    metadata
        .packages
        .iter()
        .map(|package| {
            let deps = package
                .dependencies
                .iter()
                .filter(|d| d.kind == cargo_metadata::DependencyKind::Normal)
                .map(|d| d.name.clone())
                .filter(|name| members.contains(name))
                .collect();
            (package.name.to_string(), deps)
        })
        .collect()
}

fn allowed_dependencies(package: &str) -> Option<BTreeSet<&'static str>> {
    let services: BTreeSet<&str> = DOMAIN_SERVICES.iter().copied().collect();
    if package == CORE {
        return Some(BTreeSet::new());
    }
    if package == "klaub-rules" {
        return Some(BTreeSet::from([CORE, "klaub-graph"]));
    }
    if DOMAIN_SERVICES.contains(&package) {
        return Some(BTreeSet::from([CORE]));
    }
    if package == APPLICATION {
        let mut allowed = services;
        allowed.insert(CORE);
        return Some(allowed);
    }
    if DRIVEN_ADAPTERS.contains(&package) {
        return Some(BTreeSet::from([CORE]));
    }
    if package == TUI {
        return Some(BTreeSet::from([CORE, APPLICATION]));
    }
    None // the CLI is the composition root and may depend on anything
}

#[test]
fn dependencies_point_inwards() {
    let graph = workspace_graph();
    let mut violations = Vec::new();
    for (package, dependencies) in &graph {
        if package == CLI {
            continue;
        }
        let Some(allowed) = allowed_dependencies(package) else {
            panic!("crate `{package}` is not assigned to a layer; add it to tests/architecture.rs");
        };
        for dependency in dependencies {
            if !allowed.contains(dependency.as_str()) {
                violations.push(format!("{package} -> {dependency}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "forbidden dependencies between layers:\n  {}\nSee docs/architecture.md for the allowed directions.",
        violations.join("\n  ")
    );
}

#[test]
fn only_the_composition_root_sees_every_adapter() {
    let graph = workspace_graph();
    for (package, dependencies) in &graph {
        if package == CLI {
            continue;
        }
        let adapters: Vec<_> = dependencies
            .iter()
            .filter(|d| DRIVEN_ADAPTERS.contains(&d.as_str()))
            .collect();
        assert!(
            adapters.is_empty(),
            "{package} depends on adapters {adapters:?}; only {CLI} may"
        );
    }
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn inner_crates_perform_no_io() {
    let root = workspace_root();
    let graph = workspace_graph();
    let mut offences = Vec::new();
    for package in std::iter::once(&CORE)
        .chain(DOMAIN_SERVICES.iter())
        .chain(std::iter::once(&APPLICATION))
    {
        if !graph.contains_key(*package) {
            continue; // the crate has not been introduced yet
        }
        let mut files = Vec::new();
        rust_sources(&root.join("crates").join(package).join("src"), &mut files);
        for file in files {
            let text = fs::read_to_string(&file).expect("read source");
            for (number, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                for api in FORBIDDEN_IO_APIS {
                    if code.contains(api) {
                        offences.push(format!(
                            "{}:{}: uses `{api}`",
                            file.strip_prefix(&root).unwrap_or(&file).display(),
                            number + 1
                        ));
                    }
                }
            }
        }
    }
    assert!(
        offences.is_empty(),
        "I/O belongs in adapters behind ports:\n  {}",
        offences.join("\n  ")
    );
}

#[test]
fn every_crate_inherits_workspace_lints() {
    let root = workspace_root();
    let crates = root.join("crates");
    for entry in fs::read_dir(&crates).expect("crates dir") {
        let manifest = entry.expect("entry").path().join("Cargo.toml");
        let text = fs::read_to_string(&manifest).expect("manifest");
        assert!(
            text.contains("[lints]") && text.contains("workspace = true"),
            "{} must contain `[lints] workspace = true`",
            manifest.display()
        );
    }
}
