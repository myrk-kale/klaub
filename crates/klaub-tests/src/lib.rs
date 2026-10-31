//! # klaub-tests
//!
//! A pure domain service for **test evidence**:
//!
//! - heuristic **association** of tests with the changed symbols (a test that calls the symbol and
//!   imports its module, a test named after it, ...), each link with its method and confidence;
//! - **import** of existing JUnit XML and Coverage.py XML reports, kept with their origin and a
//!   staleness flag;
//! - the **status** shown for a symbol, which never confuses "no association found" with "untested";
//! - building the confirmed **run request** for the user's own test command, without a shell.
//!
//! Nothing runs here. Tests are project code with side effects, so running them is an explicit,
//! previewed and confirmed action of the human (§12.7).

mod report;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use klaub_core::config::TestsConfig;
use klaub_core::graph::SymbolKey;
use klaub_core::ids::TestId;
use klaub_core::path::RepoPath;
use klaub_core::ports::TestRunRequest;
use klaub_core::semantic::Confidence;
use klaub_core::symbol::{ParsedFile, SymbolKind};
use klaub_core::testing::{
    EvidenceStatus, LinkMethod, TestCase, TestLink, TestOutcome, TestResult,
};
use klaub_core::timestamp::Timestamp;
use klaub_core::{Error, Result};

pub use report::{parse_coverage_xml, parse_junit};

/// Links below this confidence are not reported.
const MIN_CONFIDENCE: f32 = 0.5;
/// At most this many links are kept per symbol.
const MAX_LINKS_PER_SYMBOL: usize = 10;

/// A parsed test file.
#[derive(Debug, Clone, Copy)]
pub struct TestSource<'a> {
    /// Path of the file.
    pub path: &'a RepoPath,
    /// Its parse.
    pub parsed: &'a ParsedFile,
}

/// A changed symbol tests may exercise.
#[derive(Debug, Clone)]
pub struct Target {
    /// Identity of the symbol.
    pub key: SymbolKey,
    /// Its simple name (`validate`).
    pub name: String,
}

/// Lists the test cases of a test file: functions and methods the parser marked as tests.
pub fn enumerate_tests(source: &TestSource<'_>) -> Vec<TestCase> {
    source
        .parsed
        .symbols
        .iter()
        .filter(|s| {
            s.features.is_test && matches!(s.kind, SymbolKind::Function | SymbolKind::Method)
        })
        .filter_map(|s| {
            Some(TestCase {
                id: TestId::parse(format!("{}::{}", source.path, s.qualified_name)).ok()?,
                file: source.path.clone(),
                name: s.qualified_name.clone(),
                line: s.span.start_line,
            })
        })
        .collect()
}

/// Lower-cases and drops separators so `validateToken`, `validate_token` and `ValidateToken` compare equal.
fn normalize(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The part of a test file name that names the code under test (`test_auth.py` -> `auth`).
fn subject_stem(path: &RepoPath) -> String {
    let stem = path.stem();
    let stem = stem
        .strip_suffix(".test")
        .or_else(|| stem.strip_suffix(".spec"))
        .unwrap_or(stem);
    let stem = stem
        .strip_prefix("test_")
        .or_else(|| stem.strip_suffix("_test"))
        .or_else(|| stem.strip_suffix("_spec"))
        .unwrap_or(stem);
    // Java names the test after the class: `CartTest`, `CartTests`, `CartIT`.
    let stem = if path.extension() == Some("java") {
        ["Tests", "Test", "IT"]
            .iter()
            .find_map(|suffix| stem.strip_suffix(suffix).filter(|rest| !rest.is_empty()))
            .unwrap_or(stem)
    } else {
        stem
    };
    normalize(stem)
}

fn module_matches(import: &str, target_file: &RepoPath) -> bool {
    let import = import.trim_start_matches(['.', '/']);
    if import.is_empty() {
        return false;
    }
    let stem = target_file.stem();
    let dotted = import.replace(['/', ':'], ".");
    let mut parts = target_file
        .as_str()
        .trim_end_matches(|c: char| c != '.')
        .trim_end_matches('.')
        .split('/')
        .collect::<Vec<_>>();
    if parts.last() == Some(&"__init__") {
        parts.pop();
    }
    let path_dotted = parts.join(".");
    dotted == stem
        || dotted.ends_with(&format!(".{stem}"))
        || path_dotted.ends_with(&dotted)
        || dotted.ends_with(&path_dotted)
}

/// Finds the tests that probably exercise each target.
///
/// A link is a *hint with a reason and a confidence*, not coverage: a test that calls `validate`
/// and imports its module is a strong hint; a test that merely has a similar name is a weak one.
pub fn associate(sources: &[TestSource<'_>], targets: &[(Target, &RepoPath)]) -> Vec<TestLink> {
    struct Case<'a> {
        case: TestCase,
        calls: HashSet<&'a str>,
        name: String,
    }
    /// The cases of one test file. What a file imports and what it is named after are the same for
    /// every case in it, so they are kept once per file and checked once per target and file, not
    /// once per target and case. A project with thousands of tests made the difference seconds.
    struct Group<'a> {
        file_subject: String,
        imports: Vec<&'a str>,
        cases: Vec<Case<'a>>,
    }
    let mut groups: Vec<Group<'_>> = Vec::new();
    for source in sources {
        let imports: Vec<&str> = source
            .parsed
            .imports
            .iter()
            .flat_map(|i| {
                std::iter::once(i.module.as_str()).chain(i.names.iter().map(|n| n.name.as_str()))
            })
            .collect();
        let mut cases: Vec<Case<'_>> = Vec::new();
        for case in enumerate_tests(source) {
            let Some(index) = source.parsed.find(&case.name) else {
                continue;
            };
            let calls = source
                .parsed
                .calls
                .iter()
                .filter(|c| c.caller == index)
                .map(|c| c.name.as_str())
                .collect();
            let name = normalize(
                case.name
                    .rsplit('.')
                    .next()
                    .unwrap_or(&case.name)
                    .trim_start_matches("test")
                    .trim_start_matches("Test"),
            );
            cases.push(Case { case, calls, name });
        }
        if !cases.is_empty() {
            groups.push(Group {
                file_subject: subject_stem(source.path),
                imports,
                cases,
            });
        }
    }

    let mut links: HashMap<(SymbolKey, TestId), TestLink> = HashMap::new();
    for (target, target_file) in targets {
        let wanted = normalize(&target.name);
        if wanted.is_empty() {
            continue;
        }
        let target_subject = normalize(target_file.stem());
        for group in &groups {
            let imports_module = group
                .imports
                .iter()
                .any(|i| module_matches(i, target_file) || *i == target.name);
            let file_match = !group.file_subject.is_empty() && group.file_subject == target_subject;
            for case in &group.cases {
                let referenced = case.calls.contains(target.name.as_str());
                let name_match = !case.name.is_empty()
                    && (case.name.contains(&wanted)
                        || (wanted.contains(&case.name) && case.name.len() >= 4));
                let (method, confidence) =
                    match (referenced, imports_module, name_match, file_match) {
                        (true, true, _, _) => (LinkMethod::Reference, 0.9),
                        (true, false, _, true) => (LinkMethod::Reference, 0.75),
                        (true, false, _, false) => (LinkMethod::Reference, 0.6),
                        (false, _, true, true) => (LinkMethod::NameConvention, 0.7),
                        (false, _, true, false) => (LinkMethod::NameConvention, 0.5),
                        _ => continue,
                    };
                let key = (target.key.clone(), case.case.id.clone());
                let link = TestLink {
                    test: case.case.clone(),
                    target: target.key.clone(),
                    method,
                    confidence: Confidence::new(confidence),
                };
                links
                    .entry(key)
                    .and_modify(|existing| {
                        if link.confidence > existing.confidence {
                            *existing = link.clone();
                        }
                    })
                    .or_insert(link);
            }
        }
    }

    let mut all: Vec<TestLink> = links
        .into_values()
        .filter(|l| l.confidence.value() >= MIN_CONFIDENCE)
        .collect();
    all.sort_by(|a, b| {
        a.target
            .cmp(&b.target)
            .then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.test.id.cmp(&b.test.id))
    });
    let mut per_symbol: HashMap<SymbolKey, usize> = HashMap::new();
    all.retain(|link| {
        let count = per_symbol.entry(link.target.clone()).or_default();
        *count += 1;
        *count <= MAX_LINKS_PER_SYMBOL
    });
    all
}

/// Whether `result` reports the outcome of `case`.
pub fn result_matches(case: &TestCase, result: &TestResult) -> bool {
    let simple = case.name.rsplit('.').next().unwrap_or(&case.name);
    let named = result.name == simple
        || result.name.ends_with(&format!("::{simple}"))
        || result.name.ends_with(&format!(".{simple}"));
    if !named {
        return false;
    }
    match &result.file {
        Some(file) => file == &case.file,
        None => {
            // pytest's JUnit names the module instead of the file: `tests.test_auth.TestX::test_y`.
            let module = case
                .file
                .as_str()
                .trim_end_matches(|c: char| c != '.')
                .trim_end_matches('.')
                .replace('/', ".");
            result.name.contains(&module)
                || result.name.contains(&normalize_dotted(&module))
                || !result.name.contains("::") && !result.name.contains('.')
        }
    }
}

fn normalize_dotted(module: &str) -> String {
    module.rsplit('.').next().unwrap_or(module).to_owned()
}

/// Whether a report is older than the newest change it should reflect.
pub fn is_stale(report_modified: Option<Timestamp>, newest_change: Option<Timestamp>) -> bool {
    match (report_modified, newest_change) {
        (Some(report), Some(change)) => report < change,
        (None, _) => true,
        (Some(_), None) => false,
    }
}

/// The evidence status of a symbol from its links and the imported results.
pub fn evidence_status(links: &[&TestLink], results: &[TestResult], stale: bool) -> EvidenceStatus {
    if links.is_empty() {
        return EvidenceStatus::NoAssociationFound;
    }
    let matched: Vec<&TestResult> = results
        .iter()
        .filter(|r| links.iter().any(|l| result_matches(&l.test, r)))
        .collect();
    if matched.is_empty() {
        return EvidenceStatus::NotRun;
    }
    if stale {
        return EvidenceStatus::Stale;
    }
    if matched.iter().any(|r| r.outcome == TestOutcome::Failed) {
        EvidenceStatus::Failed
    } else if matched.iter().any(|r| r.outcome == TestOutcome::Passed) {
        EvidenceStatus::Passed
    } else {
        EvidenceStatus::NotRun
    }
}

/// Builds the request to run the user's configured test command for a selection.
///
/// The command comes only from configuration (`[tests] command`); nothing is inferred from the
/// repository, from comments or from code under review. Selected ids are appended as separate
/// arguments and cannot start with `-`, so they cannot become options.
pub fn run_request(
    config: &TestsConfig,
    selection: &[TestId],
    cwd: &Path,
) -> Result<TestRunRequest> {
    let Some(command) = config.command.clone() else {
        return Err(Error::Unsupported(
            "no test command is configured; set `command = [...]` under [tests] in .klaub.toml"
                .to_owned(),
        ));
    };
    Ok(TestRunRequest {
        command,
        selection: selection.to_vec(),
        cwd: cwd.to_path_buf(),
        timeout: Duration::from_secs(config.timeout_secs.max(1)),
    })
}

/// A human-readable preview of what a request would run (for confirmation; never executed through a shell).
pub fn preview(request: &TestRunRequest) -> String {
    let quote = |word: &str| {
        if !word.is_empty()
            && word
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "/._-=:,@%+".contains(c))
        {
            word.to_owned()
        } else {
            format!("{word:?}")
        }
    };
    let words: Vec<String> = request
        .command
        .iter()
        .map(String::as_str)
        .chain(request.selection.iter().map(TestId::as_str))
        .map(quote)
        .collect();
    format!(
        "{}\n  in {}\n  timeout {}s, no shell",
        words.join(" "),
        request.cwd.display(),
        request.timeout.as_secs()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use klaub_core::language::Language;
    use klaub_core::symbol::{
        CallSite, Fingerprint, ImportRef, Receiver, Span, Symbol, SymbolFeatures, Visibility,
    };

    fn symbol(name: &str, is_test: bool) -> Symbol {
        Symbol {
            name: name.into(),
            qualified_name: name.into(),
            kind: SymbolKind::Function,
            signature: format!("def {name}()"),
            visibility: Visibility::Public,
            span: Span {
                start_byte: 0,
                end_byte: 0,
                start_line: 1,
                end_line: 2,
            },
            parent: Some(0),
            decorators: vec![],
            fingerprint: Fingerprint::default(),
            features: SymbolFeatures {
                is_test,
                ..SymbolFeatures::default()
            },
        }
    }

    fn test_file(tests: &[(&str, &[&str])], imports: &[&str]) -> ParsedFile {
        let mut symbols = vec![symbol("<module>", false)];
        let mut calls = Vec::new();
        for (name, called) in tests {
            symbols.push(symbol(name, true));
            let caller = symbols.len() - 1;
            calls.extend(called.iter().map(|c| CallSite {
                caller,
                name: (*c).into(),
                receiver: Receiver::None,
                line: 1,
                in_loop: false,
            }));
        }
        ParsedFile {
            language: Language::Python,
            symbols,
            imports: imports
                .iter()
                .map(|m| ImportRef {
                    module: (*m).into(),
                    relative_level: 0,
                    names: vec![],
                    alias: None,
                    wildcard: false,
                    reexport: false,
                    dynamic: false,
                    line: 1,
                })
                .collect(),
            calls,
            entrypoints: vec![],
            has_syntax_errors: false,
            parser_version: 1,
        }
    }

    fn target(file: &str, name: &str) -> (Target, RepoPath) {
        let path = RepoPath::new(file).unwrap();
        (
            Target {
                key: SymbolKey::new(path.clone(), name),
                name: name.into(),
            },
            path,
        )
    }

    fn links(sources: &[(&str, ParsedFile)], targets: &[(Target, RepoPath)]) -> Vec<TestLink> {
        let paths: Vec<RepoPath> = sources
            .iter()
            .map(|(p, _)| RepoPath::new(p).unwrap())
            .collect();
        let test_sources: Vec<TestSource<'_>> = sources
            .iter()
            .zip(&paths)
            .map(|((_, parsed), path)| TestSource { path, parsed })
            .collect();
        let targets: Vec<(Target, &RepoPath)> =
            targets.iter().map(|(t, p)| (t.clone(), p)).collect();
        associate(&test_sources, &targets)
    }

    #[test]
    fn a_test_that_calls_the_symbol_and_imports_its_module_is_a_strong_link() {
        let file = test_file(
            &[
                ("test_login", &["validate"]),
                ("test_other", &["something_else"]),
            ],
            &["app.auth"],
        );
        let found = links(
            &[("tests/test_auth.py", file)],
            &[target("src/app/auth.py", "validate")],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].method, LinkMethod::Reference);
        assert!((found[0].confidence.value() - 0.9).abs() < f32::EPSILON);
        assert_eq!(found[0].test.id.as_str(), "tests/test_auth.py::test_login");
    }

    #[test]
    fn name_only_links_are_weaker_and_unrelated_tests_are_not_linked() {
        let file = test_file(
            &[
                ("test_validate_token_rejects_empty", &[]),
                ("test_render", &[]),
            ],
            &[],
        );
        let found = links(
            &[("tests/test_misc.py", file)],
            &[target("src/auth.py", "validate_token")],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].method, LinkMethod::NameConvention);
        assert!((found[0].confidence.value() - 0.5).abs() < f32::EPSILON);

        let same_file = test_file(&[("test_validate_token_rejects_empty", &[])], &[]);
        let found = links(
            &[("tests/test_auth.py", same_file)],
            &[target("src/auth.py", "validate_token")],
        );
        assert!(
            (found[0].confidence.value() - 0.7).abs() < f32::EPSILON,
            "a matching file name raises the confidence"
        );
    }

    #[test]
    fn no_evidence_no_link() {
        let file = test_file(&[("test_a", &["x"])], &["os"]);
        assert!(
            links(
                &[("tests/test_a.py", file)],
                &[target("src/auth.py", "validate")]
            )
            .is_empty()
        );
    }

    #[test]
    fn links_are_capped_per_symbol() {
        let names: Vec<String> = (0..30).map(|i| format!("test_validate_{i}")).collect();
        let tests: Vec<(&str, &[&str])> = names.iter().map(|n| (n.as_str(), &[][..])).collect();
        let found = links(
            &[("tests/test_auth.py", test_file(&tests, &[]))],
            &[target("src/auth.py", "validate")],
        );
        assert_eq!(found.len(), MAX_LINKS_PER_SYMBOL);
    }

    fn case(file: &str, name: &str) -> TestCase {
        TestCase {
            id: TestId::parse(format!("{file}::{name}")).unwrap(),
            file: RepoPath::new(file).unwrap(),
            name: name.into(),
            line: 1,
        }
    }

    fn link(file: &str, name: &str) -> TestLink {
        TestLink {
            test: case(file, name),
            target: SymbolKey::new(RepoPath::new("src/a.py").unwrap(), "f"),
            method: LinkMethod::Reference,
            confidence: Confidence::new(0.9),
        }
    }

    fn result(name: &str, outcome: TestOutcome) -> TestResult {
        TestResult {
            name: name.into(),
            file: None,
            outcome,
            duration_ms: None,
            message: None,
        }
    }

    #[test]
    fn evidence_statuses_do_not_overclaim() {
        let l = link("tests/test_auth.py", "TestAuth.test_login");
        assert_eq!(
            evidence_status(&[], &[], false),
            EvidenceStatus::NoAssociationFound
        );
        assert_eq!(evidence_status(&[&l], &[], false), EvidenceStatus::NotRun);
        let passed = [result(
            "tests.test_auth.TestAuth::test_login",
            TestOutcome::Passed,
        )];
        assert_eq!(
            evidence_status(&[&l], &passed, false),
            EvidenceStatus::Passed
        );
        assert_eq!(
            evidence_status(&[&l], &passed, true),
            EvidenceStatus::Stale,
            "old reports are marked, not trusted"
        );
        let failed = [result(
            "tests.test_auth.TestAuth::test_login",
            TestOutcome::Failed,
        )];
        assert_eq!(
            evidence_status(&[&l], &failed, false),
            EvidenceStatus::Failed
        );
        let unrelated = [result(
            "tests.test_other.TestX::test_login",
            TestOutcome::Failed,
        )];
        assert_eq!(
            evidence_status(&[&l], &unrelated, false),
            EvidenceStatus::NotRun,
            "another module's test_login is not ours"
        );
    }

    #[test]
    fn staleness_compares_the_report_with_the_newest_change() {
        let t = |n| Some(Timestamp::from_unix(n).unwrap());
        assert!(is_stale(t(10), t(20)));
        assert!(!is_stale(t(30), t(20)));
        assert!(is_stale(None, t(20)), "no timestamp, no trust");
        assert!(!is_stale(t(10), None));
    }

    #[test]
    fn the_run_request_comes_only_from_configuration() {
        let mut config = TestsConfig::default();
        assert!(matches!(
            run_request(&config, &[], Path::new("/repo")),
            Err(Error::Unsupported(_))
        ));
        config.command = Some(vec!["pytest".into(), "-q".into()]);
        let ids = [TestId::parse("tests/test_a.py::test_x").unwrap()];
        let request = run_request(&config, &ids, Path::new("/repo")).unwrap();
        assert_eq!(request.command, vec!["pytest", "-q"]);
        assert_eq!(request.selection, ids);
        assert_eq!(request.timeout, Duration::from_secs(600));
        let shown = preview(&request);
        assert!(
            shown.contains("pytest -q tests/test_a.py::test_x") && shown.contains("no shell"),
            "{shown}"
        );
    }

    #[test]
    fn previews_quote_what_is_odd() {
        let request = TestRunRequest {
            command: vec!["run".into()],
            selection: vec![TestId::parse("tests/a b.py::t; rm -rf ~").unwrap()],
            cwd: "/r".into(),
            timeout: Duration::from_secs(5),
        };
        assert!(preview(&request).contains("\"tests/a b.py::t; rm -rf ~\""));
    }
}
