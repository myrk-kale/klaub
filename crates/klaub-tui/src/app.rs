//! The state of the reader and the questions that can be asked of it.
//!
//! `App` owns the [`ReviewSession`] and everything the screen needs: the current mode, the unit
//! being read, scroll positions, filters, overlays and the last status message. Input handling
//! lives in [`crate::update`] and drawing in [`crate::ui`]; this module has no terminal code, so it
//! can be driven by tests.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use klaub_app::{CodeScope, CodeView, ReviewSession};
use klaub_core::finding::{Priority, QueueItem, QueueTag, Temporal};
use klaub_core::graph::{EdgeOrigin, ImpactReport, ImpactScope, SymbolKey};
use klaub_core::ids::{CommentId, UnitId};
use klaub_core::path::RepoPath;
use klaub_core::ports::{AgentChannel, Clipboard, SourceParser, TestRunRequest};
use klaub_core::review::{FeedbackRecord, FollowupStatus, Version};

use klaub_core::text::sanitize_single_line;

use crate::keymap::Keymap;
use crate::labels;
use crate::theme::Theme;
use crate::viewport::Viewport;

/// What the main area shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The prioritised list of changes.
    Queue,
    /// The module, file and symbol tree.
    Structure,
    /// One version of the code.
    Code,
    /// Callers, entrypoints and callees of the selected unit.
    Impact,
    /// All comments.
    Comments,
    /// The brief and acceptance criteria.
    Context,
    /// Test evidence of the selected unit.
    Tests,
    /// Comments of the previous round awaiting a decision.
    Followup,
    /// Review progress.
    Coverage,
    /// The explained flow of the change.
    Flow,
}

impl Mode {
    /// Every mode in `Tab` order.
    pub const ALL: [Self; 10] = [
        Self::Queue,
        Self::Structure,
        Self::Code,
        Self::Impact,
        Self::Comments,
        Self::Context,
        Self::Tests,
        Self::Followup,
        Self::Coverage,
        Self::Flow,
    ];

    /// The name shown in the title bar.
    pub fn label(self) -> &'static str {
        match self {
            Self::Queue => "Queue",
            Self::Structure => "Structure",
            Self::Code => "Code",
            Self::Impact => "Impact",
            Self::Comments => "Comments",
            Self::Context => "Context",
            Self::Tests => "Tests",
            Self::Followup => "Follow-up",
            Self::Coverage => "Coverage",
            Self::Flow => "Flow",
        }
    }

    /// The mode after (or before) this one in `Tab` order.
    #[must_use]
    pub fn cycle(self, forward: bool) -> Self {
        let position = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        let next = if forward {
            (position + 1) % Self::ALL.len()
        } else {
            (position + Self::ALL.len() - 1) % Self::ALL.len()
        };
        Self::ALL[next]
    }
}

/// Queue filters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    /// Show only this priority and more urgent ones.
    pub min_priority: Option<Priority>,
    /// Hide units already marked reviewed.
    pub unreviewed_only: bool,
    /// Only units with a finding introduced by this change.
    pub introduced_only: bool,
    /// Only units without an associated test.
    pub no_tests_only: bool,
    /// Only units in new files.
    pub new_only: bool,
    /// Only units whose change reaches at least this far.
    pub min_scope: Option<ImpactScope>,
    /// Only this module.
    pub module: Option<String>,
    /// Text that must appear in the label, path or a rule id.
    pub search: String,
}

impl Filters {
    /// Whether any filter is active.
    pub fn any(&self) -> bool {
        *self != Self::default()
    }

    /// A short description for the status bar.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(p) = self.min_priority {
            parts.push(format!("≤{}", p.label()));
        }
        if self.unreviewed_only {
            parts.push("unreviewed".to_owned());
        }
        if self.introduced_only {
            parts.push("introduced".to_owned());
        }
        if self.no_tests_only {
            parts.push("no-tests".to_owned());
        }
        if self.new_only {
            parts.push("new".to_owned());
        }
        if let Some(scope) = self.min_scope {
            parts.push(format!("scope ≥{}", scope.label()));
        }
        if let Some(m) = &self.module {
            parts.push(format!("module {m}"));
        }
        if !self.search.is_empty() {
            parts.push(format!("/{}", self.search));
        }
        parts.join(" · ")
    }
}

/// How deep the structure tree is expanded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// Modules only.
    Module,
    /// Modules and files.
    File,
    /// Everything down to symbols.
    Symbol,
}

/// How many visited units `<` can walk back through.
const HISTORY_LIMIT: usize = 64;

/// A row of the structure tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeKind {
    /// A module.
    Module(String),
    /// A file.
    File(RepoPath),
    /// A symbol or file unit.
    Unit(UnitId),
}

/// A row of the structure tree with its display data.
#[derive(Debug, Clone)]
pub struct TreeRow {
    /// What the row is.
    pub kind: TreeKind,
    /// Indentation level.
    pub depth: usize,
    /// Text to show.
    pub label: String,
    /// The most urgent priority below this row.
    pub priority: Option<Priority>,
    /// `(reviewed, total)` units at or below this row.
    pub progress: (usize, usize),
}

/// The code the reader is looking at.
#[derive(Debug, Clone, Default)]
pub struct CodeState {
    /// What to draw.
    pub view: Option<CodeView>,
    /// Which version.
    pub version: Option<Version>,
    /// Symbol or whole file.
    pub scope: Option<CodeScope>,
    /// The cursor line (index into `view.lines`).
    pub cursor: usize,
    /// First visible line.
    pub offset: usize,
    /// Rows available for code (set by the view each frame).
    pub height: usize,
    /// Columns scrolled to the right.
    pub hscroll: usize,
    /// Where a multi-line selection started.
    pub selection: Option<usize>,
    /// A symbol of the project that is not part of the change (reached from the impact view).
    pub detached: Option<klaub_core::graph::SymbolKey>,
}

/// What a row of the impact view is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactRowKind {
    /// A section title.
    Heading,
    /// An entrypoint or the path to it.
    Entrypoint,
    /// A caller.
    Caller,
    /// A callee.
    Callee,
    /// Explanatory text.
    Note,
}

/// One line of the impact view.
#[derive(Debug, Clone)]
pub struct ImpactRow {
    /// What it is.
    pub kind: ImpactRowKind,
    /// Text to show (already free of terminal sequences).
    pub text: String,
    /// The symbol it leads to, when it can be opened.
    pub target: Option<SymbolKey>,
    /// How certain the connection is.
    pub origin: Option<EdgeOrigin>,
}

/// The impact view of one symbol.
#[derive(Debug, Clone, Default)]
pub struct ImpactState {
    /// A symbol picked with `p`; otherwise the current unit's symbol is used.
    pub focus: Option<SymbolKey>,
    /// The graph answer.
    pub report: Option<ImpactReport>,
    /// What to draw.
    pub rows: Vec<ImpactRow>,
}

/// What a text editor overlay is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorPurpose {
    /// A new comment on a unit.
    NewComment {
        /// The unit.
        unit: UnitId,
        /// The version being read.
        version: Version,
        /// The selected lines.
        lines: Option<(u32, u32)>,
    },
    /// Editing an existing draft comment.
    EditComment(CommentId),
    /// The intent summary.
    Intent,
    /// A new acceptance criterion.
    Criterion,
    /// The note for a follow-up decision.
    FollowupNote {
        /// The comment.
        comment: CommentId,
        /// The decision.
        status: FollowupStatus,
    },
}

/// A small text editor shown over the screen.
#[derive(Debug, Clone)]
pub struct Editor {
    /// The heading.
    pub title: String,
    /// What it is for.
    pub purpose: EditorPurpose,
    /// The text so far.
    pub text: String,
    /// Optional priority of a comment.
    pub severity: Option<Priority>,
    /// Whether the field accepts several lines (`Ctrl-J` inserts a line break; `Enter` always saves).
    pub multiline: bool,
    /// Why the last attempt to save failed.
    pub error: Option<String>,
}

/// Something the user must confirm.
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    /// Paste the instruction into the agent's input.
    PrepareInAgent,
    /// Paste the request for a flow diagram into the agent's input.
    PrepareFlowRequest,
    /// Run the user's test command.
    RunTests(TestRunRequest),
    /// Delete a draft comment.
    DeleteComment(CommentId),
    /// Take a new snapshot.
    Refresh,
}

/// A yes/no question with the facts needed to answer it.
#[derive(Debug, Clone)]
pub struct Confirm {
    /// The heading.
    pub title: String,
    /// Detail lines (destination, exact text, warnings).
    pub lines: Vec<String>,
    /// What happens on yes.
    pub action: ConfirmAction,
}

/// The finishing flow.
#[derive(Debug, Clone)]
pub enum FinishStage {
    /// Waiting for confirmation, with the non-blocking warnings.
    Confirm {
        /// Warnings to show.
        warnings: Vec<String>,
    },
    /// Finished; choosing how to deliver.
    Deliver {
        /// The payload.
        record: FeedbackRecord,
        /// The selected option.
        selected: usize,
    },
}

/// Delivery options after finishing.
pub const DELIVERY_OPTIONS: [&str; 4] = ["Prepare in agent", "Copy feedback", "Show file", "Close"];

/// A layer over the main screen.
#[derive(Debug, Clone)]
pub enum Overlay {
    /// The key reference.
    Help,
    /// The action palette.
    Palette {
        /// What was typed.
        input: String,
        /// The highlighted entry.
        selected: usize,
    },
    /// The search field.
    Search {
        /// What was typed.
        input: String,
    },
    /// A text editor.
    Editor(Editor),
    /// A question.
    Confirm(Confirm),
    /// The reasons behind a priority.
    Explain(UnitId),
    /// What changed about one symbol's contract.
    Contract(UnitId),
    /// Finishing the review.
    Finish(FinishStage),
    /// A one-time explanation. Any key closes it.
    Notice {
        /// The heading.
        title: String,
        /// The text, wrapped when drawn.
        text: String,
    },
}

/// How a status message is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Information.
    Info,
    /// Something to watch.
    Warn,
    /// Something failed.
    Error,
}

/// The reader.
pub struct App {
    /// The review being worked on.
    pub session: ReviewSession,
    /// Highlighting and parsing for ad-hoc views.
    pub parser: Arc<dyn SourceParser>,
    /// The agent's input, when the reader runs inside `klaub wrap`.
    pub agent: Option<Arc<dyn AgentChannel>>,
    /// The clipboard.
    pub clipboard: Option<Arc<dyn Clipboard>>,
    /// Colours.
    pub theme: Theme,
    /// Which character triggers which action (the defaults plus `[keys]`).
    pub keymap: Keymap,
    /// What the main area shows.
    pub mode: Mode,
    /// Modes visited before the current one (for `Esc`).
    pub history: Vec<Mode>,
    /// The unit that Code, Impact and Tests refer to.
    pub unit: Option<UnitId>,
    /// Units visited before the current one, oldest first. `<` walks back through them.
    pub(crate) unit_back: Vec<UnitId>,
    /// Units left by walking back, nearest last. `>` walks forward; any other move clears them.
    pub(crate) unit_forward: Vec<UnitId>,
    /// Units marked for a quick return. Held only while the reader is open.
    pub(crate) bookmarks: Vec<UnitId>,
    /// Queue filters.
    pub filters: Filters,
    /// Scrolling of the queue.
    pub queue_view: Viewport,
    /// Scrolling of the structure tree.
    pub structure_view: Viewport,
    /// How deep the tree is open.
    pub depth: Depth,
    /// Files whose symbols are hidden.
    pub collapsed: HashSet<RepoPath>,
    /// The code being read.
    pub code: CodeState,
    /// The impact of the current symbol.
    pub impact: ImpactState,
    /// Scrolling of the impact view.
    pub impact_view: Viewport,
    /// Scrolling of the comments list.
    pub comments_view: Viewport,
    /// Scrolling of the context view.
    pub context_view: Viewport,
    /// Scrolling of the tests view.
    pub tests_view: Viewport,
    /// Scrolling of the follow-up list.
    pub followup_view: Viewport,
    /// Scrolling of the coverage view.
    pub coverage_view: Viewport,
    /// Scrolling of the flow view.
    pub flow_view: Viewport,
    /// A layer over the screen.
    pub overlay: Option<Overlay>,
    /// The last message.
    pub status: Option<(String, StatusKind)>,
    /// An action to run after the next frame is drawn (it blocks, so the screen says so first).
    pub deferred: Option<ConfirmAction>,
    /// The tail of the output of the last test run started from the reader.
    pub test_output: Option<String>,
    /// `q` was pressed once; a second press leaves.
    pub quit_armed: bool,
    /// Leave the reader.
    pub should_quit: bool,
    /// When the worktree was last probed for changes.
    pub last_probe: Instant,
}

/// What the first review of a worktree explains about `.klaub/`.
const STORAGE_NOTICE: &str = "Your comments, briefs and flow diagrams are saved in a .klaub/ folder inside this repository. It is local data.\n\nKlaub does not add it to .gitignore or .git/info/exclude, and it never commits it. If you do not want .klaub/ in git, add it to .gitignore yourself; if you want to share a flow diagram, commit .klaub/flows/ yourself.\n\nKlaub always leaves .klaub/ out of the changes it reviews, so saving a comment never starts a new snapshot.";

impl App {
    /// Opens with the explanation of where Klaub keeps its files, for a worktree that has none yet.
    ///
    /// The text says what the spec requires and no more: `.klaub/` is local data, Klaub does not
    /// touch `.gitignore` or `.git/info/exclude` and never commits it, and it never reviews it.
    #[must_use]
    pub fn with_storage_notice(mut self) -> Self {
        self.overlay = Some(Overlay::Notice {
            title: "Where Klaub keeps its files".to_owned(),
            text: STORAGE_NOTICE.to_owned(),
        });
        self
    }

    /// Creates the reader over an open session.
    pub fn new(
        session: ReviewSession,
        parser: Arc<dyn SourceParser>,
        agent: Option<Arc<dyn AgentChannel>>,
        clipboard: Option<Arc<dyn Clipboard>>,
    ) -> Self {
        let theme = Theme::new(session.analysis().config.ui.theme);
        let keymap = match Keymap::from_overrides(&session.analysis().config.keys) {
            Ok(keymap) => keymap,
            Err(error) => {
                tracing::warn!(%error, "ignoring the [keys] table; using the default keys");
                Keymap::default()
            }
        };
        let has_previous_round = session.review().previous_review_id.is_some();
        let mut app = Self {
            session,
            parser,
            agent,
            clipboard,
            theme,
            keymap,
            mode: Mode::Queue,
            history: Vec::new(),
            unit: None,
            bookmarks: Vec::new(),
            unit_back: Vec::new(),
            unit_forward: Vec::new(),
            filters: Filters::default(),
            queue_view: Viewport::default(),
            structure_view: Viewport::default(),
            depth: Depth::Symbol,
            collapsed: HashSet::new(),
            code: CodeState::default(),
            impact: ImpactState::default(),
            impact_view: Viewport::default(),
            comments_view: Viewport::default(),
            context_view: Viewport::default(),
            tests_view: Viewport::default(),
            followup_view: Viewport::default(),
            coverage_view: Viewport::default(),
            flow_view: Viewport::default(),
            overlay: None,
            status: None,
            deferred: None,
            test_output: None,
            quit_armed: false,
            should_quit: false,
            last_probe: Instant::now(),
        };
        app.unit = app
            .session
            .review()
            .last_unit
            .clone()
            .filter(|unit| app.session.analysis().item(unit).is_some())
            .or_else(|| app.visible_queue().first().map(|i| i.unit.id.clone()));
        if let Some(unit) = app.unit.clone() {
            app.select_in_queue(&unit);
        }
        if has_previous_round {
            app.status = Some((
                "Continuing a previous round: decide each carried-over comment in Follow-up (D)."
                    .to_owned(),
                StatusKind::Info,
            ));
        }
        app
    }

    /// Sets the status line.
    pub fn say(&mut self, kind: StatusKind, text: impl Into<String>) {
        self.status = Some((text.into(), kind));
    }

    // ---- queue -----------------------------------------------------------------------------

    fn passes(&self, item: &QueueItem) -> bool {
        let filters = &self.filters;
        if filters.min_priority.is_some_and(|p| item.priority > p) {
            return false;
        }
        if filters.unreviewed_only && self.session.review().is_reviewed(&item.unit.id) {
            return false;
        }
        if filters.introduced_only
            && !item
                .findings
                .iter()
                .any(|f| f.temporal == Temporal::Introduced)
        {
            return false;
        }
        if filters.no_tests_only && !item.tags.contains(&QueueTag::NoTests) {
            return false;
        }
        if filters.new_only && !item.tags.contains(&QueueTag::NewFile) {
            return false;
        }
        if let Some(min) = filters.min_scope
            && item.scope.is_none_or(|scope| scope < min)
        {
            return false;
        }
        if let Some(module) = &filters.module
            && &self.session.analysis().modules.module_of(&item.unit.file) != module
        {
            return false;
        }
        if !filters.search.is_empty() && !self.matches_search(item) {
            return false;
        }
        true
    }

    /// Whether the search needle occurs in the unit's label, file, rules, comments, tests or criteria.
    fn matches_search(&self, item: &QueueItem) -> bool {
        let needle = self.filters.search.to_lowercase();
        let contains = |text: &str| text.to_lowercase().contains(&needle);
        contains(&item.unit.label)
            || contains(item.unit.file.as_str())
            || item.findings.iter().any(|f| contains(&f.rule_id))
            || self
                .session
                .comments_on(&item.unit.id)
                .iter()
                .any(|comment| contains(comment.comment_id.as_str()))
            || self
                .session
                .test_panel(&item.unit.id)
                .links
                .iter()
                .any(|link| contains(link.test.id.as_str()))
            || self
                .session
                .brief()
                .acceptance_criteria
                .iter()
                .any(|criterion| contains(&criterion.text))
    }

    /// Queue items that pass the filters, in review order.
    pub fn visible_queue(&self) -> Vec<&QueueItem> {
        self.session
            .analysis()
            .queue
            .iter()
            .filter(|i| self.passes(i))
            .collect()
    }

    /// The queue item of the current unit.
    pub fn current_item(&self) -> Option<&QueueItem> {
        self.unit
            .as_ref()
            .and_then(|u| self.session.analysis().item(u))
    }

    /// Makes `unit` the current one and remembers it for the next time the review is opened.
    pub(crate) fn adopt(&mut self, unit: UnitId) {
        let changed = self.unit.as_ref() != Some(&unit);
        if changed {
            // A new move starts a new branch of history, as in a browser.
            if let Some(previous) = self.unit.take() {
                self.unit_back.push(previous);
                if self.unit_back.len() > HISTORY_LIMIT {
                    self.unit_back.remove(0);
                }
            }
            self.unit_forward.clear();
        }
        self.unit = Some(unit);
        if changed && let Some(unit) = self.unit.clone() {
            // A failed save leaves the cursor where the reader put it; the status says why.
            if let Err(error) = self.session.remember_position(&unit) {
                self.say(StatusKind::Error, error.to_string());
            }
        }
    }

    /// Walks to the unit visited before this one. `false` when there is none.
    pub(crate) fn history_back(&mut self) -> bool {
        let Some(target) = self.unit_back.pop() else {
            return false;
        };
        if let Some(current) = self.unit.take() {
            self.unit_forward.push(current);
        }
        self.show_history_unit(&target);
        true
    }

    /// Walks to the unit that `<` left. `false` when there is none.
    pub(crate) fn history_forward(&mut self) -> bool {
        let Some(target) = self.unit_forward.pop() else {
            return false;
        };
        if let Some(current) = self.unit.take() {
            self.unit_back.push(current);
        }
        self.show_history_unit(&target);
        true
    }

    /// Puts the reader on a unit taken from the history, without touching the other direction.
    fn show_history_unit(&mut self, unit: &UnitId) {
        self.unit = Some(unit.clone());
        if let Err(error) = self.session.remember_position(unit) {
            self.say(StatusKind::Error, error.to_string());
        }
        self.select_in_queue(unit);
        if matches!(self.mode, Mode::Code) {
            let version = self.code.version;
            self.code = CodeState {
                version: version.or_else(|| Some(self.default_version(unit))),
                scope: Some(CodeScope::Symbol),
                ..CodeState::default()
            };
            self.rebuild_code();
        }
        if matches!(self.mode, Mode::Impact) {
            self.rebuild_impact();
        }
    }

    /// Selects the queue row of `unit` if it is visible.
    pub fn select_in_queue(&mut self, unit: &UnitId) {
        let position = self.visible_queue().iter().position(|i| &i.unit.id == unit);
        let len = self.visible_queue().len();
        if let Some(position) = position {
            self.queue_view.select(position, len);
        }
    }

    /// The unit under the queue cursor.
    pub fn queue_selection(&self) -> Option<UnitId> {
        self.visible_queue()
            .get(self.queue_view.selected)
            .map(|i| i.unit.id.clone())
    }

    /// The next (or previous) visible unit that is not marked reviewed, wrapping around.
    pub fn next_unreviewed(
        &self,
        from: Option<&UnitId>,
        forward: bool,
        critical_only: bool,
    ) -> Option<UnitId> {
        let queue = self.visible_queue();
        if queue.is_empty() {
            return None;
        }
        let start = from
            .and_then(|u| queue.iter().position(|i| &i.unit.id == u))
            .unwrap_or(if forward { queue.len() - 1 } else { 0 });
        (1..=queue.len())
            .map(|step| {
                if forward {
                    (start + step) % queue.len()
                } else {
                    (start + queue.len() - step % queue.len()) % queue.len()
                }
            })
            .map(|i| queue[i])
            .find(|item| {
                !self.session.review().is_reviewed(&item.unit.id)
                    && (!critical_only || item.priority.is_high())
            })
            .map(|item| item.unit.id.clone())
    }

    /// Modules present in the queue, sorted.
    pub fn modules(&self) -> Vec<String> {
        let mut modules: Vec<String> = self
            .session
            .analysis()
            .queue
            .iter()
            .map(|i| self.session.analysis().modules.module_of(&i.unit.file))
            .collect();
        modules.sort();
        modules.dedup();
        modules
    }

    // ---- structure ---------------------------------------------------------------------------

    /// Rows of the structure tree at the current depth.
    pub fn tree_rows(&self) -> Vec<TreeRow> {
        let analysis = self.session.analysis();
        let reviewed = |u: &UnitId| self.session.review().is_reviewed(u);
        let mut by_module: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<RepoPath, Vec<&QueueItem>>,
        > = std::collections::BTreeMap::new();
        for item in analysis.queue.iter().filter(|i| self.passes(i)) {
            by_module
                .entry(analysis.modules.module_of(&item.unit.file))
                .or_default()
                .entry(item.unit.file.clone())
                .or_default()
                .push(item);
        }
        let mut rows = Vec::new();
        for (module, files) in by_module {
            let all: Vec<&&QueueItem> = files.values().flatten().collect();
            rows.push(TreeRow {
                kind: TreeKind::Module(module.clone()),
                depth: 0,
                label: module,
                priority: all.iter().map(|i| i.priority).min(),
                progress: (
                    all.iter().filter(|i| reviewed(&i.unit.id)).count(),
                    all.len(),
                ),
            });
            if self.depth == Depth::Module {
                continue;
            }
            for (file, items) in files {
                rows.push(TreeRow {
                    kind: TreeKind::File(file.clone()),
                    depth: 1,
                    label: file.to_string(),
                    priority: items.iter().map(|i| i.priority).min(),
                    progress: (
                        items.iter().filter(|i| reviewed(&i.unit.id)).count(),
                        items.len(),
                    ),
                });
                if self.depth == Depth::File || self.collapsed.contains(&file) {
                    continue;
                }
                let mut items = items;
                items.sort_by_key(|i| {
                    analysis
                        .change_of(&i.unit.id)
                        .and_then(|c| c.after.as_ref().or(c.before.as_ref()))
                        .map_or(0, |r| r.span.start_line)
                });
                for item in items {
                    let nesting = item
                        .unit
                        .symbol
                        .as_ref()
                        .map_or(0, |s| s.qualified_name.matches('.').count());
                    rows.push(TreeRow {
                        kind: TreeKind::Unit(item.unit.id.clone()),
                        depth: 2 + nesting,
                        label: item
                            .unit
                            .label
                            .rsplit('.')
                            .next()
                            .map_or_else(|| item.unit.label.clone(), str::to_owned),
                        priority: Some(item.priority),
                        progress: (usize::from(reviewed(&item.unit.id)), 1),
                    });
                }
            }
        }
        rows
    }

    // ---- code ----------------------------------------------------------------------------------

    /// Opens `unit` in the Code mode.
    pub fn open_unit(&mut self, unit: &UnitId, version: Option<Version>) {
        self.adopt(unit.clone());
        self.select_in_queue(unit);
        let fallback = self.default_version(unit);
        self.code = CodeState {
            version: Some(version.unwrap_or(fallback)),
            scope: Some(CodeScope::Symbol),
            ..CodeState::default()
        };
        self.rebuild_code();
        self.go(Mode::Code);
    }

    /// The version a unit is first read in: the new code, or the old one when it was removed.
    fn default_version(&self, unit: &UnitId) -> Version {
        match self.session.analysis().change_of(unit) {
            Some(change) if change.after.is_none() => Version::Before,
            _ => match self.session.analysis().file_of(unit) {
                Some(file)
                    if file.analysis.status == klaub_core::snapshot::ChangeStatus::Deleted =>
                {
                    Version::Before
                }
                _ => Version::After,
            },
        }
    }

    /// Rebuilds the code view after the unit, version or scope changed.
    pub fn rebuild_code(&mut self) {
        let (Some(unit), Some(version)) = (self.unit.clone(), self.code.version) else {
            return;
        };
        let scope = self.code.scope.unwrap_or(CodeScope::Symbol);
        let view = if let Some(key) = &self.code.detached {
            self.session.code_view_of_symbol(self.parser.as_ref(), key)
        } else {
            self.session
                .analysis()
                .code_view(self.parser.as_ref(), &unit, version, scope)
        };
        // Start on the first change when there is one, otherwise at the top.
        let first_change = view.as_ref().and_then(|v| {
            v.changes
                .first()
                .and_then(|n| v.lines.iter().position(|l| l.number == *n))
        });
        self.code.cursor = first_change.unwrap_or(0);
        self.code.offset = self.code.cursor.saturating_sub(3);
        self.code.selection = None;
        self.code.view = view;
    }

    /// Switches mode, remembering where we came from.
    pub fn go(&mut self, mode: Mode) {
        if mode != self.mode {
            self.history.push(self.mode);
            if self.history.len() > 32 {
                self.history.remove(0);
            }
            self.mode = mode;
        }
        if mode == Mode::Impact {
            self.rebuild_impact();
        }
    }

    /// The comments of the previous round that still await a decision.
    pub fn followups(&self) -> Vec<&klaub_core::review::Comment> {
        let current = &self.session.review().review_id;
        self.session
            .review()
            .comments
            .iter()
            .filter(|c| {
                c.origin_review_id.as_ref().is_some_and(|o| o != current)
                    || c.followup_status != FollowupStatus::Pending
            })
            .collect()
    }
    // ---- impact ----------------------------------------------------------------------------------

    /// The symbol the impact view is about.
    pub fn impact_subject(&self) -> Option<SymbolKey> {
        self.impact.focus.clone().or_else(|| {
            self.current_item()
                .and_then(|item| item.unit.symbol.clone())
        })
    }

    /// Recomputes the impact rows for the current subject.
    pub fn rebuild_impact(&mut self) {
        let mut rows = Vec::new();
        let note = |text: &str| ImpactRow {
            kind: ImpactRowKind::Note,
            text: text.to_owned(),
            target: None,
            origin: None,
        };
        let heading = |text: String| ImpactRow {
            kind: ImpactRowKind::Heading,
            text,
            target: None,
            origin: None,
        };
        let name = |key: &SymbolKey| {
            sanitize_single_line(&format!("{} · {}", key.qualified_name, key.file))
        };
        let Some(subject) = self.impact_subject() else {
            rows.push(note(
                "This unit is a whole file, not a symbol, so it has no call graph.",
            ));
            self.impact.report = None;
            self.impact.rows = rows;
            self.impact_view = Viewport::default();
            return;
        };
        let report = self.session.impact(&subject);
        match &report {
            None => rows.push(note(
                "This symbol is not in the call graph (not indexed, or in an unsupported language).",
            )),
            Some(report) => {
                rows.push(heading(format!(
                    "{} · {} direct caller(s) · {} entrypoint(s), {} external",
                    report.scope.label(),
                    report.direct_callers(),
                    report.entrypoints.len(),
                    report.external_entrypoints(),
                )));
                let c = &report.coverage;
                rows.push(note(&format!(
                    "evidence: {} resolved · {} heuristic · {} unresolved call(s){}",
                    c.resolved_edges,
                    c.heuristic_edges,
                    c.unresolved_calls,
                    if c.truncated { " · depth limit reached" } else { "" },
                )));
                rows.push(heading("Entrypoints that can reach it".to_owned()));
                if report.entrypoints.is_empty() {
                    rows.push(note(
                        "none found (it may be reached in ways the analysis cannot see)",
                    ));
                }
                for reach in &report.entrypoints {
                    let entry = &reach.entrypoint;
                    rows.push(ImpactRow {
                        kind: ImpactRowKind::Entrypoint,
                        text: format!(
                            "{} [{}, {}] {}",
                            name(&entry.symbol),
                            labels::entrypoint_kind(&entry.kind),
                            labels::entrypoint_scope(entry.scope),
                            sanitize_single_line(&entry.evidence),
                        ),
                        target: Some(entry.symbol.clone()),
                        origin: Some(reach.origin),
                    });
                    let path: Vec<String> = reach
                        .path
                        .iter()
                        .map(|step| sanitize_single_line(&step.symbol.qualified_name))
                        .collect();
                    rows.push(ImpactRow {
                        kind: ImpactRowKind::Note,
                        text: format!("    {}", path.join(" → ")),
                        target: None,
                        origin: None,
                    });
                }
                rows.push(heading("Callers".to_owned()));
                if report.callers.is_empty() {
                    rows.push(note("no callers found"));
                }
                for level in &report.callers {
                    for step in &level.callers {
                        rows.push(ImpactRow {
                            kind: ImpactRowKind::Caller,
                            text: format!("{}  {}", level.depth, name(&step.symbol)),
                            target: Some(step.symbol.clone()),
                            origin: Some(step.origin),
                        });
                    }
                }
                rows.push(heading("Calls".to_owned()));
                if report.callees.is_empty() {
                    rows.push(note("no calls into the project"));
                }
                for step in &report.callees {
                    rows.push(ImpactRow {
                        kind: ImpactRowKind::Callee,
                        text: name(&step.symbol),
                        target: Some(step.symbol.clone()),
                        origin: Some(step.origin),
                    });
                }
            }
        }
        self.impact.report = report;
        self.impact.rows = rows;
        self.impact_view = Viewport::default();
    }

    /// Opens the code of a symbol: the changed unit when it is one, otherwise a read-only view.
    pub fn open_symbol(&mut self, key: &SymbolKey) {
        if let Some(unit) = self.session.unit_of_symbol(key) {
            self.open_unit(&unit, None);
            return;
        }
        let view = self.session.code_view_of_symbol(self.parser.as_ref(), key);
        if view.is_none() {
            self.say(
                StatusKind::Warn,
                format!(
                    "Cannot show {}: the file is unreadable or not indexed.",
                    sanitize_single_line(&key.qualified_name)
                ),
            );
            return;
        }
        self.code = CodeState {
            version: Some(Version::After),
            scope: Some(CodeScope::Symbol),
            detached: Some(key.clone()),
            ..CodeState::default()
        };
        self.code.cursor = 0;
        self.code.view = view;
        self.go(Mode::Code);
    }
}
