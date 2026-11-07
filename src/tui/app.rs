//! The state of the reader and the questions that can be asked of it.
//!
//! `App` owns the [`ReviewSession`] and everything the screen needs: the current mode, the unit
//! being read, scroll positions, filters, overlays and the last status message. Input handling
//! lives in [`crate::tui::update`] and drawing in [`crate::tui::ui`]; this module has no terminal code, so it
//! can be driven by tests.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use crate::app::{CodeScope, CodeView, ReviewSession};
use crate::domain::finding::{Priority, QueueItem, QueueTag, Temporal};
use crate::domain::graph::{EdgeOrigin, ImpactReport, ImpactScope, SymbolKey};
use crate::domain::i18n::{Locale, tui as t};
use crate::domain::ids::{CommentId, UnitId};
use crate::domain::path::RepoPath;
use crate::domain::ports::{AgentChannel, Clipboard, SourceParser, TestRunRequest};
use crate::domain::review::{FeedbackRecord, FollowupStatus, Version};

use crate::domain::text::sanitize_single_line;

use crate::tui::keymap::Keymap;
use crate::tui::labels;
use crate::tui::theme::Theme;
use crate::tui::viewport::Viewport;

/// What the main area shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The prioritised list of changes.
    Queue,
    /// The module, file and symbol tree.
    Structure,
    /// The changed files as one flat list.
    Files,
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
    pub const ALL: [Self; 11] = [
        Self::Queue,
        Self::Structure,
        Self::Files,
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
    pub fn label(self, locale: Locale) -> &'static str {
        match self {
            Self::Queue => t::mode_queue(locale),
            Self::Structure => t::mode_structure(locale),
            Self::Files => t::mode_files(locale),
            Self::Code => t::mode_code(locale),
            Self::Impact => t::mode_impact(locale),
            Self::Comments => t::mode_comments(locale),
            Self::Context => t::mode_context(locale),
            Self::Tests => t::mode_tests(locale),
            Self::Followup => t::mode_followup(locale),
            Self::Coverage => t::mode_coverage(locale),
            Self::Flow => t::mode_flow(locale),
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
    pub fn summary(&self, locale: Locale) -> String {
        let mut parts = Vec::new();
        if let Some(p) = self.min_priority {
            parts.push(format!("≤{}", p.label()));
        }
        if self.unreviewed_only {
            parts.push(t::filter_unreviewed(locale).to_owned());
        }
        if self.introduced_only {
            parts.push(t::filter_introduced(locale).to_owned());
        }
        if self.no_tests_only {
            parts.push(t::filter_no_tests(locale).to_owned());
        }
        if self.new_only {
            parts.push(t::filter_new(locale).to_owned());
        }
        if let Some(scope) = self.min_scope {
            parts.push(t::filter_scope(locale, labels::reach(locale, scope)));
        }
        if let Some(m) = &self.module {
            parts.push(t::filter_module(locale, m));
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

/// A row of the flow list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowLine {
    /// The step, its depth and how it was reached.
    pub row: crate::app::FlowRow,
    /// Other steps hang below this one.
    pub has_branches: bool,
    /// Those steps are folded away.
    pub folded: bool,
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

/// A changed file in the flat list.
#[derive(Debug, Clone)]
pub struct FileRow {
    /// The path as shown.
    pub path: String,
    /// The most urgent priority among its units.
    pub priority: Priority,
    /// `(reviewed, total)` units in the file.
    pub progress: (usize, usize),
    /// The unit shown when the file is picked: its most urgent one.
    pub first: UnitId,
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
    pub detached: Option<crate::domain::graph::SymbolKey>,
    /// Where the code lines were drawn last, so a mouse click can pick a line.
    pub area: ratatui::layout::Rect,
}

/// A key held down to look at the previous version. Releasing it puts the version back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peek {
    /// The character being held.
    pub key: char,
    /// The version to go back to on release.
    pub restore: Version,
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
    /// The second line of the row: where the symbol lives. For a heading, how many rows follow it.
    pub detail: String,
    /// The third line of the row, when it has one: the path an entrypoint takes to the symbol.
    pub extra: String,
    /// How many calls away the symbol is, to indent it. Zero for everything but a caller.
    pub depth: usize,
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
    /// The one-line answer shown above the rows.
    pub summary: String,
    /// How much of that answer is certain, shown under it.
    pub evidence: String,
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
    /// Silence a rule for this review (a false positive).
    MuteRule(String),
    /// Bring back every rule this review silenced.
    RestoreRules,
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

/// How many delivery options there are after finishing.
pub const DELIVERY_COUNT: usize = 4;

/// Delivery options after finishing.
pub fn delivery_options(locale: Locale) -> [&'static str; DELIVERY_COUNT] {
    [
        t::delivery_prepare(locale),
        t::delivery_copy(locale),
        t::delivery_show(locale),
        t::delivery_close(locale),
    ]
}

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
    /// The language every sentence is shown in.
    pub locale: Locale,
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
    /// Cursor of the flat list of changed files.
    pub files_view: Viewport,
    /// Whether the user opened (`Some(true)`) or closed (`Some(false)`) the symbol panel. `None`
    /// leaves it to the width of the terminal.
    pub inspector: Option<bool>,
    /// Whether the symbol panel was drawn last frame.
    pub inspector_shown: bool,
    /// Whether the terminal was wide enough for the symbol panel last frame.
    pub inspector_fits: bool,
    /// Which list the left pane shows: the last of Queue, Structure and Files that was current.
    pub list: Mode,
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
    /// Steps of the flow whose branches are folded away.
    pub flow_folded: HashSet<usize>,
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
    /// The adapters for the programs a paste may go to. Empty by default, which means every
    /// program gets the generic adapter and nothing is assumed about its state.
    pub harnesses: crate::domain::harness::HarnessRegistry,
    /// Where each tab was drawn last, so a mouse click can switch to it.
    pub tab_hits: Vec<(ratatui::layout::Rect, Mode)>,
    /// Whether the terminal reports key releases. Only then can a key be held to look at the
    /// previous version.
    pub hold_supported: bool,
    /// The key being held to look at the previous version, if any.
    pub peek: Option<Peek>,
}

impl App {
    /// Opens with the explanation of where Klaub keeps its files, for a worktree that has none yet.
    ///
    /// The text says what the spec requires and no more: `.klaub/` is local data, Klaub does not
    /// touch `.gitignore` or `.git/info/exclude` and never commits it, and it never reviews it.
    #[must_use]
    pub fn with_storage_notice(mut self) -> Self {
        self.overlay = Some(Overlay::Notice {
            title: t::storage_notice_title(self.locale).to_owned(),
            text: t::storage_notice_text(self.locale).to_owned(),
        });
        self
    }

    /// Replaces the adapters consulted before a paste. The confirmation names the adapter it used.
    #[must_use]
    pub fn with_harnesses(mut self, harnesses: crate::domain::harness::HarnessRegistry) -> Self {
        self.harnesses = harnesses;
        self
    }

    /// Whether a text field or the palette is open, where keys must arrive as typed.
    #[must_use]
    pub fn text_input_open(&self) -> bool {
        matches!(
            self.overlay,
            Some(Overlay::Editor(_) | Overlay::Search { .. } | Overlay::Palette { .. })
        )
    }

    /// Creates the reader over an open session.
    pub fn new(
        session: ReviewSession,
        parser: Arc<dyn SourceParser>,
        agent: Option<Arc<dyn AgentChannel>>,
        clipboard: Option<Arc<dyn Clipboard>>,
    ) -> Self {
        let theme = Theme::new(session.analysis().config.ui.theme);
        let locale = session.analysis().config.ui.language.locale_or_default();
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
            locale,
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
            files_view: Viewport::default(),
            inspector: None,
            inspector_shown: false,
            inspector_fits: false,
            list: Mode::Queue,
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
            flow_folded: HashSet::new(),
            overlay: None,
            status: None,
            deferred: None,
            test_output: None,
            quit_armed: false,
            should_quit: false,
            last_probe: Instant::now(),
            harnesses: crate::domain::harness::HarnessRegistry::default(),
            tab_hits: Vec::new(),
            hold_supported: false,
            peek: None,
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
            app.status = Some((t::continuing_round(locale).to_owned(), StatusKind::Info));
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

    // ---- flow ---------------------------------------------------------------------------------

    /// The rows of the flow the reviewer can see: the layout minus the branches they folded.
    ///
    /// Each row says whether it has branches below it, so the view can draw the fold mark. A step
    /// that was already shown higher up (a cycle or a join) never has branches of its own.
    pub fn flow_rows(&self) -> Vec<FlowLine> {
        let Some(flow) = &self.session.flow().flow else {
            return Vec::new();
        };
        let all = crate::app::flow_layout(&flow.document);
        let mut visible = Vec::with_capacity(all.len());
        let mut hidden_below: Option<usize> = None;
        for (index, row) in all.iter().enumerate() {
            if let Some(depth) = hidden_below {
                if row.depth > depth {
                    continue;
                }
                hidden_below = None;
            }
            let has_branches = all
                .get(index + 1)
                .is_some_and(|next| next.depth > row.depth);
            let folded = has_branches && self.flow_folded.contains(&row.step);
            visible.push(FlowLine {
                row: row.clone(),
                has_branches,
                folded,
            });
            if folded {
                hidden_below = Some(row.depth);
            }
        }
        visible
    }

    /// The steps from the start of the flow down to the selected row, selected one last.
    ///
    /// Computed on the whole layout, so folding never removes an ancestor from the path.
    pub fn flow_breadcrumbs(&self) -> Vec<usize> {
        let Some(flow) = &self.session.flow().flow else {
            return Vec::new();
        };
        let Some(selected) = self
            .flow_rows()
            .get(self.flow_view.selected)
            .map(|line| line.row.clone())
        else {
            return Vec::new();
        };
        let all = crate::app::flow_layout(&flow.document);
        let Some(position) = all.iter().position(|row| *row == selected) else {
            return vec![selected.step];
        };
        let mut path = vec![selected.step];
        let mut depth = selected.depth;
        for row in all[..position].iter().rev() {
            if row.depth < depth {
                path.push(row.step);
                depth = row.depth;
            }
        }
        path.reverse();
        path
    }

    // ---- structure ---------------------------------------------------------------------------

    /// The changed files, most urgent first, with how much of each is reviewed.
    pub fn files_rows(&self) -> Vec<FileRow> {
        let mut by_file: std::collections::BTreeMap<String, FileRow> =
            std::collections::BTreeMap::new();
        for item in self.visible_queue() {
            let reviewed = self.session.review().is_reviewed(&item.unit.id);
            let path = item.unit.file.as_str().to_owned();
            let row = by_file.entry(path.clone()).or_insert_with(|| FileRow {
                path,
                priority: item.priority,
                progress: (0, 0),
                first: item.unit.id.clone(),
            });
            row.progress.1 += 1;
            row.progress.0 += usize::from(reviewed);
            // The queue is ordered by urgency, so the first unit seen is the most urgent one.
        }
        let mut rows: Vec<FileRow> = by_file.into_values().collect();
        rows.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.path.cmp(&b.path))
        });
        rows
    }

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

    /// Shows `unit` in the code pane without moving the focus to it.
    pub fn preview_unit(&mut self, unit: &UnitId) {
        self.code = CodeState {
            version: Some(self.default_version(unit)),
            scope: Some(CodeScope::Symbol),
            ..CodeState::default()
        };
        self.rebuild_code();
    }

    /// The version a unit is first read in: the new code, or the old one when it was removed.
    fn default_version(&self, unit: &UnitId) -> Version {
        match self.session.analysis().change_of(unit) {
            Some(change) if change.after.is_none() => Version::Before,
            _ => match self.session.analysis().file_of(unit) {
                Some(file)
                    if file.analysis.status == crate::domain::snapshot::ChangeStatus::Deleted =>
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
    pub fn followups(&self) -> Vec<&crate::domain::review::Comment> {
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
        let row = |kind: ImpactRowKind, text: String| ImpactRow {
            kind,
            text,
            target: None,
            origin: None,
            detail: String::new(),
            extra: String::new(),
            depth: 0,
        };
        let note = |text: &str| row(ImpactRowKind::Note, text.to_owned());
        let heading = |text: &str, count: usize| ImpactRow {
            detail: count.to_string(),
            ..row(ImpactRowKind::Heading, text.to_owned())
        };
        let locale = self.locale;
        self.impact.summary.clear();
        self.impact.evidence.clear();
        let Some(subject) = self.impact_subject() else {
            rows.push(note(t::impact_whole_file(locale)));
            self.impact.report = None;
            self.impact.rows = rows;
            self.impact_view = Viewport::default();
            return;
        };
        let report = self.session.impact(&subject);
        match &report {
            None => rows.push(note(t::impact_not_indexed(locale))),
            Some(report) => {
                self.impact.summary = t::impact_summary(
                    locale,
                    labels::reach(locale, report.scope),
                    report.direct_callers(),
                    report.entrypoints.len(),
                    report.external_entrypoints(),
                );
                let c = &report.coverage;
                let tail = if c.truncated {
                    t::impact_depth_limit(locale)
                } else {
                    ""
                };
                self.impact.evidence = t::impact_evidence(
                    locale,
                    c.resolved_edges,
                    c.heuristic_edges,
                    c.unresolved_calls,
                    tail,
                );
                rows.push(heading(
                    t::impact_entrypoints(locale),
                    report.entrypoints.len(),
                ));
                if report.entrypoints.is_empty() {
                    rows.push(note(t::impact_no_entrypoints(locale)));
                }
                for reach in &report.entrypoints {
                    let entry = &reach.entrypoint;
                    let path: Vec<String> = reach
                        .path
                        .iter()
                        .map(|step| sanitize_single_line(&step.symbol.qualified_name))
                        .collect();
                    rows.push(ImpactRow {
                        target: Some(entry.symbol.clone()),
                        origin: Some(reach.origin),
                        detail: sanitize_single_line(&format!(
                            "{} · {}, {} · {}",
                            entry.symbol.file,
                            labels::entrypoint_kind(locale, &entry.kind),
                            labels::entrypoint_scope(locale, entry.scope),
                            entry.evidence,
                        )),
                        extra: path.join(" → "),
                        ..row(
                            ImpactRowKind::Entrypoint,
                            sanitize_single_line(&entry.symbol.qualified_name),
                        )
                    });
                }
                let callers: usize = report.callers.iter().map(|l| l.callers.len()).sum();
                rows.push(heading(t::impact_callers(locale), callers));
                if report.callers.is_empty() {
                    rows.push(note(t::impact_no_callers(locale)));
                }
                for level in &report.callers {
                    for step in &level.callers {
                        rows.push(ImpactRow {
                            target: Some(step.symbol.clone()),
                            origin: Some(step.origin),
                            detail: sanitize_single_line(step.symbol.file.as_ref()),
                            depth: usize::try_from(level.depth.saturating_sub(1)).unwrap_or(0),
                            ..row(
                                ImpactRowKind::Caller,
                                sanitize_single_line(&step.symbol.qualified_name),
                            )
                        });
                    }
                }
                rows.push(heading(t::impact_calls(locale), report.callees.len()));
                if report.callees.is_empty() {
                    rows.push(note(t::impact_no_calls(locale)));
                }
                for step in &report.callees {
                    rows.push(ImpactRow {
                        target: Some(step.symbol.clone()),
                        origin: Some(step.origin),
                        detail: sanitize_single_line(step.symbol.file.as_ref()),
                        ..row(
                            ImpactRowKind::Callee,
                            sanitize_single_line(&step.symbol.qualified_name),
                        )
                    });
                }
            }
        }
        self.impact.report = report;
        self.impact.rows = rows;
        // Start on the first thing that can be opened, not on a heading.
        self.impact_view = Viewport {
            selected: self
                .impact
                .rows
                .iter()
                .position(|r| r.target.is_some())
                .unwrap_or(0),
            ..Viewport::default()
        };
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
                t::impact_cannot_show(self.locale, &sanitize_single_line(&key.qualified_name)),
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
