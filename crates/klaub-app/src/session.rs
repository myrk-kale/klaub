//! A human's work on an analysis: comments, marks, finishing and following up.
//!
//! Every mutating method saves the review before returning, so what the reviewer confirmed is on
//! disk. Nothing here submits anything on its own: delivery to the agent happens only through the
//! explicit [`ReviewSession::prepare_in_agent`], and a paste is never a submission.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use klaub_context::{CoverageUnit, compute_coverage, finish_warnings, parse_brief};
use klaub_core::config::Config;
use klaub_core::finding::{FindingDelta, Priority, Temporal};
use klaub_core::flow::FlowState;
use klaub_core::graph::SymbolKey;
use klaub_core::ids::{CommentId, CriterionId, SessionId, UnitId};
use klaub_core::path::RepoPath;
use klaub_core::ports::{
    AgentChannel, CancelToken, Clipboard, PrepareOutcome, TestRunOutcome, TestRunRequest,
};
use klaub_core::review::{
    Anchor, Brief, CommentStatus, CriterionStatus, Delivery, FeedbackRecord, FlowRef,
    FollowupStatus, Review, Version, review_file,
};
use klaub_core::semantic::{Confidence, SymbolRef};
use klaub_core::snapshot::{Drift, SnapshotRequest};
use klaub_core::symbol::Symbol;
use klaub_core::testing::{CoverageEntry, EvidenceStatus, ReportSource, TestLink, TestResult};
use klaub_core::text::{sanitize_terminal_text, truncate_chars};
use klaub_core::timestamp::Timestamp;
use klaub_core::{Error, Result};
use klaub_flow::{FlowContext, ValidatedFlow, Validation};
use klaub_review::{Candidate, ExportContext, ReviewTarget, build_anchor_fingerprint, relocate};

use crate::{Analysis, ContractDelta, Ports, analysis};

const EXCERPT_CHARS: usize = 400;

/// How to open a review session.
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    /// The wrapped session this review belongs to, if any.
    pub session_id: Option<SessionId>,
    /// A brief file to import (`klaub review --brief`), already confined to the worktree by the reader.
    pub brief_file: Option<PathBuf>,
}

/// The state of the flow diagram of the current snapshot.
#[derive(Debug, Clone)]
pub struct FlowView {
    /// `current`, `missing`, `invalid` or `stale`.
    pub state: FlowState,
    /// The document, when it is valid (also when stale, to be consulted with a warning).
    pub flow: Option<ValidatedFlow>,
    /// Every problem found when the document is invalid.
    pub issues: Vec<String>,
    /// Where the document is expected.
    pub expected_path: RepoPath,
    /// The instruction that asks the agent to generate it.
    pub request_prompt: String,
}

/// What is known about the tests of one symbol.
#[derive(Debug, Clone)]
pub struct TestPanel {
    /// The honest status.
    pub status: EvidenceStatus,
    /// Associated tests with method and confidence.
    pub links: Vec<TestLink>,
    /// Imported results that match an associated test.
    pub results: Vec<TestResult>,
    /// Where imported results came from.
    pub sources: Vec<ReportSource>,
    /// Coverage of the symbol's file, when a coverage report was imported.
    pub coverage: Option<CoverageEntry>,
}

/// Test reports as last read; replaced when the user runs tests or reloads.
#[derive(Debug, Clone, Default)]
pub struct Reports {
    /// Imported results.
    pub results: Vec<TestResult>,
    /// Imported coverage.
    pub coverage: Vec<CoverageEntry>,
    /// Where they came from.
    pub sources: Vec<ReportSource>,
    /// Older than the newest change.
    pub stale: bool,
    /// Problems reading reports.
    pub notes: Vec<String>,
}

impl Reports {
    fn from_analysis(analysis: &Analysis) -> Self {
        Self {
            results: analysis.tests.results.clone(),
            coverage: analysis.tests.coverage.clone(),
            sources: analysis.tests.sources.clone(),
            stale: analysis.reports_stale,
            notes: analysis
                .notes
                .iter()
                .filter(|n| n.starts_with("test report "))
                .cloned()
                .collect(),
        }
    }
}

/// A review in progress.
pub struct ReviewSession {
    ports: Ports,
    request: SnapshotRequest,
    analysis: Arc<Analysis>,
    review: Review,
    flow: FlowView,
    drift: Drift,
    reports: Reports,
}

fn target(
    analysis: &Analysis,
    session: Option<SessionId>,
    request: &SnapshotRequest,
) -> ReviewTarget {
    let snapshot = &analysis.snapshot;
    ReviewTarget {
        repository: snapshot.worktree.root.display().to_string(),
        repository_id: snapshot.worktree.repository_id.clone(),
        worktree_id: snapshot.worktree.id.clone(),
        base: request.base.clone(),
        base_oid: snapshot.base_oid.clone(),
        scope: snapshot.scope.clone(),
        snapshot_id: snapshot.id.clone(),
        session_id: session,
    }
}

/// Symbols of the new snapshot that an old anchor may land on.
fn candidates(analysis: &Analysis, wanted: &HashSet<RepoPath>) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (path, parsed) in &analysis.parsed {
        let changed = analysis.file(path);
        if changed.is_none() && !wanted.contains(path) {
            continue;
        }
        let text = analysis
            .snapshot
            .file(path)
            .and_then(|f| f.after_text().cloned());
        for symbol in parsed
            .symbols
            .iter()
            .filter(|s| s.kind != klaub_core::symbol::SymbolKind::Module)
        {
            let unit_id =
                klaub_core::semantic::symbol_unit_id(path, &symbol.qualified_name, symbol.kind);
            let body = text
                .as_ref()
                .and_then(|t| t.get(symbol.span.start_byte..symbol.span.end_byte.min(t.len())))
                .unwrap_or_default()
                .to_owned();
            out.push(Candidate {
                file: path.clone(),
                qualified_name: symbol.qualified_name.clone(),
                kind: symbol.kind,
                signature: symbol.signature.clone(),
                unit_id,
                start_line: symbol.span.start_line,
                end_line: symbol.span.end_line,
                text: body,
            });
        }
    }
    out
}

fn relocate_comments(analysis: &Analysis, review: &mut Review) {
    let wanted: HashSet<RepoPath> = review
        .comments
        .iter()
        .map(|c| c.anchor.file_path.clone())
        .collect();
    let pool = candidates(analysis, &wanted);
    let files: Vec<RepoPath> = analysis.project_files.iter().cloned().collect();
    for comment in &mut review.comments {
        let relocation = relocate(&comment.anchor, &pool, &files);
        comment.anchor_confidence = relocation.confidence;
        if relocation.is_stale() {
            comment.status = CommentStatus::Stale;
            comment.followup_status = FollowupStatus::Stale;
        } else {
            comment.anchor = relocation.anchor;
        }
    }
}

impl ReviewSession {
    /// Analyses the changes and opens (or resumes) the review of this worktree.
    ///
    /// - the most recent review of the worktree for the very same snapshot is resumed;
    /// - one for an older snapshot with comments becomes the previous round: its comments are carried
    ///   over, relocated, and wait for a decision;
    /// - otherwise a fresh review starts.
    pub fn open(
        ports: Ports,
        request: SnapshotRequest,
        config: &Config,
        options: &OpenOptions,
    ) -> Result<Self> {
        let analysis = Arc::new(analysis::analyze(&ports, &request, config)?);
        let now = ports.clock.now();
        let target = target(&analysis, options.session_id.clone(), &request);

        let previous = match ports.reviews.latest(&analysis.snapshot.worktree.id)? {
            Some(id) => Some(ports.reviews.load(&id)?),
            None => None,
        };
        let mut review = match previous {
            Some(previous) if previous.snapshot_id == analysis.snapshot.id => previous,
            Some(previous) if !previous.comments.is_empty() => {
                let mut next =
                    klaub_review::next_round(&previous, &ports.ids.random_hex(), now, &target);
                relocate_comments(&analysis, &mut next);
                next
            }
            _ => klaub_review::new_review(&target, &ports.ids.random_hex(), now),
        };
        if options.session_id.is_some() {
            review.session_id.clone_from(&options.session_id);
        }
        if let Some(brief_file) = &options.brief_file {
            let bytes = ports.files.read_bounded(
                &analysis.snapshot.worktree.root,
                brief_file,
                klaub_context::MAX_BRIEF_BYTES as u64,
            )?;
            let text =
                String::from_utf8(bytes).map_err(|_| Error::invalid("brief", "not valid UTF-8"))?;
            review.brief = parse_brief(&text)?;
        }

        let flow = Self::read_flow(&ports, &analysis)?;
        let reports = Reports::from_analysis(&analysis);
        let mut session = Self {
            ports,
            request,
            analysis,
            review,
            flow,
            drift: Drift::Unchanged,
            reports,
        };
        session.sync_flow_ref();
        session.save()?;
        Ok(session)
    }

    /// The immutable analysis being reviewed.
    pub fn analysis(&self) -> &Arc<Analysis> {
        &self.analysis
    }

    /// The review document (comments, marks, brief, feedback).
    pub fn review(&self) -> &Review {
        &self.review
    }

    /// The flow diagram state.
    pub fn flow(&self) -> &FlowView {
        &self.flow
    }

    /// Whether the worktree changed since the snapshot was taken (as of the last check).
    pub fn drift(&self) -> Drift {
        self.drift
    }

    /// The snapshot request in use.
    pub fn request(&self) -> &SnapshotRequest {
        &self.request
    }

    /// The test reports as last read.
    pub fn reports(&self) -> &Reports {
        &self.reports
    }

    /// The ports in use.
    pub fn ports(&self) -> &Ports {
        &self.ports
    }

    fn now(&self) -> Timestamp {
        self.ports.clock.now()
    }

    fn save(&self) -> Result<()> {
        self.ports.reviews.save(&self.review)
    }

    /// The path of the review file relative to the worktree.
    pub fn review_file(&self) -> String {
        review_file(&self.review.review_id)
    }

    // ---- comments -------------------------------------------------------------------------

    /// Builds the anchor for a selection of lines (or the whole unit) in one version.
    pub fn anchor_for(
        &self,
        unit: &UnitId,
        version: Version,
        lines: Option<(u32, u32)>,
    ) -> Result<Anchor> {
        let info = self
            .analysis
            .units
            .get(unit)
            .ok_or_else(|| Error::not_found("unit", unit))?;
        let entry = &self.analysis.files[info.file_index];
        let change = &self.analysis.snapshot.files[info.file_index];
        let symbol: Option<&SymbolRef> = info.change_index.and_then(|i| {
            let c = &entry.analysis.changes[i];
            match version {
                Version::After => c.after.as_ref().or(c.before.as_ref()),
                Version::Before => c.before.as_ref().or(c.after.as_ref()),
            }
        });
        let text = match version {
            Version::After => change.after_text().or(change.before_text()),
            Version::Before => change.before_text().or(change.after_text()),
        };
        let excerpt = lines.zip(text).map(|((from, to), text)| {
            let selected: Vec<&str> = text
                .lines()
                .skip(from.saturating_sub(1) as usize)
                .take((to.saturating_sub(from) + 1) as usize)
                .collect();
            truncate_chars(&sanitize_terminal_text(&selected.join("\n")), EXCERPT_CHARS)
        });
        let basis = excerpt
            .clone()
            .or_else(|| symbol.map(|s| s.qualified_name.clone()))
            .unwrap_or_else(|| entry.analysis.path.to_string());
        Ok(Anchor {
            file_path: entry.analysis.path.clone(),
            module_id: Some(self.analysis.modules.module_of(&entry.analysis.path)),
            symbol_id: symbol.map(|_| unit.clone()),
            symbol_kind: symbol.map(|s| s.kind),
            qualified_name: symbol.map(|s| s.qualified_name.clone()),
            symbol_signature: symbol.map(|s| s.signature.clone()),
            version,
            byte_range: None,
            line_range: lines,
            ast_path: symbol
                .map(|s| s.qualified_name.split('.').map(str::to_owned).collect())
                .unwrap_or_default(),
            fingerprint: build_anchor_fingerprint(&basis),
            excerpt: excerpt.filter(|e| !e.trim().is_empty()),
        })
    }

    /// Adds a draft comment on a unit and saves.
    pub fn add_comment(
        &mut self,
        unit: &UnitId,
        version: Version,
        lines: Option<(u32, u32)>,
        text: &str,
        severity: Option<Priority>,
    ) -> Result<CommentId> {
        let anchor = self.anchor_for(unit, version, lines)?;
        let now = self.now();
        let id = klaub_review::add_comment(
            &mut self.review,
            &self.ports.ids.random_hex(),
            now,
            anchor,
            text,
            severity,
        )?;
        self.save()?;
        Ok(id)
    }

    /// Edits a comment and saves.
    pub fn edit_comment(
        &mut self,
        id: &CommentId,
        text: &str,
        severity: Option<Priority>,
    ) -> Result<()> {
        let now = self.now();
        klaub_review::edit_comment(&mut self.review, id, now, text, severity)?;
        self.save()
    }

    /// Deletes a draft comment and saves.
    pub fn delete_comment(&mut self, id: &CommentId) -> Result<()> {
        let now = self.now();
        klaub_review::delete_comment(&mut self.review, id, now)?;
        self.save()
    }

    /// Comments attached to a unit (by symbol, or by file for file-level units).
    pub fn comments_on(&self, unit: &UnitId) -> Vec<&klaub_core::review::Comment> {
        let file = self
            .analysis
            .units
            .get(unit)
            .map(|i| &self.analysis.files[i.file_index].analysis.path);
        self.review
            .comments
            .iter()
            .filter(|c| {
                c.anchor.symbol_id.as_ref() == Some(unit)
                    || (c.anchor.symbol_id.is_none() && Some(&c.anchor.file_path) == file)
            })
            .collect()
    }

    /// Remembers which unit the reader is on and saves.
    ///
    /// This only records the cursor: it marks nothing reviewed and takes no snapshot.
    pub fn remember_position(&mut self, unit: &UnitId) -> Result<()> {
        if self.review.last_unit.as_ref() == Some(unit) {
            return Ok(());
        }
        self.review.last_unit = Some(unit.clone());
        self.review.updated_at = self.now();
        self.save()
    }

    /// Marks or unmarks a unit as reviewed (an explicit human action) and saves.
    pub fn toggle_reviewed(&mut self, unit: &UnitId) -> Result<bool> {
        let now = self.now();
        let marked = klaub_review::toggle_reviewed(&mut self.review, unit, now);
        self.save()?;
        Ok(marked)
    }

    /// Records the human's decision about a comment of the previous round and saves.
    pub fn decide_followup(
        &mut self,
        id: &CommentId,
        status: FollowupStatus,
        note: Option<String>,
    ) -> Result<()> {
        let now = self.now();
        klaub_review::decide_followup(&mut self.review, id, status, note, now)?;
        self.save()
    }

    // ---- brief ----------------------------------------------------------------------------

    /// The brief of the review.
    pub fn brief(&self) -> &Brief {
        &self.review.brief
    }

    /// Sets the intent summary and saves.
    pub fn set_intent(&mut self, text: &str) -> Result<()> {
        klaub_context::set_intent(&mut self.review.brief, text);
        self.review.updated_at = self.now();
        self.save()
    }

    /// Adds an acceptance criterion and saves.
    pub fn add_criterion(&mut self, text: &str) -> Result<CriterionId> {
        let id = klaub_context::add_criterion(&mut self.review.brief, text)?;
        self.review.updated_at = self.now();
        self.save()?;
        Ok(id)
    }

    /// Sets the status of a criterion (only the human verifies) and saves.
    pub fn set_criterion(
        &mut self,
        id: &CriterionId,
        status: CriterionStatus,
        note: Option<&str>,
    ) -> Result<()> {
        klaub_context::set_criterion_status(&mut self.review.brief, id, status, note)?;
        self.review.updated_at = self.now();
        self.save()
    }

    // ---- coverage and finishing ------------------------------------------------------------

    /// How much of the change the human has marked as reviewed.
    pub fn coverage(&self) -> klaub_context::Coverage {
        let units: Vec<CoverageUnit> = self
            .analysis
            .queue
            .iter()
            .map(|item| CoverageUnit {
                id: item.unit.id.clone(),
                file: item.unit.file.clone(),
                module: self.analysis.modules.module_of(&item.unit.file),
                priority: item.priority,
                semantic: item.unit.semantic,
            })
            .collect();
        compute_coverage(
            &units,
            &self.review.reviewed_units,
            self.analysis.config.coverage.count_unit,
        )
    }

    /// Non-blocking notes to show before finishing.
    pub fn warnings_before_finish(&self) -> Vec<String> {
        let untested = self
            .analysis
            .queue
            .iter()
            .filter(|i| i.tags.contains(&klaub_core::finding::QueueTag::NoTests))
            .count();
        finish_warnings(
            &self.coverage(),
            &self.review.brief,
            untested,
            self.analysis.config.coverage.warn_on_unreviewed_p0_p1,
        )
    }

    /// Finishes the review: freezes the open comments into a feedback payload and saves.
    pub fn finish(&mut self) -> Result<FeedbackRecord> {
        let file = self.review_file();
        let now = self.now();
        let record =
            klaub_review::finish(&mut self.review, &self.ports.ids.random_hex(), now, &file)?;
        self.save()?;
        Ok(record)
    }

    /// Pastes the instruction into the agent's input, without `Enter`, after the caller confirmed.
    ///
    /// On success the comments become `prepared`. When the destination cannot be proven the
    /// reason is returned and nothing changes, so the caller can offer *Copy* or *Show file*.
    pub fn prepare_in_agent(&mut self, channel: &dyn AgentChannel) -> Result<PrepareOutcome> {
        let record = self.feedback_or_finish()?;
        let outcome = channel.prepare_input(&record.prompt)?;
        if outcome == PrepareOutcome::Prepared {
            let now = self.now();
            klaub_review::record_delivery(&mut self.review, Delivery::PreparedInAgent, now)?;
            self.save()?;
        }
        Ok(outcome)
    }

    /// Copies the instruction for the user to paste themselves.
    pub fn copy_feedback(&mut self, clipboard: &dyn Clipboard) -> Result<()> {
        let record = self.feedback_or_finish()?;
        clipboard.copy(&record.prompt)?;
        let now = self.now();
        klaub_review::record_delivery(&mut self.review, Delivery::Copied, now)?;
        self.save()
    }

    /// Notes that the file path was shown to the user.
    pub fn mark_shown(&mut self) -> Result<()> {
        self.feedback_or_finish()?;
        let now = self.now();
        klaub_review::record_delivery(&mut self.review, Delivery::Shown, now)?;
        self.save()
    }

    /// The human confirms that they submitted the instruction.
    pub fn confirm_submitted(&mut self) -> Result<()> {
        let now = self.now();
        klaub_review::confirm_submitted(&mut self.review, now)?;
        self.save()
    }

    fn feedback_or_finish(&mut self) -> Result<FeedbackRecord> {
        match (&self.review.feedback, self.review.status) {
            (Some(existing), status) if status != klaub_core::review::ReviewStatus::Draft => {
                Ok(existing.clone())
            }
            _ => self.finish(),
        }
    }

    // ---- snapshot drift and refresh -----------------------------------------------------------

    /// Cheaply checks whether the worktree still matches the snapshot.
    pub fn check_drift(&mut self) -> Result<Drift> {
        self.drift = self
            .ports
            .snapshots
            .probe(&self.analysis.snapshot, &self.request)?;
        Ok(self.drift)
    }

    /// Takes a new snapshot, relocates the anchors of existing comments (uncertain ones become
    /// `stale`), and keeps "reviewed" marks only for units whose content did not change.
    pub fn refresh(&mut self) -> Result<()> {
        let fresh = Arc::new(analysis::analyze(
            &self.ports,
            &self.request,
            &self.analysis.config,
        )?);
        let unchanged = |unit: &UnitId| -> bool {
            let before = Self::fingerprint(&self.analysis, unit);
            before.is_some() && before == Self::fingerprint(&fresh, unit)
        };
        let kept: Vec<UnitId> = self
            .review
            .reviewed_units
            .iter()
            .filter(|u| unchanged(u))
            .cloned()
            .collect();
        self.review.reviewed_units = kept;
        relocate_comments(&fresh, &mut self.review);
        self.review.snapshot_id = fresh.snapshot.id.clone();
        self.review.base_oid.clone_from(&fresh.snapshot.base_oid);
        self.review.updated_at = self.now();
        self.reports = Reports::from_analysis(&fresh);
        self.analysis = fresh;
        self.drift = Drift::Unchanged;
        self.flow = Self::read_flow(&self.ports, &self.analysis)?;
        self.sync_flow_ref();
        self.save()
    }

    fn fingerprint(analysis: &Analysis, unit: &UnitId) -> Option<String> {
        let info = analysis.units.get(unit)?;
        let entry = &analysis.files[info.file_index];
        match info.change_index {
            Some(index) => {
                let change = &entry.analysis.changes[index];
                let after = change.after.as_ref()?;
                let symbol = &entry.analysis.after.as_ref()?.symbols[after.index];
                Some(format!(
                    "{}:{}",
                    symbol.fingerprint.text_hash, symbol.signature
                ))
            }
            None => analysis.snapshot.files[info.file_index]
                .after
                .as_ref()
                .map(|b| b.hash().to_owned()),
        }
    }

    // ---- impact ---------------------------------------------------------------------------------

    /// Callers, entrypoints and callees of a symbol of the call graph.
    ///
    /// `None` when the graph does not know the symbol.
    pub fn impact(&self, key: &SymbolKey) -> Option<klaub_core::graph::ImpactReport> {
        let options = klaub_graph::ImpactOptions {
            max_depth: self.analysis.config.analysis.max_call_depth,
            ..klaub_graph::ImpactOptions::default()
        };
        self.analysis
            .graph
            .impact(key, &options, &self.analysis.modules)
    }

    /// The contract of one changed symbol: its declaration on both sides, the effects a parser
    /// already recorded, and the direct callers the call graph knows.
    ///
    /// `None` when `unit` is not a symbol of this analysis. A file-level unit has no contract to
    /// compare, and neither does an id the queue does not contain.
    pub fn contract(&self, unit: &UnitId) -> Option<ContractDelta> {
        let change = self.analysis.change_of(unit)?;
        let entry = self.analysis.file_of(unit)?;
        let before = Self::symbol_of(entry.analysis.before.as_deref(), change.before.as_ref());
        let after = Self::symbol_of(entry.analysis.after.as_deref(), change.after.as_ref());
        let impact = change
            .after
            .as_ref()
            .or(change.before.as_ref())
            .map(|side| SymbolKey::new(side.file.clone(), side.qualified_name.clone()))
            .and_then(|key| self.impact(&key));
        Some(crate::contract::inspect(
            change,
            before,
            after,
            impact.as_ref(),
        ))
    }

    /// The parsed symbol a change references, when the parse and the index both exist.
    fn symbol_of<'a>(
        parsed: Option<&'a klaub_core::symbol::ParsedFile>,
        side: Option<&SymbolRef>,
    ) -> Option<&'a Symbol> {
        let side = side?;
        parsed?.symbols.get(side.index)
    }

    /// The changed unit that holds `key`, when the symbol is part of the change.
    pub fn unit_of_symbol(&self, key: &SymbolKey) -> Option<UnitId> {
        self.analysis
            .queue
            .iter()
            .find(|item| item.unit.symbol.as_ref() == Some(key))
            .map(|item| item.unit.id.clone())
    }

    // ---- project code ----------------------------------------------------------------------------

    /// The code of any indexed project symbol (callers and callees reached through the impact view).
    ///
    /// Changed files are read from the snapshot; others from the worktree through the confined reader.
    pub fn code_view_of_symbol(
        &self,
        parser: &dyn klaub_core::ports::SourceParser,
        key: &SymbolKey,
    ) -> Option<crate::CodeView> {
        let text = match self
            .analysis
            .snapshot
            .file(&key.file)
            .and_then(|f| f.after_text())
        {
            Some(text) => text.to_string(),
            None => {
                let root = &self.analysis.snapshot.worktree.root;
                let limit = self.analysis.config.review.max_file_bytes;
                let bytes = self
                    .ports
                    .files
                    .read_bounded(root, Path::new(key.file.as_str()), limit)
                    .ok()?;
                String::from_utf8(bytes).ok()?
            }
        };
        self.analysis.symbol_view(parser, key, &text)
    }

    // ---- agent requests -----------------------------------------------------------------------------

    /// Pastes the request for a flow diagram into the agent's input, without `Enter`.
    ///
    /// Unlike [`ReviewSession::prepare_in_agent`] this finishes nothing and marks nothing.
    pub fn prepare_flow_request(&self, channel: &dyn AgentChannel) -> Result<PrepareOutcome> {
        channel.prepare_input(&self.flow.request_prompt)
    }

    /// What a test request would run, for the confirmation dialog.
    pub fn preview_test_run(request: &TestRunRequest) -> String {
        klaub_tests::preview(request)
    }

    // ---- tests ----------------------------------------------------------------------------------

    /// Test evidence for a unit: associated tests, matching results and coverage.
    pub fn test_panel(&self, unit: &UnitId) -> TestPanel {
        let key = self
            .analysis
            .change_of(unit)
            .and_then(|c| c.after.as_ref())
            .map(|r| SymbolKey::new(r.file.clone(), r.qualified_name.clone()));
        let links: Vec<TestLink> = key
            .as_ref()
            .map(|k| self.analysis.tests.links_for(k).cloned().collect())
            .unwrap_or_default();
        let link_refs: Vec<&TestLink> = links.iter().collect();
        let status =
            klaub_tests::evidence_status(&link_refs, &self.reports.results, self.reports.stale);
        let results = self
            .reports
            .results
            .iter()
            .filter(|r| {
                links
                    .iter()
                    .any(|l| klaub_tests::result_matches(&l.test, r))
            })
            .cloned()
            .collect();
        let coverage = key.and_then(|k| {
            self.reports
                .coverage
                .iter()
                .find(|c| c.file == k.file)
                .cloned()
        });
        TestPanel {
            status,
            links,
            results,
            sources: self.reports.sources.clone(),
            coverage,
        }
    }

    /// Builds the (not yet confirmed) request to run the associated tests of a unit.
    pub fn plan_test_run(&self, unit: &UnitId) -> Result<TestRunRequest> {
        let panel = self.test_panel(unit);
        if panel.links.is_empty() {
            return Err(Error::Unsupported(
                "no associated test was found for this symbol; nothing to select".to_owned(),
            ));
        }
        let mut ids: Vec<_> = panel.links.iter().map(|l| l.test.id.clone()).collect();
        ids.sort();
        ids.dedup();
        klaub_tests::run_request(
            &self.analysis.config.tests,
            &ids,
            &self.analysis.snapshot.worktree.root,
        )
    }

    /// Runs a previously previewed and **confirmed** request, then re-reads the configured reports.
    pub fn run_tests(
        &mut self,
        request: &TestRunRequest,
        cancel: &CancelToken,
    ) -> Result<TestRunOutcome> {
        let runner = self
            .ports
            .runner
            .clone()
            .ok_or_else(|| Error::Unsupported("this build cannot run tests".to_owned()))?;
        let outcome = runner.run(request, cancel)?;
        self.reload_reports();
        Ok(outcome)
    }

    /// Re-reads the test reports named in the configuration (after a run, or on demand).
    pub fn reload_reports(&mut self) {
        let root = self.analysis.snapshot.worktree.root.clone();
        let newest = self
            .analysis
            .snapshot
            .files
            .iter()
            .filter_map(|f| {
                self.ports
                    .files
                    .modified(&root, Path::new(f.path.as_str()))
                    .ok()
                    .flatten()
            })
            .max();
        let mut reports = Reports::default();
        for raw in &self.analysis.config.tests.report_paths {
            match analysis::read_report(&self.ports, &root, raw, newest) {
                Ok((source, results, coverage)) => {
                    reports.stale |= source.stale;
                    reports.results.extend(results);
                    reports.coverage.extend(coverage);
                    reports.sources.push(source);
                }
                Err(error) => reports.notes.push(format!("test report {raw}: {error}")),
            }
        }
        self.reports = reports;
    }

    // ---- flow ------------------------------------------------------------------------------------

    fn read_flow(ports: &Ports, analysis: &Analysis) -> Result<FlowView> {
        let snapshot = &analysis.snapshot;
        let expected_path = ports.flows.expected_path(&snapshot.id)?;
        let request_prompt = klaub_flow::request_prompt(&snapshot.id, expected_path.as_str());
        let max_bytes = analysis.config.flows.max_file_bytes;
        let missing = || FlowView {
            state: FlowState::Missing,
            flow: None,
            issues: Vec::new(),
            expected_path: expected_path.clone(),
            request_prompt: request_prompt.clone(),
        };
        if !analysis.config.flows.enabled {
            return Ok(missing());
        }
        let file = match ports.flows.read(&snapshot.id, max_bytes) {
            Ok(Some(file)) => file,
            Ok(None) => return Ok(missing()),
            Err(error) => {
                return Ok(FlowView {
                    state: FlowState::Invalid {
                        reason: error.to_string(),
                    },
                    flow: None,
                    issues: vec![error.to_string()],
                    expected_path,
                    request_prompt,
                });
            }
        };
        let known = |p: &RepoPath| {
            analysis.project_files.contains(p) || analysis.snapshot.file(p).is_some()
        };
        let symbols = |p: &RepoPath, name: &str| {
            analysis
                .parsed
                .get(p)
                .is_some_and(|f| f.find(name).is_some())
        };
        let connected = |a: &SymbolKey, b: &SymbolKey| analysis.graph.reaches(a, b, 6);
        let context = FlowContext {
            snapshot_id: &snapshot.id,
            worktree_id: &snapshot.worktree.id,
            base_oid: &snapshot.base_oid,
            max_bytes,
            knows_file: &known,
            symbol_exists: &symbols,
            connected: &connected,
        };
        let validation = klaub_flow::validate(&file.bytes, &context);
        let state = validation.state();
        Ok(match validation {
            Validation::Current(flow) | Validation::Stale { flow, .. } => FlowView {
                state,
                flow: Some(*flow),
                issues: Vec::new(),
                expected_path,
                request_prompt,
            },
            Validation::Invalid(issues) => FlowView {
                state,
                flow: None,
                issues,
                expected_path,
                request_prompt,
            },
        })
    }

    /// Re-reads the flow file (`Reload flow`).
    pub fn reload_flow(&mut self) -> Result<()> {
        self.flow = Self::read_flow(&self.ports, &self.analysis)?;
        self.sync_flow_ref();
        self.save()
    }

    fn sync_flow_ref(&mut self) {
        let weakest = self
            .flow
            .flow
            .as_ref()
            .and_then(|f| f.document.steps.iter().map(|s| s.confidence).max());
        self.review.flow = Some(FlowRef {
            status: self.flow.state.label().to_owned(),
            file: matches!(
                self.flow.state,
                FlowState::Current | FlowState::Stale { .. }
            )
            .then(|| self.flow.expected_path.to_string()),
            schema_version: self.flow.flow.as_ref().map(|f| f.document.schema_version),
            weakest_origin: weakest,
        });
    }

    // ---- export -------------------------------------------------------------------------------------

    /// The context an export needs.
    pub fn export_context(&self) -> ExportContext {
        let coverage = self.coverage();
        let delta = FindingDelta::of(self.analysis.queue.iter().flat_map(|i| i.findings.iter()));
        let linked: HashSet<&SymbolKey> = self
            .analysis
            .tests
            .links
            .iter()
            .map(|l| &l.target)
            .collect();
        let (mut passed, mut failed) = (0, 0);
        for key in &linked {
            let links: Vec<&TestLink> = self.analysis.tests.links_for(key).collect();
            match klaub_tests::evidence_status(&links, &self.reports.results, self.reports.stale) {
                EvidenceStatus::Passed => passed += 1,
                EvidenceStatus::Failed => failed += 1,
                _ => {}
            }
        }
        ExportContext {
            comments_file: self.review_file(),
            snapshot_changed_since_open: self.drift == Drift::Changed,
            coverage: Some((coverage.reviewed, coverage.total, coverage.unknown)),
            finding_delta: Some(delta),
            test_evidence: Some((
                linked.len(),
                passed,
                failed,
                linked.len().saturating_sub(passed + failed),
            )),
            flow: self.review.flow.clone(),
        }
    }

    /// The JSON export of the review.
    pub fn export_json(&self) -> serde_json::Value {
        klaub_review::export_json(&self.review, &self.export_context())
    }

    /// The Markdown export of the review.
    pub fn export_markdown(&self) -> String {
        klaub_review::export_markdown(&self.review)
    }

    /// Findings of the queue by temporal class (for the status line).
    pub fn introduced_findings(&self) -> usize {
        self.analysis
            .queue
            .iter()
            .flat_map(|i| &i.findings)
            .filter(|f| f.temporal == Temporal::Introduced)
            .count()
    }

    /// The match confidence of a comment's anchor.
    pub fn anchor_confidence(&self, id: &CommentId) -> Option<Confidence> {
        self.review.comment(id).map(|c| c.anchor_confidence)
    }
}
