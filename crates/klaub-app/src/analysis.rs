//! The analysis use case: snapshot in, prioritised review queue out.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use klaub_core::config::Config;
use klaub_core::finding::QueueItem;
use klaub_core::graph::SymbolKey;
use klaub_core::ids::{UnitId, digest_parts};
use klaub_core::language::Language;
use klaub_core::path::RepoPath;
use klaub_core::ports::CacheKey;
use klaub_core::semantic::{FileAnalysis, SemanticSupport, SymbolChange, SymbolRef};
use klaub_core::snapshot::{ContentKind, FileChange, Snapshot, SnapshotRequest};
use klaub_core::symbol::{ParsedFile, Symbol, SymbolKind};
use klaub_core::testing::{ReportFormat, ReportSource, TestCase, TestEvidence};
use klaub_core::timestamp::Timestamp;
use klaub_core::{Error, Result};
use klaub_graph::{IndexedFile, ManifestText, ModuleMap, ProjectGraph, build_graph};
use klaub_match::{FilePair, line_diff, match_symbols};
use klaub_rules::{
    ArchInput, Evaluator, FileContext, FileReason, FileUnit, SymbolInput, is_test_path,
};
use klaub_tests::{
    Target, TestSource, associate, enumerate_tests, parse_coverage_xml, parse_junit,
};

use crate::Ports;

/// Project files beyond this many are not indexed for the call graph.
const MAX_PROJECT_FILES: usize = 50_000;

/// Project files read and parsed together.
const PROJECT_BATCH: usize = 512;
/// Largest report or manifest that is read.
const MAX_SIDE_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// Changed files whose modification time is consulted to date a test report.
const MAX_MTIME_LOOKUPS: usize = 500;

/// One changed file after parsing and matching.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Parses, symbol changes, line marks.
    pub analysis: FileAnalysis,
    /// The file looks machine-generated.
    pub generated: bool,
    /// The file is a test file by the configured patterns.
    pub is_test: bool,
    /// A parser reported syntax errors in one of the versions.
    pub parse_errors: bool,
}

/// Where a review unit lives, for navigation.
#[derive(Debug, Clone)]
pub struct UnitInfo {
    /// The unit.
    pub id: UnitId,
    /// Index into [`Analysis::files`].
    pub file_index: usize,
    /// Index into the file's `analysis.changes`, for symbol units.
    pub change_index: Option<usize>,
}

/// Everything known about a snapshot. Immutable once built.
pub struct Analysis {
    /// The changes under review.
    pub snapshot: Snapshot,
    /// The configuration in force.
    pub config: Config,
    /// Changed files in path order.
    pub files: Vec<FileEntry>,
    /// The review queue in order.
    pub queue: Vec<QueueItem>,
    /// Lookup from unit id to its place.
    pub units: HashMap<UnitId, UnitInfo>,
    /// The project's call graph.
    pub graph: ProjectGraph,
    /// Parses of the project's files (after state), changed or not.
    pub parsed: HashMap<RepoPath, Arc<ParsedFile>>,
    /// Every non-ignored file of the project on the "after" side (any language).
    pub project_files: HashSet<RepoPath>,
    /// Test associations, results and coverage.
    pub tests: TestEvidence,
    /// Test cases found in the project.
    pub test_cases: Vec<TestCase>,
    /// Imported reports are older than the newest change.
    pub reports_stale: bool,
    /// Module assignment in use.
    pub modules: ModuleMap,
    /// Findings dropped because their rule is disabled.
    pub silenced: u32,
    /// Notices about limits that were hit or data that was skipped.
    pub notes: Vec<String>,
    /// How many parses came from the cache, and how many were computed.
    pub cache_hits: usize,
    /// Parses computed (cache misses).
    pub cache_misses: usize,
}

impl Analysis {
    /// The file entry containing `unit`.
    pub fn file_of(&self, unit: &UnitId) -> Option<&FileEntry> {
        self.units.get(unit).map(|u| &self.files[u.file_index])
    }

    /// The symbol change of `unit`, if it is a symbol unit.
    pub fn change_of(&self, unit: &UnitId) -> Option<&SymbolChange> {
        let info = self.units.get(unit)?;
        info.change_index
            .map(|i| &self.files[info.file_index].analysis.changes[i])
    }

    /// The queue item of `unit`.
    pub fn item(&self, unit: &UnitId) -> Option<&QueueItem> {
        self.queue.iter().find(|i| &i.unit.id == unit)
    }

    /// The changed file with this path.
    pub fn file(&self, path: &RepoPath) -> Option<&FileEntry> {
        self.files.iter().find(|f| &f.analysis.path == path)
    }

    /// The changed file a symbol reference points into.
    ///
    /// A symbol that was moved to another file has its "before" side in a different file from the
    /// one the change is filed under, so the reference names the file: its path, or, for a file
    /// that was renamed, the path it had before.
    pub fn entry_of_ref(&self, side: &SymbolRef) -> Option<&FileEntry> {
        entry_for(&self.files, side)
    }
}

/// Position in `files` of the file a symbol reference points into.
pub(crate) fn index_for(files: &[FileEntry], side: &SymbolRef) -> Option<usize> {
    files
        .iter()
        .position(|f| f.analysis.path == side.file)
        .or_else(|| {
            files
                .iter()
                .position(|f| f.analysis.old_path.as_ref() == Some(&side.file))
        })
}

fn entry_for<'a>(files: &'a [FileEntry], side: &SymbolRef) -> Option<&'a FileEntry> {
    index_for(files, side).map(|i| &files[i])
}

/// The symbol a reference names as a [`SymbolView`], looked up like [`symbol_of_ref`].
pub(crate) fn view_of_ref<'a>(
    files: &'a [FileEntry],
    side: &SymbolRef,
    after: bool,
) -> Option<crate::contract::SymbolView<'a>> {
    let entry = entry_for(files, side)?;
    let parsed = if after {
        entry.analysis.after.as_deref()
    } else {
        entry.analysis.before.as_deref()
    }?;
    parsed.symbols.get(side.index)?;
    Some(crate::contract::SymbolView {
        file: parsed,
        index: side.index,
    })
}

/// The parsed symbol a reference names, looked up in the file the reference points into and not in
/// the file the change is filed under.
pub(crate) fn symbol_of_ref<'a>(
    files: &'a [FileEntry],
    side: &SymbolRef,
    after: bool,
) -> Option<&'a Symbol> {
    let entry = entry_for(files, side)?;
    let parsed = if after {
        entry.analysis.after.as_ref()
    } else {
        entry.analysis.before.as_ref()
    }?;
    parsed.symbols.get(side.index)
}

/// Runs `f` over `items` on a few threads, keeping the order.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .min(8);
    if items.len() < 8 || threads < 2 {
        return items.iter().map(&f).collect();
    }
    let chunk = items.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|part| scope.spawn(|| part.iter().map(&f).collect::<Vec<R>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    })
}

struct Parsing<'a> {
    ports: &'a Ports,
    worktree: klaub_core::ids::WorktreeId,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl Parsing<'_> {
    fn parse(&self, language: Language, text: &str) -> Result<Arc<ParsedFile>> {
        let content = digest_parts([language.name().as_bytes(), text.as_bytes()]);
        let key = CacheKey {
            worktree: self.worktree.clone(),
            content_hash: content,
            parser_version: self.ports.parser.parser_version(),
            kind: "parsed_file",
        };
        if let Ok(Some(bytes)) = self.ports.cache.get(&key)
            && let Ok(parsed) = serde_json::from_slice::<ParsedFile>(&bytes)
            && parsed.language == language
        {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::new(parsed));
        }
        self.misses.fetch_add(1, Ordering::Relaxed);
        let parsed = self.ports.parser.parse(language, text)?;
        if let Ok(bytes) = serde_json::to_vec(&parsed) {
            let _ = self.ports.cache.put(&key, &bytes);
        }
        Ok(Arc::new(parsed))
    }
}

fn language_enabled(config: &Config, language: Language) -> bool {
    config
        .analysis
        .languages
        .iter()
        .any(|name| language.enabled_by(name))
}

fn kind_label(change: &FileChange) -> String {
    let blob = change.after.as_ref().or(change.before.as_ref());
    match blob.map(klaub_core::snapshot::Blob::kind) {
        Some(ContentKind::Binary) => "binary or not UTF-8".to_owned(),
        Some(ContentKind::TooLarge) => "larger than the size limit".to_owned(),
        Some(ContentKind::Symlink) => "symbolic link".to_owned(),
        _ => "not readable as text".to_owned(),
    }
}

struct Task<'a> {
    file: usize,
    after: bool,
    language: Language,
    text: &'a Arc<str>,
}

/// Builds the analysis of one snapshot.
pub fn analyze(ports: &Ports, request: &SnapshotRequest, config: &Config) -> Result<Analysis> {
    let snapshot = ports.snapshots.capture(request)?;
    analyze_snapshot(ports, snapshot, config)
}

/// Analyses a snapshot that was already captured.
///
/// This is how a review is recomputed under different settings (a silenced rule) without taking a
/// new snapshot: the worktree is not read again, so the content under review cannot move.
pub(crate) fn analyze_snapshot(
    ports: &Ports,
    snapshot: Snapshot,
    config: &Config,
) -> Result<Analysis> {
    let parsing = Parsing {
        ports,
        worktree: snapshot.worktree.id.clone(),
        hits: AtomicUsize::new(0),
        misses: AtomicUsize::new(0),
    };
    let mut notes = Vec::new();
    let thresholds = config.analysis.thresholds();

    // 1. Parse both versions of every changed, readable, supported file.
    let mut tasks: Vec<Task<'_>> = Vec::new();
    for (index, change) in snapshot.files.iter().enumerate() {
        let Some(language) = change.language.filter(|l| language_enabled(config, *l)) else {
            continue;
        };
        if !change.is_reviewable_text() || change.generated {
            continue;
        }
        if let Some(text) = change.before_text() {
            tasks.push(Task {
                file: index,
                after: false,
                language,
                text,
            });
        }
        if let Some(text) = change.after_text() {
            tasks.push(Task {
                file: index,
                after: true,
                language,
                text,
            });
        }
    }
    let results = par_map(&tasks, |task| parsing.parse(task.language, task.text));
    let mut before: Vec<Option<Arc<ParsedFile>>> = vec![None; snapshot.files.len()];
    let mut after: Vec<Option<Arc<ParsedFile>>> = vec![None; snapshot.files.len()];
    let mut failed: HashSet<usize> = HashSet::new();
    for (task, result) in tasks.iter().zip(results) {
        match result {
            Ok(parsed) => {
                if task.after {
                    after[task.file] = Some(parsed)
                } else {
                    before[task.file] = Some(parsed)
                }
            }
            Err(error) => {
                tracing::warn!(%error, file = %snapshot.files[task.file].path, "parse failed");
                failed.insert(task.file);
            }
        }
    }
    if !failed.is_empty() {
        notes.push(format!(
            "{} file(s) could not be parsed and are reviewed as text",
            failed.len()
        ));
    }

    // 2. Match symbols across versions.
    let mut pair_index: Vec<usize> = Vec::new();
    let mut pairs: Vec<FilePair<'_>> = Vec::new();
    for (index, change) in snapshot.files.iter().enumerate() {
        if failed.contains(&index) || (before[index].is_none() && after[index].is_none()) {
            continue;
        }
        pair_index.push(index);
        pairs.push(FilePair {
            path: &change.path,
            old_path: change.old_path.as_ref(),
            before: before[index].as_deref(),
            after: after[index].as_deref(),
        });
    }
    let all_changes = match_symbols(&pairs, thresholds);
    drop(pairs);

    // 3. Per-file results.
    let mut files: Vec<FileEntry> = Vec::with_capacity(snapshot.files.len());
    for (index, change) in snapshot.files.iter().enumerate() {
        let mut changes: Vec<SymbolChange> = all_changes
            .iter()
            .filter(|c| {
                c.after
                    .as_ref()
                    .or(c.before.as_ref())
                    .is_some_and(|r| r.file == change.path)
            })
            .cloned()
            .collect();
        changes.sort_by_key(|c| {
            c.after
                .as_ref()
                .or(c.before.as_ref())
                .map(|r| r.span.start_line)
        });
        let support = if failed.contains(&index) {
            SemanticSupport::TextOnly("the parser failed".to_owned())
        } else if change.generated {
            SemanticSupport::TextOnly("generated file".to_owned())
        } else if !change.is_reviewable_text() {
            SemanticSupport::TextOnly(kind_label(change))
        } else if change.language.is_none() {
            SemanticSupport::TextOnly("no parser for this file type".to_owned())
        } else if !change.language.is_some_and(|l| language_enabled(config, l)) {
            SemanticSupport::TextOnly("the language is disabled in the configuration".to_owned())
        } else {
            SemanticSupport::Full
        };
        let parse_errors = [&before[index], &after[index]]
            .iter()
            .any(|p| p.as_ref().is_some_and(|p| p.has_syntax_errors));
        files.push(FileEntry {
            is_test: is_test_path(config, &change.path),
            generated: change.generated,
            parse_errors,
            analysis: FileAnalysis {
                path: change.path.clone(),
                old_path: change.old_path.clone(),
                status: change.status,
                language: change.language,
                before: before[index].clone(),
                after: after[index].clone(),
                changes,
                line_diff: line_diff(
                    change.before_text().map(AsRef::as_ref),
                    change.after_text().map(AsRef::as_ref),
                ),
                support,
            },
        });
    }

    // 4. The project index and the call graph.
    let mut parsed: HashMap<RepoPath, Arc<ParsedFile>> = HashMap::new();
    for (index, change) in snapshot.files.iter().enumerate() {
        if let Some(p) = &after[index] {
            parsed.insert(change.path.clone(), Arc::clone(p));
        }
    }
    let mut project_files: Vec<RepoPath> = ports.snapshots.list_project_files(&snapshot)?;
    project_files.retain(|p| !p.is_internal());
    let project_set: HashSet<RepoPath> = project_files.iter().cloned().collect();
    let candidates: Vec<(RepoPath, Language)> = project_files
        .iter()
        .filter(|p| !parsed.contains_key(*p))
        .filter_map(|p| Some((p.clone(), Language::from_path(p)?)))
        .filter(|(_, l)| language_enabled(config, *l))
        .collect();
    let omitted = candidates.len().saturating_sub(MAX_PROJECT_FILES);
    if omitted > 0 {
        notes.push(format!("{omitted} project file(s) were not indexed for the call graph (limit {MAX_PROJECT_FILES}); impact may be incomplete"));
    }
    // Files are read a batch at a time, so a project kept in Git objects costs a few `git`
    // processes instead of one per file, and only a batch of texts is in memory at once.
    let mut indexed: Vec<Option<(RepoPath, Arc<ParsedFile>)>> = Vec::new();
    for batch in candidates[..candidates.len().min(MAX_PROJECT_FILES)].chunks(PROJECT_BATCH) {
        let paths: Vec<RepoPath> = batch.iter().map(|(path, _)| path.clone()).collect();
        let texts = ports.snapshots.read_project_files(&snapshot, &paths);
        let work: Vec<(&RepoPath, Language, Option<Arc<str>>)> = batch
            .iter()
            .zip(texts)
            .map(|((path, language), text)| (path, *language, text))
            .collect();
        indexed.extend(par_map(&work, |(path, language, text)| {
            let parsed = parsing.parse(*language, text.as_ref()?).ok()?;
            Some(((*path).clone(), parsed))
        }));
    }
    parsed.extend(indexed.into_iter().flatten());

    let manifests: Vec<ManifestText> = project_files
        .iter()
        .filter(|p| matches!(p.file_name(), "package.json" | "pyproject.toml"))
        .filter_map(|p| {
            Some(ManifestText {
                path: p.clone(),
                text: ports
                    .snapshots
                    .read_project_file(&snapshot, p)
                    .ok()
                    .flatten()?
                    .to_string(),
            })
        })
        .collect();
    let mut indexed_files: Vec<IndexedFile> = parsed
        .iter()
        .map(|(path, parsed)| IndexedFile {
            path: path.clone(),
            parsed: Arc::clone(parsed),
        })
        .collect();
    indexed_files.sort_by(|a, b| a.path.cmp(&b.path));
    let graph = build_graph(indexed_files, &manifests);
    let modules = ModuleMap::configured(&config.architecture.modules);

    // 5. Test evidence.
    let mut test_paths: Vec<&RepoPath> =
        parsed.keys().filter(|p| is_test_path(config, p)).collect();
    test_paths.sort();
    let test_sources: Vec<TestSource<'_>> = test_paths
        .iter()
        .filter_map(|p| {
            Some(TestSource {
                path: p,
                parsed: parsed.get(*p)?,
            })
        })
        .collect();
    let test_cases: Vec<TestCase> = test_sources.iter().flat_map(enumerate_tests).collect();
    let mut targets: Vec<(Target, &RepoPath)> = Vec::new();
    for entry in &files {
        if entry.is_test {
            continue;
        }
        for change in &entry.analysis.changes {
            let Some(after_ref) = &change.after else {
                continue;
            };
            if after_ref.kind.is_callable() && after_ref.kind != SymbolKind::Module {
                let simple = after_ref
                    .qualified_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(&after_ref.qualified_name)
                    .to_owned();
                targets.push((
                    Target {
                        key: SymbolKey::new(
                            after_ref.file.clone(),
                            after_ref.qualified_name.clone(),
                        ),
                        name: simple,
                    },
                    &entry.analysis.path,
                ));
            }
        }
    }
    let mut tests = TestEvidence {
        links: associate(&test_sources, &targets),
        ..TestEvidence::default()
    };

    let root = snapshot.worktree.root.clone();
    let newest_change = snapshot
        .files
        .iter()
        .take(MAX_MTIME_LOOKUPS)
        .filter_map(|f| {
            ports
                .files
                .modified(&root, Path::new(f.path.as_str()))
                .ok()
                .flatten()
        })
        .max();
    let mut reports_stale = false;
    for raw in &config.tests.report_paths {
        match read_report(ports, &root, raw, newest_change) {
            Ok((source, results, coverage)) => {
                reports_stale |= source.stale;
                tests.results.extend(results);
                tests.coverage.extend(coverage);
                tests.sources.push(source);
            }
            Err(error) => notes.push(format!("test report {raw}: {error}")),
        }
    }

    // 6. Rules and the queue.
    let evaluator = Evaluator::new(config, Some(&graph), &tests);
    let mut symbol_inputs: Vec<SymbolInput<'_>> = Vec::new();
    let mut file_units: Vec<FileUnit<'_>> = Vec::new();
    let mut arch_inputs: Vec<ArchInput<'_>> = Vec::new();
    for (index, entry) in files.iter().enumerate() {
        let path = &entry.analysis.path;
        let ctx = FileContext {
            path,
            status: entry.analysis.status,
            is_test: entry.is_test,
            generated: entry.generated,
            parse_errors: entry.parse_errors,
        };
        let supported = matches!(entry.analysis.support, SemanticSupport::Full);
        if supported {
            for change in &entry.analysis.changes {
                // A symbol moved from another file has its old side in that file's parse.
                let before_sym = change
                    .before
                    .as_ref()
                    .and_then(|r| symbol_of_ref(&files, r, false));
                let after_sym = change
                    .after
                    .as_ref()
                    .and_then(|r| symbol_of_ref(&files, r, true));
                symbol_inputs.push(SymbolInput {
                    file: ctx,
                    change,
                    before: before_sym,
                    after: after_sym,
                });
            }
            arch_inputs.push(ArchInput {
                path,
                old_path: entry.analysis.old_path.as_ref(),
                status: entry.analysis.status,
                is_test: entry.is_test,
                before: entry.analysis.before.as_deref(),
                after: entry.analysis.after.as_deref(),
            });
        }
        let reason = match &entry.analysis.support {
            SemanticSupport::TextOnly(why) if snapshot.files[index].is_reviewable_text() => {
                Some(FileReason::NoSemanticSupport(why.clone()))
            }
            SemanticSupport::TextOnly(why) => Some(FileReason::Opaque(why.clone())),
            SemanticSupport::Full if entry.analysis.changes.is_empty() => {
                Some(FileReason::NoSymbolChanges)
            }
            SemanticSupport::Full => None,
        };
        if let Some(reason) = reason {
            file_units.push(FileUnit { file: ctx, reason });
        }
    }
    let evaluation = evaluator.evaluate(&symbol_inputs, &file_units, &arch_inputs);

    // 7. Navigation map.
    let mut units: HashMap<UnitId, UnitInfo> = HashMap::new();
    for (file_index, entry) in files.iter().enumerate() {
        units.insert(
            klaub_core::semantic::file_unit_id(&entry.analysis.path),
            UnitInfo {
                id: klaub_core::semantic::file_unit_id(&entry.analysis.path),
                file_index,
                change_index: None,
            },
        );
        for (change_index, change) in entry.analysis.changes.iter().enumerate() {
            units.insert(
                change.id.clone(),
                UnitInfo {
                    id: change.id.clone(),
                    file_index,
                    change_index: Some(change_index),
                },
            );
        }
    }
    let queue = evaluation.items;
    let silenced = evaluation.silenced;
    drop(evaluator);

    if files.is_empty() && snapshot.files.is_empty() {
        tracing::info!("the snapshot has no changes");
    }
    let cache_hits = parsing.hits.load(Ordering::Relaxed);
    let cache_misses = parsing.misses.load(Ordering::Relaxed);
    Ok(Analysis {
        snapshot,
        config: config.clone(),
        files,
        queue,
        units,
        graph,
        parsed,
        project_files: project_set,
        tests,
        test_cases,
        reports_stale,
        modules,
        silenced,
        notes,
        cache_hits,
        cache_misses,
    })
}

type ReportRead = (
    ReportSource,
    Vec<klaub_core::testing::TestResult>,
    Vec<klaub_core::testing::CoverageEntry>,
);

/// Reads one configured report (JUnit or Coverage.py XML, detected by content).
pub(crate) fn read_report(
    ports: &Ports,
    root: &Path,
    raw: &str,
    newest_change: Option<Timestamp>,
) -> Result<ReportRead> {
    let path = Path::new(raw);
    let bytes = ports.files.read_bounded(root, path, MAX_SIDE_FILE_BYTES)?;
    let text = String::from_utf8(bytes).map_err(|_| Error::invalid("report", "not valid UTF-8"))?;
    let modified = ports.files.modified(root, path).ok().flatten();
    let is_coverage = text.contains("<coverage");
    let (results, coverage) = if is_coverage {
        (Vec::new(), parse_coverage_xml(&text)?)
    } else {
        (parse_junit(&text)?, Vec::new())
    };
    let source = ReportSource {
        path: raw.to_owned(),
        format: if is_coverage {
            ReportFormat::CoveragePy
        } else {
            ReportFormat::JUnit
        },
        modified,
        content_hash: digest_parts([text.as_bytes()]).chars().take(16).collect(),
        stale: klaub_tests::is_stale(modified, newest_change),
    };
    Ok((source, results, coverage))
}
