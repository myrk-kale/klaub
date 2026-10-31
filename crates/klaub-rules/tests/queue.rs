//! The review queue over small scenarios, through parser, matcher, graph and rules.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_raw_string_hashes
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use klaub_core::config::Config;
use klaub_core::finding::{Priority, QueueItem, QueueTag, ScoreComponent, Temporal};
use klaub_core::language::Language;
use klaub_core::path::RepoPath;
use klaub_core::ports::SourceParser;
use klaub_core::snapshot::ChangeStatus;
use klaub_core::symbol::ParsedFile;
use klaub_core::testing::TestEvidence;
use klaub_graph::{IndexedFile, build_graph};
use klaub_match::{FilePair, match_symbols};
use klaub_parse::TreeSitterParser;
use klaub_rules::{
    ArchInput, Evaluation, Evaluator, FileContext, FileReason, FileUnit, SymbolInput, is_test_path,
};

type Files<'a> = &'a [(&'a str, &'a str)];

fn parse(path: &str, source: &str) -> Arc<ParsedFile> {
    let language = Language::from_path(&RepoPath::new(path).unwrap()).unwrap();
    Arc::new(TreeSitterParser::new().parse(language, source).unwrap())
}

/// Runs the whole analysis for `before` -> `after` and returns the queue.
fn queue(before: Files<'_>, after: Files<'_>, config: &Config) -> Evaluation {
    let before: BTreeMap<&str, Arc<ParsedFile>> =
        before.iter().map(|(p, s)| (*p, parse(p, s))).collect();
    let after: BTreeMap<&str, Arc<ParsedFile>> =
        after.iter().map(|(p, s)| (*p, parse(p, s))).collect();
    let paths: Vec<&str> = before
        .keys()
        .chain(after.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let repo_paths: Vec<RepoPath> = paths.iter().map(|p| RepoPath::new(p).unwrap()).collect();

    let pairs: Vec<FilePair<'_>> = paths
        .iter()
        .zip(&repo_paths)
        .map(|(p, rp)| FilePair {
            path: rp,
            old_path: None,
            before: before.get(p).map(AsRef::as_ref),
            after: after.get(p).map(AsRef::as_ref),
        })
        .collect();
    let changes = match_symbols(&pairs, config.analysis.thresholds());

    let graph = build_graph(
        after
            .iter()
            .map(|(p, parsed)| IndexedFile {
                path: RepoPath::new(p).unwrap(),
                parsed: Arc::clone(parsed),
            })
            .collect(),
        &[],
    );
    let evidence = TestEvidence::default();
    let evaluator = Evaluator::new(config, Some(&graph), &evidence);

    let mut inputs = Vec::new();
    for change in &changes {
        let anchor = change.after.as_ref().or(change.before.as_ref()).unwrap();
        let path = repo_paths.iter().find(|p| **p == anchor.file).unwrap();
        let status = match (
            before.contains_key(path.as_str()),
            after.contains_key(path.as_str()),
        ) {
            (false, true) => ChangeStatus::Added,
            (true, false) => ChangeStatus::Deleted,
            _ => ChangeStatus::Modified,
        };
        let file = FileContext {
            path,
            status,
            is_test: is_test_path(config, path),
            generated: false,
            parse_errors: false,
        };
        let before_sym = change
            .before
            .as_ref()
            .map(|r| &before[r.file.as_str()].symbols[r.index]);
        let after_sym = change
            .after
            .as_ref()
            .map(|r| &after[r.file.as_str()].symbols[r.index]);
        inputs.push(SymbolInput {
            file,
            change,
            before: before_sym,
            after: after_sym,
        });
    }
    evaluator.evaluate(&inputs, &[], &[])
}

fn find<'a>(evaluation: &'a Evaluation, label: &str) -> &'a QueueItem {
    evaluation
        .items
        .iter()
        .find(|i| i.unit.label == label)
        .unwrap_or_else(|| {
            panic!(
                "{label} not in {:?}",
                evaluation
                    .items
                    .iter()
                    .map(|i| &i.unit.label)
                    .collect::<Vec<_>>()
            )
        })
}

const ROUTES: &str = "from fastapi import FastAPI\nfrom service import find_user\n\napp = FastAPI()\n\n@app.get(\"/users\")\ndef users(name):\n    return find_user(name)\n";

#[test]
fn an_introduced_sql_injection_reachable_from_a_route_is_p0() {
    let before = [
        ("api/routes.py", ROUTES),
        (
            "service/db.py",
            "def find_user(cursor, name):\n    return cursor.execute(\"SELECT * FROM users WHERE name = %s\", (name,))\n",
        ),
    ];
    let after = [
        ("api/routes.py", ROUTES),
        (
            "service/db.py",
            "def find_user(cursor, name):\n    return cursor.execute(f\"SELECT * FROM users WHERE name = '{name}'\")\n",
        ),
    ];
    // routes imports `service` (a package): make the import resolvable.
    let evaluation = queue(&before, &after, &Config::default());
    let item = find(&evaluation, "find_user");
    assert_eq!(item.priority, Priority::P0, "{:#?}", item.score);
    let finding = item
        .findings
        .iter()
        .find(|f| f.rule_id == "risk.sql_from_dynamic_text")
        .unwrap();
    assert_eq!(finding.temporal, Temporal::Introduced);
    assert!(
        item.score
            .parts
            .iter()
            .any(|p| p.component == ScoreComponent::TechnicalRisk
                && p.rule_id.as_deref() == Some("risk.sql_from_dynamic_text"))
    );
    assert_eq!(
        evaluation.items[0].unit.label, "find_user",
        "the queue starts with it"
    );
}

#[test]
fn a_preexisting_risk_keeps_its_weight_but_is_labelled_and_sorted_after_introduced_ones() {
    let risky = "def run(cursor, name):\n    cursor.execute(f\"DELETE FROM t WHERE n = '{name}'\")\n    return 1\n";
    let other = "def other(cursor, name):\n    cursor.execute(f\"DELETE FROM t WHERE n = '{name}'\")\n    return 1\n";
    let before = [("a.py", risky), ("b.py", other)];
    // `run` is edited without touching the query; `other` gets a *new* concatenated query.
    let after = [
        ("a.py", &risky.replace("return 1", "return 2")[..]),
        (
            "b.py",
            &format!("{other}    cursor.execute(f\"UPDATE t SET n = '{{name}}'\")\n")[..],
        ),
    ];
    let evaluation = queue(&before, &after, &Config::default());
    let run = find(&evaluation, "run");
    assert!(
        run.findings
            .iter()
            .any(|f| f.rule_id == "risk.sql_from_dynamic_text"
                && f.temporal == Temporal::Preexisting)
    );
    assert!(
        !run.findings
            .iter()
            .any(|f| f.temporal == Temporal::Introduced)
    );
    let other_item = find(&evaluation, "other");
    assert!(
        other_item
            .findings
            .iter()
            .any(|f| f.temporal == Temporal::Introduced)
    );
    let position = |label: &str| {
        evaluation
            .items
            .iter()
            .position(|i| i.unit.label == label)
            .unwrap()
    };
    assert!(
        position("other") < position("run") || run.priority > other_item.priority,
        "introduced risk is shown first"
    );
}

#[test]
fn a_sensitive_name_alone_never_makes_p0() {
    let before = [(
        "auth/login.py",
        "def check_password(user, password):\n    return user.password == password\n",
    )];
    let after = [(
        "auth/login.py",
        "def check_password(user, password):\n    return user.password == password and user.active\n",
    )];
    let evaluation = queue(&before, &after, &Config::default());
    let item = find(&evaluation, "check_password");
    assert!(
        item.score
            .parts
            .iter()
            .any(|p| p.component == ScoreComponent::Criticality)
    );
    assert!(
        item.priority > Priority::P0,
        "no critical evidence, so at most P1: {:?}",
        item.priority
    );
}

#[test]
fn formatting_only_is_p3_and_says_why() {
    let before = [(
        "a.py",
        "def f(a, b):\n    total = a + b\n    return total\n",
    )];
    let after = [(
        "a.py",
        "def f(a,b):\n    total   =  a+b  # sum\n    return total\n",
    )];
    let evaluation = queue(&before, &after, &Config::default());
    let item = find(&evaluation, "f");
    assert_eq!(item.priority, Priority::P3);
    assert!(item.tags.contains(&QueueTag::FormatOnly));
    assert!(
        item.score
            .parts
            .iter()
            .any(|p| p.component == ScoreComponent::Mitigation && p.points < 0)
    );
}

#[test]
fn a_public_signature_change_reached_from_a_route_is_p0_with_itemised_reasons() {
    let before = [
        (
            "api/routes.py",
            "from fastapi import FastAPI\nfrom core.pricing import price\n\napp = FastAPI()\n\n@app.get(\"/price\")\ndef route(item):\n    return price(item)\n",
        ),
        (
            "core/pricing.py",
            "def price(item):\n    return item.cost\n",
        ),
    ];
    let after = [
        before[0],
        (
            "core/pricing.py",
            "def price(item, currency):\n    return item.cost\n",
        ),
    ];
    let evaluation = queue(&before, &after, &Config::default());
    let item = find(&evaluation, "price");
    let reasons: Vec<&str> = item.score.parts.iter().map(|p| p.reason.as_str()).collect();
    assert!(item.tags.contains(&QueueTag::Contract), "{reasons:?}");
    assert!(item.score.component_total(ScoreComponent::Contract) >= 25);
    assert!(
        item.score.component_total(ScoreComponent::Reach) >= 20,
        "{reasons:?}"
    );
    assert_eq!(
        item.priority,
        Priority::P0,
        "{reasons:?} = {}",
        item.score.total()
    );
    assert!(item.entrypoints >= 1);
}

#[test]
fn rewritten_code_never_claims_introduced_findings() {
    let before = "def f(cursor, x):\n    rows = cursor.fetchall()\n    cleaned = [r for r in rows if r]\n    grouped = {}\n    for row in cleaned:\n        grouped.setdefault(row.kind, []).append(row)\n    return grouped\n";
    let after = "def f(cursor, x):\n    import os\n    import sys\n    command = os.environ.get('RUNNER', 'sh') + ' ' + x\n    os.system(command)\n    sys.stdout.flush()\n    return 0\n";
    let evaluation = queue(&[("a.py", before)], &[("a.py", after)], &Config::default());
    let item = find(&evaluation, "f");
    let shell = item
        .findings
        .iter()
        .find(|f| f.rule_id == "risk.shell_execution")
        .unwrap();
    assert_eq!(
        item.change,
        klaub_core::semantic::ChangeKind::Rewritten,
        "{:?}",
        item.change
    );
    assert_eq!(
        shell.temporal,
        Temporal::Unknown,
        "a rewrite cannot say what is new"
    );
}

#[test]
fn new_files_and_deleted_files() {
    let evaluation = queue(
        &[("old.py", "def gone():\n    return 1\n")],
        &[("new.py", "def fresh(a):\n    eval(a)\n    return 1\n")],
        &Config::default(),
    );
    let fresh = find(&evaluation, "fresh");
    assert!(fresh.tags.contains(&QueueTag::NewFile));
    assert!(
        fresh
            .findings
            .iter()
            .any(|f| f.rule_id == "risk.dynamic_execution" && f.temporal == Temporal::Introduced)
    );
    let gone = find(&evaluation, "gone");
    assert!(gone.tags.contains(&QueueTag::RemovedFile));
}

#[test]
fn disabled_rules_are_silenced_and_counted() {
    let mut config = Config::default();
    config
        .rules
        .disabled
        .push("risk.dynamic_execution".to_owned());
    let evaluation = queue(
        &[("a.py", "def f(a):\n    return 1\n")],
        &[("a.py", "def f(a):\n    eval(a)\n    return 1\n")],
        &config,
    );
    assert!(find(&evaluation, "f").findings.is_empty());
    assert_eq!(evaluation.silenced, 1);
}

#[test]
fn tests_files_are_mitigated_and_untested_code_is_flagged() {
    let config = Config::default();
    let evaluation = queue(
        &[
            ("src/app.py", "def feature(x):\n    return x\n"),
            (
                "tests/test_app.py",
                "def test_feature():\n    assert feature(1) == 1\n",
            ),
        ],
        &[
            ("src/app.py", "def feature(x):\n    return x + 1\n"),
            (
                "tests/test_app.py",
                "def test_feature():\n    assert feature(1) == 2\n",
            ),
        ],
        &config,
    );
    let feature = find(&evaluation, "feature");
    assert!(feature.tags.contains(&QueueTag::NoTests));
    let test = find(&evaluation, "test_feature");
    assert!(!test.tags.contains(&QueueTag::NoTests));
    assert!(
        test.score
            .parts
            .iter()
            .any(|p| p.rule_id.as_deref() == Some("mitigation.test_file"))
    );
    assert!(test.priority >= feature.priority);
}

fn ctx(path: &RepoPath, generated: bool) -> FileContext<'_> {
    FileContext {
        path,
        status: ChangeStatus::Modified,
        is_test: false,
        generated,
        parse_errors: false,
    }
}

#[test]
fn unsupported_and_manifest_files_become_file_units() {
    let config = Config::default();
    let evidence = TestEvidence::default();
    let evaluator = Evaluator::new(&config, None, &evidence);
    let manifest = RepoPath::new("web/package.json").unwrap();
    let readme = RepoPath::new("README.md").unwrap();
    let lock = RepoPath::new("package-lock.json").unwrap();
    let units = [
        FileUnit {
            file: ctx(&manifest, false),
            reason: FileReason::NoSemanticSupport("JSON".into()),
        },
        FileUnit {
            file: ctx(&readme, false),
            reason: FileReason::NoSemanticSupport("Markdown".into()),
        },
        FileUnit {
            file: ctx(&lock, true),
            reason: FileReason::NoSemanticSupport("JSON".into()),
        },
    ];
    let evaluation = evaluator.evaluate(&[], &units, &[]);
    let by = |label: &str| {
        evaluation
            .items
            .iter()
            .find(|i| i.unit.label == label)
            .unwrap()
    };
    assert!(
        by("web/package.json")
            .findings
            .iter()
            .any(|f| f.rule_id == "risk.dependency_manifest")
    );
    assert!(by("web/package.json").score.total() > by("README.md").score.total());
    assert_eq!(by("package-lock.json").priority, Priority::P3, "generated");
    assert!(
        by("README.md").tags.contains(&QueueTag::Unknown),
        "no semantic analysis is said, not hidden"
    );
}

#[test]
fn forbidden_dependencies_are_introduced_or_preexisting() {
    let mut config = Config::default();
    config.architecture.modules = vec!["src/domain".into(), "src/infra".into()];
    config.architecture.forbidden_dependencies = vec![klaub_core::config::ForbiddenDependency {
        from: "src/domain".into(),
        to: "src/infra".into(),
    }];

    let before_domain = "def rule():\n    return 1\n";
    let after_domain = "from infra.db import connect\n\ndef rule():\n    return connect()\n";
    let infra = "def connect():\n    return 1\n";

    let build = |before_domain_src: &str| {
        let before_parsed = parse("src/domain/rules.py", before_domain_src);
        let after_parsed = parse("src/domain/rules.py", after_domain);
        let infra_parsed = parse("src/infra/db.py", infra);
        let graph = build_graph(
            vec![
                IndexedFile {
                    path: RepoPath::new("src/domain/rules.py").unwrap(),
                    parsed: Arc::clone(&after_parsed),
                },
                IndexedFile {
                    path: RepoPath::new("src/infra/db.py").unwrap(),
                    parsed: infra_parsed,
                },
            ],
            &[],
        );
        let evidence = TestEvidence::default();
        let evaluator = Evaluator::new(&config, Some(&graph), &evidence);
        let path = RepoPath::new("src/domain/rules.py").unwrap();
        let arch = [ArchInput {
            path: &path,
            old_path: None,
            status: ChangeStatus::Modified,
            is_test: false,
            before: Some(&before_parsed),
            after: Some(&after_parsed),
        }];
        evaluator.evaluate(&[], &[], &arch)
    };

    let introduced = build(before_domain);
    let item = &introduced.items[0];
    let finding = item
        .findings
        .iter()
        .find(|f| f.rule_id == "arch.forbidden_dependency")
        .unwrap();
    assert_eq!(finding.temporal, Temporal::Introduced);
    assert!(finding.detail.contains("src/domain") && finding.detail.contains("src/infra"));
    assert!(item.priority <= Priority::P1);

    let already = build("from infra.db import connect\n\ndef rule():\n    return connect()\n");
    assert_eq!(already.items[0].findings[0].temporal, Temporal::Preexisting);
}

#[test]
fn test_path_patterns_come_from_the_configuration() {
    let config = Config::default();
    let is = |p: &str| is_test_path(&config, &RepoPath::new(p).unwrap());
    assert!(
        is("tests/test_a.py")
            && is("pkg/test_b.py")
            && is("web/app.test.ts")
            && is("pkg/x_test.go")
    );
    assert!(!is("src/app.py") && !is("src/contest.py"));
}
