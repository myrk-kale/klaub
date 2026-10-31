//! Things the reader can do. Every key and every palette entry maps to an [`Action`], so the two
//! cannot drift apart and the help screen is generated from the same table.

use klaub_core::finding::Priority;
use klaub_core::graph::ImpactScope;
use klaub_core::review::{FollowupStatus, Version};
use klaub_core::text::sanitize_single_line;

use crate::app::{
    App, Confirm, ConfirmAction, Depth, Editor, EditorPurpose, FinishStage, Mode, Overlay,
    StatusKind,
};

/// A user-level operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// Switch to a mode.
    Mode(Mode),
    /// Jump to the next unit not marked reviewed.
    NextUnreviewed,
    /// Jump to the previous unit not marked reviewed.
    PrevUnreviewed,
    /// Jump to the next P0/P1 unit not marked reviewed.
    NextCritical,
    /// Mark or unmark the current unit as reviewed.
    ToggleReviewed,
    /// Mark or unmark the current unit for a quick return. Kept only while the reader is open.
    ToggleBookmark,
    /// Jump to the next bookmarked unit.
    NextBookmark,
    /// Comment on the selection.
    Comment,
    /// Start finishing the review.
    Finish,
    /// Take a new snapshot.
    Refresh,
    /// Open the search field.
    Search,
    /// Show the key reference.
    Help,
    /// Switch between the new and the old code.
    ToggleVersion,
    /// Look at the old code for as long as the key is held.
    PeekPrevious,
    /// Go to the next change region.
    NextChange,
    /// Go to the previous change region.
    PrevChange,
    /// Show why the unit has its priority.
    Explain,
    /// Show what changed about the current symbol's contract.
    InspectContract,
    /// Bring back the rules this review silenced.
    RestoreRules,
    /// Open the current unit in the whole file.
    OpenWholeFile,
    /// Go back to the unit visited before this one.
    HistoryBack,
    /// Go forward again after going back.
    HistoryForward,
    /// Run the associated tests (after confirmation).
    RunTests,
    /// Re-read test reports.
    ReloadReports,
    /// Re-read the flow file.
    ReloadFlow,
    /// Ask the agent for a flow diagram (after confirmation).
    RequestFlow,
    /// Remove every filter.
    ClearFilters,
    /// Toggle "P0/P1 only".
    FilterHigh,
    /// Toggle "introduced by this change only".
    FilterIntroduced,
    /// Toggle "without associated tests only".
    FilterNoTests,
    /// Toggle "not yet reviewed only".
    FilterUnreviewed,
    /// Toggle "new files only".
    FilterNew,
    /// Cycle the minimum structural reach.
    CycleScope,
    /// Cycle the module filter.
    CycleModule,
    /// Switch the code between the symbol and the whole file.
    ToggleCodeScope,
    /// Show more or less of the structure tree.
    DeeperTree,
    /// Show less of the structure tree.
    ShallowerTree,
    /// The reviewer submitted the feedback in the agent.
    ConfirmSubmitted,
    /// Copy the feedback instruction.
    CopyFeedback,
    /// Leave the reader.
    Quit,
}

/// One line of the palette and the help.
pub(crate) struct Entry {
    /// The action.
    pub action: Action,
    /// What the palette shows.
    pub name: &'static str,
    /// The default key, for display.
    pub key: &'static str,
}

/// Every action with its key. The palette and the help screen are built from this table.
pub(crate) const ENTRIES: &[Entry] = &[
    Entry {
        action: Action::Mode(Mode::Queue),
        name: "Go to Queue",
        key: "1  F2",
    },
    Entry {
        action: Action::Mode(Mode::Structure),
        name: "Go to Structure",
        key: "2  F3",
    },
    Entry {
        action: Action::Mode(Mode::Code),
        name: "Go to Code",
        key: "3  F4",
    },
    Entry {
        action: Action::Mode(Mode::Impact),
        name: "Go to Impact (callers, entrypoints)",
        key: "4  g  F5",
    },
    Entry {
        action: Action::Mode(Mode::Comments),
        name: "Go to Comments",
        key: "5  F6",
    },
    Entry {
        action: Action::Mode(Mode::Context),
        name: "Go to Context (brief, criteria)",
        key: "6  b  F8",
    },
    Entry {
        action: Action::Mode(Mode::Tests),
        name: "Go to Tests",
        key: "7  t  F7",
    },
    Entry {
        action: Action::Mode(Mode::Followup),
        name: "Go to Follow-up",
        key: "8  F10",
    },
    Entry {
        action: Action::Mode(Mode::Coverage),
        name: "Go to Coverage",
        key: "9",
    },
    Entry {
        action: Action::Mode(Mode::Flow),
        name: "Go to Flow",
        key: "0  f  F9",
    },
    Entry {
        action: Action::NextUnreviewed,
        name: "Next unreviewed",
        key: "n",
    },
    Entry {
        action: Action::PrevUnreviewed,
        name: "Previous unreviewed",
        key: "P",
    },
    Entry {
        action: Action::NextCritical,
        name: "Next unreviewed P0/P1",
        key: "N",
    },
    Entry {
        action: Action::ToggleReviewed,
        name: "Mark reviewed / unmark",
        key: "r",
    },
    Entry {
        action: Action::ToggleBookmark,
        name: "Bookmark this unit",
        key: "m",
    },
    Entry {
        action: Action::NextBookmark,
        name: "Next bookmark",
        key: ",",
    },
    Entry {
        action: Action::Comment,
        name: "Comment on selection",
        key: "c",
    },
    Entry {
        action: Action::Explain,
        name: "Why this priority?",
        key: "w",
    },
    Entry {
        action: Action::RestoreRules,
        name: "Bring back silenced rules",
        key: "",
    },
    Entry {
        action: Action::InspectContract,
        name: "Inspect the contract",
        key: "i",
    },
    Entry {
        action: Action::OpenWholeFile,
        name: "Open the unit in the whole file",
        key: "o",
    },
    Entry {
        action: Action::HistoryBack,
        name: "Back to the previous unit",
        key: "<",
    },
    Entry {
        action: Action::HistoryForward,
        name: "Forward to the next unit in the history",
        key: ">",
    },
    Entry {
        action: Action::ToggleVersion,
        name: "Toggle before / after",
        key: "v",
    },
    Entry {
        action: Action::PeekPrevious,
        name: "Hold to see the previous version",
        key: "B",
    },
    Entry {
        action: Action::NextChange,
        name: "Next change",
        key: "]",
    },
    Entry {
        action: Action::PrevChange,
        name: "Previous change",
        key: "[",
    },
    Entry {
        action: Action::ToggleCodeScope,
        name: "Code: symbol / whole file",
        key: "S",
    },
    Entry {
        action: Action::Search,
        name: "Search",
        key: "/",
    },
    Entry {
        action: Action::FilterHigh,
        name: "Filter: P0/P1 only",
        key: "H",
    },
    Entry {
        action: Action::FilterUnreviewed,
        name: "Filter: unreviewed only",
        key: "u",
    },
    Entry {
        action: Action::FilterIntroduced,
        name: "Filter: introduced by this change",
        key: "I",
    },
    Entry {
        action: Action::FilterNoTests,
        name: "Filter: no associated tests",
        key: "T",
    },
    Entry {
        action: Action::FilterNew,
        name: "Filter: new files",
        key: "W",
    },
    Entry {
        action: Action::CycleScope,
        name: "Filter: minimum reach (local/module/global)",
        key: "s",
    },
    Entry {
        action: Action::CycleModule,
        name: "Filter: next module",
        key: "M",
    },
    Entry {
        action: Action::ClearFilters,
        name: "Clear filters",
        key: "x",
    },
    Entry {
        action: Action::DeeperTree,
        name: "Structure: show more",
        key: "+",
    },
    Entry {
        action: Action::ShallowerTree,
        name: "Structure: show less",
        key: "-",
    },
    Entry {
        action: Action::RunTests,
        name: "Run associated tests (asks first)",
        key: "X",
    },
    Entry {
        action: Action::ReloadReports,
        name: "Reload test reports",
        key: "U",
    },
    Entry {
        action: Action::ReloadFlow,
        name: "Reload flow",
        key: "L",
    },
    Entry {
        action: Action::RequestFlow,
        name: "Prepare flow request in agent",
        key: "Flow: p",
    },
    Entry {
        action: Action::Refresh,
        name: "Refresh snapshot",
        key: "R",
    },
    Entry {
        action: Action::Finish,
        name: "Finish review…",
        key: "F",
    },
    Entry {
        action: Action::CopyFeedback,
        name: "Copy feedback instruction",
        key: "",
    },
    Entry {
        action: Action::ConfirmSubmitted,
        name: "I submitted the feedback in the agent",
        key: "",
    },
    Entry {
        action: Action::Help,
        name: "Help",
        key: "?  F1",
    },
    Entry {
        action: Action::Quit,
        name: "Close the reader",
        key: "q q",
    },
];

/// Applies an action to the reader.
pub(crate) fn perform(app: &mut App, action: Action) {
    match action {
        Action::Mode(mode) => switch_mode(app, mode),
        Action::NextUnreviewed => jump_unreviewed(app, true, false),
        Action::PrevUnreviewed => jump_unreviewed(app, false, false),
        Action::NextCritical => jump_unreviewed(app, true, true),
        Action::ToggleReviewed => toggle_reviewed(app),
        Action::ToggleBookmark => toggle_bookmark(app),
        Action::NextBookmark => next_bookmark(app),
        Action::Comment => start_comment(app),
        Action::Finish => start_finish(app),
        Action::Refresh => ask_refresh(app),
        Action::Search => {
            app.overlay = Some(Overlay::Search {
                input: app.filters.search.clone(),
            });
        }
        Action::Help => app.overlay = Some(Overlay::Help),
        Action::ToggleVersion => toggle_version(app),
        // Holding needs the key itself, which a palette entry does not have, so the palette only
        // says how it works.
        Action::PeekPrevious => app.say(
            StatusKind::Info,
            format!(
                "Hold {} while reading code to see the previous version; {} toggles it.",
                app.keymap.key_for("peek_previous"),
                app.keymap.key_for("toggle_version"),
            ),
        ),
        Action::NextChange => jump_change(app, true),
        Action::PrevChange => jump_change(app, false),
        Action::Explain => match app.unit.clone() {
            Some(unit) => app.overlay = Some(Overlay::Explain(unit)),
            None => app.say(StatusKind::Info, "Select a unit first."),
        },
        Action::InspectContract => match app.unit.clone() {
            Some(unit) => app.overlay = Some(Overlay::Contract(unit)),
            None => app.say(StatusKind::Info, "Select a unit first."),
        },
        Action::RestoreRules => {
            let muted = app.session.muted_rules().to_vec();
            if muted.is_empty() {
                app.say(StatusKind::Info, "No rule is silenced in this review.");
            } else {
                app.overlay = Some(Overlay::Confirm(Confirm {
                    title: "Bring back the silenced rules?".to_owned(),
                    lines: vec![
                        format!("silenced here: {}", muted.join(", ")),
                        "Their findings return and the queue is recomputed on this snapshot."
                            .to_owned(),
                    ],
                    action: ConfirmAction::RestoreRules,
                }));
            }
        }
        Action::OpenWholeFile => open_whole_file(app),
        Action::HistoryBack => {
            if !app.history_back() {
                app.say(StatusKind::Info, "No earlier unit in the history.");
            }
        }
        Action::HistoryForward => {
            if !app.history_forward() {
                app.say(StatusKind::Info, "No later unit in the history.");
            }
        }
        Action::RunTests => ask_run_tests(app),
        Action::ReloadReports => {
            app.session.reload_reports();
            let n = app.session.reports().sources.len();
            app.say(
                StatusKind::Info,
                format!("Test reports reloaded ({n} source(s))."),
            );
        }
        Action::ReloadFlow => reload_flow(app),
        Action::RequestFlow => ask_flow_request(app),
        Action::ClearFilters => {
            app.filters = crate::app::Filters::default();
            refresh_queue_selection(app);
            app.say(StatusKind::Info, "Filters cleared.");
        }
        Action::FilterHigh => {
            app.filters.min_priority = if app.filters.min_priority.is_some() {
                None
            } else {
                Some(Priority::P1)
            };
            refresh_queue_selection(app);
        }
        Action::FilterIntroduced => {
            app.filters.introduced_only = !app.filters.introduced_only;
            refresh_queue_selection(app);
        }
        Action::FilterNoTests => {
            app.filters.no_tests_only = !app.filters.no_tests_only;
            refresh_queue_selection(app);
        }
        Action::FilterUnreviewed => {
            app.filters.unreviewed_only = !app.filters.unreviewed_only;
            refresh_queue_selection(app);
        }
        Action::FilterNew => {
            app.filters.new_only = !app.filters.new_only;
            refresh_queue_selection(app);
        }
        Action::CycleScope => {
            app.filters.min_scope = match app.filters.min_scope {
                None => Some(ImpactScope::Module),
                Some(ImpactScope::Module) => Some(ImpactScope::CrossModule),
                Some(ImpactScope::CrossModule) => Some(ImpactScope::Global),
                Some(_) => None,
            };
            refresh_queue_selection(app);
        }
        Action::CycleModule => cycle_module(app),
        Action::ToggleCodeScope => toggle_code_scope(app),
        Action::DeeperTree => {
            app.depth = match app.depth {
                Depth::Module => Depth::File,
                _ => Depth::Symbol,
            };
            app.collapsed.clear();
        }
        Action::ShallowerTree => {
            app.depth = match app.depth {
                Depth::Symbol => Depth::File,
                _ => Depth::Module,
            };
        }
        Action::ConfirmSubmitted => match app.session.confirm_submitted() {
            Ok(()) => app.say(StatusKind::Info, "Recorded: you submitted the feedback."),
            Err(error) => app.say(StatusKind::Error, error.to_string()),
        },
        Action::CopyFeedback => copy_feedback(app),
        Action::Quit => app.should_quit = true,
    }
}

fn refresh_queue_selection(app: &mut App) {
    let len = app.visible_queue().len();
    app.queue_view.clamp(len);
    app.structure_view.clamp(app.tree_rows().len());
    if app.filters.any() {
        app.say(
            StatusKind::Info,
            format!("Filter: {} — {len} unit(s).", app.filters.summary()),
        );
    }
    if let Some(unit) = app.queue_selection() {
        app.adopt(unit);
    }
}

fn cycle_module(app: &mut App) {
    let modules = app.modules();
    app.filters.module = match &app.filters.module {
        None => modules.first().cloned(),
        Some(current) => modules
            .iter()
            .position(|m| m == current)
            .and_then(|i| modules.get(i + 1))
            .cloned(),
    };
    refresh_queue_selection(app);
}

fn switch_mode(app: &mut App, mode: Mode) {
    if matches!(mode, Mode::Code) && app.code.view.is_none() {
        match app.unit.clone().or_else(|| app.queue_selection()) {
            Some(unit) => app.open_unit(&unit, None),
            None => app.say(StatusKind::Info, "Nothing to show: the queue is empty."),
        }
        return;
    }
    if matches!(mode, Mode::Impact | Mode::Tests)
        && app.unit.is_none()
        && let Some(unit) = app.queue_selection()
    {
        app.adopt(unit);
    }
    app.go(mode);
}

/// Makes `unit` the current one; the code view follows when it is on screen.
pub(crate) fn focus_unit(app: &mut App, unit: &klaub_core::ids::UnitId) {
    app.adopt(unit.clone());
    app.select_in_queue(unit);
    match app.mode {
        Mode::Code => app.open_unit(unit, None),
        Mode::Impact | Mode::Tests => {
            app.impact.focus = None;
            if app.mode == Mode::Impact {
                app.rebuild_impact();
            }
        }
        _ => {}
    }
}

fn jump_unreviewed(app: &mut App, forward: bool, critical: bool) {
    let from = app.unit.clone().or_else(|| app.queue_selection());
    match app.next_unreviewed(from.as_ref(), forward, critical) {
        Some(unit) if Some(&unit) != from.as_ref() || !app.session.review().is_reviewed(&unit) => {
            focus_unit(app, &unit);
        }
        _ => {
            let what = if critical { "P0/P1 unit" } else { "unit" };
            app.say(
                StatusKind::Info,
                format!("No unreviewed {what} left in view."),
            );
        }
    }
}

fn toggle_reviewed(app: &mut App) {
    let Some(unit) = app.unit.clone().or_else(|| app.queue_selection()) else {
        app.say(StatusKind::Info, "Select a unit first.");
        return;
    };
    match app.session.toggle_reviewed(&unit) {
        Ok(true) => {
            let c = app.session.coverage();
            app.say(
                StatusKind::Info,
                format!(
                    "Marked reviewed ({}/{}). Press r again to undo, n for the next.",
                    c.reviewed, c.total
                ),
            );
        }
        Ok(false) => app.say(StatusKind::Info, "Mark removed."),
        Err(error) => app.say(StatusKind::Error, error.to_string()),
    }
    let len = app.visible_queue().len();
    app.queue_view.clamp(len);
}

fn toggle_bookmark(app: &mut App) {
    let Some(unit) = app.unit.clone().or_else(|| app.queue_selection()) else {
        app.say(StatusKind::Info, "Select a unit first.");
        return;
    };
    if let Some(position) = app.bookmarks.iter().position(|marked| marked == &unit) {
        app.bookmarks.remove(position);
        app.say(StatusKind::Info, "Bookmark removed.");
    } else {
        app.bookmarks.push(unit);
        app.say(
            StatusKind::Info,
            format!("Bookmarked ({}). , jumps to the next.", app.bookmarks.len()),
        );
    }
}

fn next_bookmark(app: &mut App) {
    if app.bookmarks.is_empty() {
        app.say(
            StatusKind::Info,
            "No bookmarks yet. m marks the current unit.",
        );
        return;
    }
    let from = app.unit.clone().or_else(|| app.queue_selection());
    let start = from
        .as_ref()
        .and_then(|unit| app.bookmarks.iter().position(|marked| marked == unit))
        .map_or(0, |position| (position + 1) % app.bookmarks.len());
    let target = (0..app.bookmarks.len())
        .map(|step| &app.bookmarks[(start + step) % app.bookmarks.len()])
        .find(|unit| app.session.analysis().item(unit).is_some())
        .cloned();
    match target {
        Some(unit) if Some(&unit) != from.as_ref() => focus_unit(app, &unit),
        Some(_) => app.say(StatusKind::Info, "Already on the only bookmark."),
        None => app.say(StatusKind::Info, "No bookmark is still in the queue."),
    }
}

fn start_comment(app: &mut App) {
    let Some(unit) = app.unit.clone() else {
        app.say(StatusKind::Info, "Select a unit first.");
        return;
    };
    if app.mode == Mode::Code && app.code.detached.is_some() {
        app.say(
            StatusKind::Warn,
            "This code is not part of the change; comments attach to changed code only.",
        );
        return;
    }
    let version = app.code.version.unwrap_or(Version::After);
    let lines = (app.mode == Mode::Code)
        .then(|| selected_lines(app))
        .flatten();
    let label = app
        .current_item()
        .map_or_else(String::new, |i| sanitize_single_line(&i.unit.label));
    let place = lines.map_or_else(
        || "whole unit".to_owned(),
        |(a, b)| {
            if a == b {
                format!("line {a}")
            } else {
                format!("lines {a}–{b}")
            }
        },
    );
    let side = if version == Version::Before {
        "old"
    } else {
        "new"
    };
    app.overlay = Some(Overlay::Editor(Editor {
        title: format!("Comment on {label} ({side} code, {place})"),
        purpose: EditorPurpose::NewComment {
            unit,
            version,
            lines,
        },
        text: String::new(),
        severity: None,
        multiline: true,
        error: None,
    }));
}

/// The line numbers (in the version shown) covered by the selection or the cursor.
pub(crate) fn selected_lines(app: &App) -> Option<(u32, u32)> {
    let view = app.code.view.as_ref()?;
    let at = |i: usize| view.lines.get(i).map(|l| l.number);
    let cursor = at(app.code.cursor)?;
    let anchor = app.code.selection.and_then(at).unwrap_or(cursor);
    Some((cursor.min(anchor), cursor.max(anchor)))
}

fn start_finish(app: &mut App) {
    let warnings = app.session.warnings_before_finish();
    app.overlay = Some(Overlay::Finish(FinishStage::Confirm { warnings }));
}

fn ask_refresh(app: &mut App) {
    app.overlay = Some(Overlay::Confirm(Confirm {
        title: "Take a new snapshot?".to_owned(),
        lines: vec![
            "The changes are analysed again from the current worktree.".to_owned(),
            "Comments are moved to where their code went; uncertain ones become stale.".to_owned(),
            "Units marked reviewed stay marked only if their code did not change.".to_owned(),
        ],
        action: ConfirmAction::Refresh,
    }));
}

pub(crate) fn toggle_version(app: &mut App) {
    let Some(view) = &app.code.view else {
        app.say(StatusKind::Info, "Open some code first.");
        return;
    };
    if app.code.detached.is_some() {
        app.say(
            StatusKind::Info,
            "This code is not part of the change; there is no other version.",
        );
        return;
    }
    if !view.other_version_exists {
        let why = match app.code.version {
            Some(Version::Before) => "this code was removed; there is no new version",
            _ => "NEW · there is no previous version to compare with",
        };
        app.say(StatusKind::Info, why);
        return;
    }
    app.code.version = Some(match app.code.version {
        Some(Version::Before) => Version::After,
        _ => Version::Before,
    });
    app.rebuild_code();
}

/// `o`: the current unit, read inside its whole file. Unlike `S` it does not toggle: pressing it
/// twice stays in the file.
fn open_whole_file(app: &mut App) {
    let Some(unit) = app.unit.clone() else {
        app.say(StatusKind::Info, "Select a unit first.");
        return;
    };
    if app.mode != Mode::Code || app.code.view.is_none() {
        app.open_unit(&unit, None);
    }
    if app.code.view.is_none() || app.code.detached.is_some() {
        return;
    }
    app.code.scope = Some(klaub_app::CodeScope::File);
    app.rebuild_code();
}

fn toggle_code_scope(app: &mut App) {
    if app.code.view.is_none() || app.code.detached.is_some() {
        return;
    }
    app.code.scope = Some(match app.code.scope {
        Some(klaub_app::CodeScope::File) => klaub_app::CodeScope::Symbol,
        _ => klaub_app::CodeScope::File,
    });
    app.rebuild_code();
}

fn jump_change(app: &mut App, forward: bool) {
    let Some(view) = &app.code.view else { return };
    let Some(current) = view.lines.get(app.code.cursor).map(|l| l.number) else {
        return;
    };
    let target = if forward {
        view.changes.iter().copied().find(|n| *n > current)
    } else {
        view.changes.iter().copied().rev().find(|n| *n < current)
    };
    match target.and_then(|n| view.lines.iter().position(|l| l.number == n)) {
        Some(index) => {
            app.code.cursor = index;
            app.code.selection = None;
            ensure_code_visible(app);
        }
        None => app.say(
            StatusKind::Info,
            if forward {
                "No further change in this code. n goes to the next unreviewed unit."
            } else {
                "No earlier change in this code."
            },
        ),
    }
}

/// Scrolls the code so that the cursor is on screen.
pub(crate) fn ensure_code_visible(app: &mut App) {
    let height = if app.code.height == 0 {
        20
    } else {
        app.code.height
    };
    if app.code.cursor < app.code.offset {
        app.code.offset = app.code.cursor;
    } else if app.code.cursor >= app.code.offset + height {
        app.code.offset = app.code.cursor + 1 - height;
    }
}

fn ask_run_tests(app: &mut App) {
    let Some(unit) = app.unit.clone() else {
        app.say(StatusKind::Info, "Select a unit first.");
        return;
    };
    match app.session.plan_test_run(&unit) {
        Ok(request) => {
            let preview = klaub_app::ReviewSession::preview_test_run(&request);
            app.overlay = Some(Overlay::Confirm(Confirm {
                title: "Run these tests?".to_owned(),
                lines: vec![
                    format!("command: {preview}"),
                    format!("in:      {}", request.cwd.display()),
                    format!("limit:   {} s", request.timeout.as_secs()),
                    "The command comes from your .klaub.toml; Klaub never invents one.".to_owned(),
                    "The screen is busy until it finishes or times out.".to_owned(),
                ],
                action: ConfirmAction::RunTests(request),
            }));
        }
        Err(error) => app.say(StatusKind::Warn, error.to_string()),
    }
}

fn reload_flow(app: &mut App) {
    match app.session.reload_flow() {
        Ok(()) => {
            let label = app.session.flow().state.label();
            app.say(StatusKind::Info, format!("Flow reloaded: {label}."));
        }
        Err(error) => app.say(StatusKind::Error, error.to_string()),
    }
}

fn ask_flow_request(app: &mut App) {
    let prompt = app.session.flow().request_prompt.clone();
    app.overlay = Some(Overlay::Confirm(prepare_confirm(
        app,
        "Prepare the diagram request in the agent?",
        &prompt,
        ConfirmAction::PrepareFlowRequest,
    )));
}

/// The confirmation shown before anything is pasted into the agent.
/// Whether a process name is an interactive shell. A login shell is shown with a leading `-`.
fn is_shell(command: &str) -> bool {
    const SHELLS: &[&str] = &[
        "sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "csh", "nu", "pwsh", "elvish", "xonsh",
    ];
    let name = command.rsplit('/').next().unwrap_or(command);
    SHELLS.contains(&name.trim_start_matches('-'))
}

pub(crate) fn prepare_confirm(
    app: &App,
    title: &str,
    text: &str,
    action: ConfirmAction,
) -> Confirm {
    let mut foreground = None;
    let destination = match &app.agent {
        None => "none — Klaub is not managing an agent here; use Copy instead".to_owned(),
        Some(channel) => match channel.target() {
            Ok(Some(target)) => {
                foreground = Some(sanitize_single_line(&target.command));
                sanitize_single_line(&target.description)
            }
            Ok(None) => "none — no agent pane was found; use Copy instead".to_owned(),
            Err(error) => format!("unverified — {error}"),
        },
    };
    let mut lines = vec![format!("destination: {destination}")];
    if let Some(command) = &foreground {
        lines.push(format!("running there: {command}"));
        if is_shell(command) {
            lines.push(format!(
                "⚠ {command} is a shell. If your agent is not the one reading the keyboard, the text lands on a shell prompt."
            ));
        }
    }
    lines.extend([
        "text to paste (Klaub never presses Enter; you decide when to send it):".to_owned(),
        format!("  {}", sanitize_single_line(text)),
        "If the agent's input already has text, the paste may be appended to it.".to_owned(),
    ]);
    Confirm {
        title: title.to_owned(),
        lines,
        action,
    }
}

fn copy_feedback(app: &mut App) {
    let Some(clipboard) = app.clipboard.clone() else {
        app.say(
            StatusKind::Warn,
            "No clipboard is available here. Use Show file instead.",
        );
        return;
    };
    match app.session.copy_feedback(clipboard.as_ref()) {
        Ok(()) => app.say(
            StatusKind::Info,
            "Feedback instruction copied. Paste it into the agent yourself.",
        ),
        Err(error) => app.say(StatusKind::Error, error.to_string()),
    }
}

/// The follow-up decision a key stands for.
pub(crate) fn followup_decision(key: char) -> Option<FollowupStatus> {
    match key {
        'y' => Some(FollowupStatus::Resolved),
        'p' => Some(FollowupStatus::Partial),
        'u' => Some(FollowupStatus::Unresolved),
        'X' => Some(FollowupStatus::Regression),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::is_shell;

    #[test]
    fn shells_are_recognised_by_name_even_as_login_shells_or_paths() {
        for shell in ["bash", "-zsh", "/bin/sh", "fish", "pwsh"] {
            assert!(is_shell(shell), "{shell}");
        }
        for other in ["codex", "node", "claude", "python3", "bashful", ""] {
            assert!(!is_shell(other), "{other}");
        }
    }
}
