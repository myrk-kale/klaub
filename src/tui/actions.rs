//! Things the reader can do. Every key and every palette entry maps to an [`Action`], so the two
//! cannot drift apart and the help screen is generated from the same table.

use crate::domain::finding::Priority;
use crate::domain::graph::ImpactScope;
use crate::domain::i18n::{Locale, tui as t};
use crate::domain::review::{FollowupStatus, Version};
use crate::domain::text::sanitize_single_line;

use crate::tui::app::{
    App, Confirm, ConfirmAction, Depth, Editor, EditorPurpose, FinishStage, Mode, Overlay,
    StatusKind,
};
use crate::tui::labels;

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
    /// Show or hide the panel with the symbol's signature, documentation and comments.
    ToggleInspector,
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
    /// Switch to the next colour theme for as long as the reader is open.
    NextTheme,
    /// Leave the reader.
    Quit,
}

/// One line of the palette and the help.
pub(crate) struct Entry {
    /// The action.
    pub action: Action,
    /// The default key, for display.
    pub key: &'static str,
}

impl Entry {
    /// What the palette and the help show for this action.
    pub(crate) fn name(&self, locale: Locale) -> &'static str {
        match self.action {
            Action::Mode(Mode::Queue) => t::action_go_queue(locale),
            Action::Mode(Mode::Structure) => t::action_go_structure(locale),
            Action::Mode(Mode::Files) => t::action_go_files(locale),
            Action::Mode(Mode::Code) => t::action_go_code(locale),
            Action::Mode(Mode::Impact) => t::action_go_impact(locale),
            Action::Mode(Mode::Comments) => t::action_go_comments(locale),
            Action::Mode(Mode::Context) => t::action_go_context(locale),
            Action::Mode(Mode::Tests) => t::action_go_tests(locale),
            Action::Mode(Mode::Followup) => t::action_go_followup(locale),
            Action::Mode(Mode::Coverage) => t::action_go_coverage(locale),
            Action::Mode(Mode::Flow) => t::action_go_flow(locale),
            Action::NextUnreviewed => t::action_next_unreviewed(locale),
            Action::PrevUnreviewed => t::action_prev_unreviewed(locale),
            Action::NextCritical => t::action_next_critical(locale),
            Action::ToggleReviewed => t::action_toggle_reviewed(locale),
            Action::ToggleBookmark => t::action_toggle_bookmark(locale),
            Action::NextBookmark => t::action_next_bookmark(locale),
            Action::Comment => t::action_comment(locale),
            Action::Explain => t::action_explain(locale),
            Action::RestoreRules => t::action_restore_rules(locale),
            Action::InspectContract => t::action_inspect_contract(locale),
            Action::ToggleInspector => t::action_toggle_inspector(locale),
            Action::OpenWholeFile => t::action_open_whole_file(locale),
            Action::HistoryBack => t::action_history_back(locale),
            Action::HistoryForward => t::action_history_forward(locale),
            Action::ToggleVersion => t::action_toggle_version(locale),
            Action::PeekPrevious => t::action_peek_previous(locale),
            Action::NextChange => t::action_next_change(locale),
            Action::PrevChange => t::action_prev_change(locale),
            Action::ToggleCodeScope => t::action_toggle_code_scope(locale),
            Action::Search => t::action_search(locale),
            Action::FilterHigh => t::action_filter_high(locale),
            Action::FilterUnreviewed => t::action_filter_unreviewed(locale),
            Action::FilterIntroduced => t::action_filter_introduced(locale),
            Action::FilterNoTests => t::action_filter_no_tests(locale),
            Action::FilterNew => t::action_filter_new(locale),
            Action::CycleScope => t::action_cycle_scope(locale),
            Action::CycleModule => t::action_cycle_module(locale),
            Action::ClearFilters => t::action_clear_filters(locale),
            Action::DeeperTree => t::action_deeper_tree(locale),
            Action::ShallowerTree => t::action_shallower_tree(locale),
            Action::RunTests => t::action_run_tests(locale),
            Action::ReloadReports => t::action_reload_reports(locale),
            Action::ReloadFlow => t::action_reload_flow(locale),
            Action::RequestFlow => t::action_request_flow(locale),
            Action::Refresh => t::action_refresh(locale),
            Action::Finish => t::action_finish(locale),
            Action::CopyFeedback => t::action_copy_feedback(locale),
            Action::ConfirmSubmitted => t::action_confirm_submitted(locale),
            Action::NextTheme => t::action_next_theme(locale),
            Action::Help => t::action_help(locale),
            Action::Quit => t::action_quit(locale),
        }
    }
}

/// Every action with its key. The palette and the help screen are built from this table.
pub(crate) const ENTRIES: &[Entry] = &[
    Entry {
        action: Action::Mode(Mode::Queue),
        key: "1  F2",
    },
    Entry {
        action: Action::Mode(Mode::Structure),
        key: "2  F3",
    },
    Entry {
        action: Action::Mode(Mode::Files),
        key: "3",
    },
    Entry {
        action: Action::Mode(Mode::Code),
        key: "F4",
    },
    Entry {
        action: Action::Mode(Mode::Impact),
        key: "4  g  F5",
    },
    Entry {
        action: Action::Mode(Mode::Comments),
        key: "5  F6",
    },
    Entry {
        action: Action::Mode(Mode::Context),
        key: "6  b  F8",
    },
    Entry {
        action: Action::Mode(Mode::Tests),
        key: "7  t  F7",
    },
    Entry {
        action: Action::Mode(Mode::Followup),
        key: "8  F10",
    },
    Entry {
        action: Action::Mode(Mode::Coverage),
        key: "9",
    },
    Entry {
        action: Action::Mode(Mode::Flow),
        key: "0  f  F9",
    },
    Entry {
        action: Action::NextUnreviewed,
        key: "n",
    },
    Entry {
        action: Action::PrevUnreviewed,
        key: "P",
    },
    Entry {
        action: Action::NextCritical,
        key: "N",
    },
    Entry {
        action: Action::ToggleReviewed,
        key: "r",
    },
    Entry {
        action: Action::ToggleBookmark,
        key: "m",
    },
    Entry {
        action: Action::NextBookmark,
        key: ",",
    },
    Entry {
        action: Action::Comment,
        key: "c",
    },
    Entry {
        action: Action::Explain,
        key: "w",
    },
    Entry {
        action: Action::RestoreRules,
        key: "",
    },
    Entry {
        action: Action::InspectContract,
        key: "i",
    },
    Entry {
        action: Action::ToggleInspector,
        key: "K",
    },
    Entry {
        action: Action::OpenWholeFile,
        key: "o",
    },
    Entry {
        action: Action::HistoryBack,
        key: "<",
    },
    Entry {
        action: Action::HistoryForward,
        key: ">",
    },
    Entry {
        action: Action::ToggleVersion,
        key: "v",
    },
    Entry {
        action: Action::PeekPrevious,
        key: "B",
    },
    Entry {
        action: Action::NextChange,
        key: "]",
    },
    Entry {
        action: Action::PrevChange,
        key: "[",
    },
    Entry {
        action: Action::ToggleCodeScope,
        key: "S",
    },
    Entry {
        action: Action::Search,
        key: "/",
    },
    Entry {
        action: Action::FilterHigh,
        key: "H",
    },
    Entry {
        action: Action::FilterUnreviewed,
        key: "u",
    },
    Entry {
        action: Action::FilterIntroduced,
        key: "I",
    },
    Entry {
        action: Action::FilterNoTests,
        key: "T",
    },
    Entry {
        action: Action::FilterNew,
        key: "W",
    },
    Entry {
        action: Action::CycleScope,
        key: "s",
    },
    Entry {
        action: Action::CycleModule,
        key: "M",
    },
    Entry {
        action: Action::ClearFilters,
        key: "x",
    },
    Entry {
        action: Action::DeeperTree,
        key: "+",
    },
    Entry {
        action: Action::ShallowerTree,
        key: "-",
    },
    Entry {
        action: Action::RunTests,
        key: "X",
    },
    Entry {
        action: Action::ReloadReports,
        key: "U",
    },
    Entry {
        action: Action::ReloadFlow,
        key: "L",
    },
    Entry {
        action: Action::RequestFlow,
        key: "Flow: p",
    },
    Entry {
        action: Action::Refresh,
        key: "R",
    },
    Entry {
        action: Action::Finish,
        key: "F",
    },
    Entry {
        action: Action::CopyFeedback,
        key: "",
    },
    Entry {
        action: Action::ConfirmSubmitted,
        key: "",
    },
    Entry {
        action: Action::NextTheme,
        key: "",
    },
    Entry {
        action: Action::Help,
        key: "?  F1",
    },
    Entry {
        action: Action::Quit,
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
        Action::NextTheme => {
            app.theme = app.theme.cycled();
            let name = app.theme.name();
            app.say(
                StatusKind::Info,
                t::theme_switched(
                    app.locale,
                    labels::theme_title(app.locale, name),
                    name.key(),
                ),
            );
        }
        Action::ToggleVersion => toggle_version(app),
        // Holding needs the key itself, which a palette entry does not have, so the palette only
        // says how it works.
        Action::PeekPrevious => app.say(
            StatusKind::Info,
            t::peek_hint(
                app.locale,
                app.keymap.key_for("peek_previous"),
                app.keymap.key_for("toggle_version"),
            ),
        ),
        Action::NextChange => jump_change(app, true),
        Action::PrevChange => jump_change(app, false),
        Action::Explain => match app.unit.clone() {
            Some(unit) => app.overlay = Some(Overlay::Explain(unit)),
            None => app.say(StatusKind::Info, t::select_unit_first(app.locale)),
        },
        Action::InspectContract => match app.unit.clone() {
            Some(unit) => app.overlay = Some(Overlay::Contract(unit)),
            None => app.say(StatusKind::Info, t::select_unit_first(app.locale)),
        },
        Action::ToggleInspector => {
            if app.inspector_fits {
                app.inspector = Some(!app.inspector_shown);
            } else {
                app.say(StatusKind::Info, t::inspector_too_narrow(app.locale));
            }
        }
        Action::RestoreRules => {
            let muted = app.session.muted_rules().to_vec();
            if muted.is_empty() {
                app.say(StatusKind::Info, t::no_rule_silenced(app.locale));
            } else {
                app.overlay = Some(Overlay::Confirm(Confirm {
                    title: t::restore_rules_title(app.locale).to_owned(),
                    lines: vec![
                        t::restore_rules_silenced(app.locale, &muted.join(", ")),
                        t::restore_rules_explain(app.locale).to_owned(),
                    ],
                    action: ConfirmAction::RestoreRules,
                }));
            }
        }
        Action::OpenWholeFile => open_whole_file(app),
        Action::HistoryBack => {
            if !app.history_back() {
                app.say(StatusKind::Info, t::history_no_earlier(app.locale));
            }
        }
        Action::HistoryForward => {
            if !app.history_forward() {
                app.say(StatusKind::Info, t::history_no_later(app.locale));
            }
        }
        Action::RunTests => ask_run_tests(app),
        Action::ReloadReports => {
            app.session.reload_reports();
            let n = app.session.reports().sources.len();
            app.say(StatusKind::Info, t::reports_reloaded(app.locale, n));
        }
        Action::ReloadFlow => reload_flow(app),
        Action::RequestFlow => ask_flow_request(app),
        Action::ClearFilters => {
            app.filters = crate::tui::app::Filters::default();
            refresh_queue_selection(app);
            app.say(StatusKind::Info, t::filters_cleared(app.locale));
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
            Ok(()) => app.say(StatusKind::Info, t::recorded_submitted(app.locale)),
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
        let summary = app.filters.summary(app.locale);
        app.say(
            StatusKind::Info,
            t::filter_status(app.locale, &summary, len),
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
            None => app.say(StatusKind::Info, t::queue_is_empty(app.locale)),
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
pub(crate) fn focus_unit(app: &mut App, unit: &crate::domain::ids::UnitId) {
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
            let message = if critical {
                t::no_unreviewed_critical(app.locale)
            } else {
                t::no_unreviewed(app.locale)
            };
            app.say(StatusKind::Info, message);
        }
    }
}

fn toggle_reviewed(app: &mut App) {
    let Some(unit) = app.unit.clone().or_else(|| app.queue_selection()) else {
        app.say(StatusKind::Info, t::select_unit_first(app.locale));
        return;
    };
    match app.session.toggle_reviewed(&unit) {
        Ok(true) => {
            let c = app.session.coverage();
            app.say(
                StatusKind::Info,
                t::marked_reviewed(app.locale, c.reviewed, c.total),
            );
        }
        Ok(false) => app.say(StatusKind::Info, t::mark_removed(app.locale)),
        Err(error) => app.say(StatusKind::Error, error.to_string()),
    }
    let len = app.visible_queue().len();
    app.queue_view.clamp(len);
}

fn toggle_bookmark(app: &mut App) {
    let Some(unit) = app.unit.clone().or_else(|| app.queue_selection()) else {
        app.say(StatusKind::Info, t::select_unit_first(app.locale));
        return;
    };
    if let Some(position) = app.bookmarks.iter().position(|marked| marked == &unit) {
        app.bookmarks.remove(position);
        app.say(StatusKind::Info, t::bookmark_removed(app.locale));
    } else {
        app.bookmarks.push(unit);
        app.say(
            StatusKind::Info,
            t::bookmarked(app.locale, app.bookmarks.len()),
        );
    }
}

fn next_bookmark(app: &mut App) {
    if app.bookmarks.is_empty() {
        app.say(StatusKind::Info, t::bookmarks_none(app.locale));
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
        Some(_) => app.say(StatusKind::Info, t::bookmark_only_one(app.locale)),
        None => app.say(StatusKind::Info, t::bookmark_none_in_queue(app.locale)),
    }
}

fn start_comment(app: &mut App) {
    let l = app.locale;
    let Some(unit) = app.unit.clone() else {
        app.say(StatusKind::Info, t::select_unit_first(l));
        return;
    };
    if app.mode == Mode::Code && app.code.detached.is_some() {
        app.say(StatusKind::Warn, t::comment_detached(l));
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
        || t::comment_place_unit(l).to_owned(),
        |(a, b)| {
            if a == b {
                t::comment_place_line(l, a)
            } else {
                t::comment_place_lines(l, a, b)
            }
        },
    );
    let side = if version == Version::Before {
        t::comment_side_old(l)
    } else {
        t::comment_side_new(l)
    };
    app.overlay = Some(Overlay::Editor(Editor {
        title: t::comment_title(l, &label, side, &place),
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
    let locale = app.locale;
    let warnings = app
        .session
        .finish_warnings()
        .into_iter()
        .map(|warning| warning.text(locale))
        .collect();
    app.overlay = Some(Overlay::Finish(FinishStage::Confirm { warnings }));
}

fn ask_refresh(app: &mut App) {
    let l = app.locale;
    app.overlay = Some(Overlay::Confirm(Confirm {
        title: t::refresh_title(l).to_owned(),
        lines: vec![
            t::refresh_line_analyse(l).to_owned(),
            t::refresh_line_comments(l).to_owned(),
            t::refresh_line_reviewed(l).to_owned(),
        ],
        action: ConfirmAction::Refresh,
    }));
}

pub(crate) fn toggle_version(app: &mut App) {
    let l = app.locale;
    let Some(view) = &app.code.view else {
        app.say(StatusKind::Info, t::open_code_first(l));
        return;
    };
    if app.code.detached.is_some() {
        app.say(StatusKind::Info, t::version_detached(l));
        return;
    }
    if !view.other_version_exists {
        let why = match app.code.version {
            Some(Version::Before) => t::version_removed(l),
            _ => t::version_new_only(l),
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
        app.say(StatusKind::Info, t::select_unit_first(app.locale));
        return;
    };
    if app.mode != Mode::Code || app.code.view.is_none() {
        app.open_unit(&unit, None);
    }
    if app.code.view.is_none() || app.code.detached.is_some() {
        return;
    }
    app.code.scope = Some(crate::app::CodeScope::File);
    app.rebuild_code();
}

fn toggle_code_scope(app: &mut App) {
    if app.code.view.is_none() || app.code.detached.is_some() {
        return;
    }
    app.code.scope = Some(match app.code.scope {
        Some(crate::app::CodeScope::File) => crate::app::CodeScope::Symbol,
        _ => crate::app::CodeScope::File,
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
                t::change_none_further(app.locale)
            } else {
                t::change_none_earlier(app.locale)
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
    let l = app.locale;
    let Some(unit) = app.unit.clone() else {
        app.say(StatusKind::Info, t::select_unit_first(l));
        return;
    };
    match app.session.plan_test_run(&unit) {
        Ok(request) => {
            let preview = crate::app::ReviewSession::preview_test_run(&request);
            app.overlay = Some(Overlay::Confirm(Confirm {
                title: t::run_tests_title(l).to_owned(),
                lines: vec![
                    t::run_tests_command(l, &preview),
                    t::run_tests_cwd(l, &request.cwd.display().to_string()),
                    t::run_tests_limit(l, request.timeout.as_secs()),
                    t::run_tests_source(l).to_owned(),
                    t::run_tests_busy(l).to_owned(),
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
            let label = labels::flow_state(app.locale, &app.session.flow().state);
            app.say(StatusKind::Info, t::flow_reloaded(app.locale, label));
        }
        Err(error) => app.say(StatusKind::Error, error.to_string()),
    }
}

fn ask_flow_request(app: &mut App) {
    let prompt = app.session.flow().request_prompt.clone();
    app.overlay = Some(Overlay::Confirm(prepare_confirm(
        app,
        t::diagram_request_title(app.locale),
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
    let l = app.locale;
    let mut foreground = None;
    let destination = match &app.agent {
        None => t::destination_none_unmanaged(l).to_owned(),
        Some(channel) => match channel.target() {
            Ok(Some(target)) => {
                foreground = Some(sanitize_single_line(&target.command));
                sanitize_single_line(&target.description)
            }
            Ok(None) => t::destination_none_pane(l).to_owned(),
            Err(error) => t::destination_unverified(l, &error.to_string()),
        },
    };
    let mut lines = vec![t::paste_destination(l, &destination)];
    if let Some(command) = &foreground {
        lines.push(t::paste_running_there(l, command));
        if is_shell(command) {
            lines.push(t::paste_shell_warning(l, command));
        }
        lines.extend(harness_lines(app, command));
    }
    lines.extend([
        t::paste_text_heading(l).to_owned(),
        format!("  {}", sanitize_single_line(text)),
        t::paste_appended(l).to_owned(),
    ]);
    Confirm {
        title: title.to_owned(),
        lines,
        action,
    }
}

/// What the adapter for the program in the pane has to say. The generic adapter says that nothing
/// is known; an adapter that does know can warn that the program looks busy. Either way the
/// answer only informs the confirmation, it never decides whether to paste.
fn harness_lines(app: &App, command: &str) -> Vec<String> {
    let adapter = app.harnesses.select(command);
    let mut lines = vec![t::paste_harness(
        app.locale,
        &sanitize_single_line(adapter.name()),
    )];
    if adapter.id() == "generic" {
        return lines;
    }
    let screen = app
        .agent
        .as_ref()
        .and_then(|channel| channel.screen_tail(HARNESS_SCREEN_LINES).ok().flatten());
    if let Some(screen) = screen
        && let crate::domain::harness::Readiness::Busy(reason) = adapter.readiness(&screen)
    {
        lines.push(format!("⚠ {}", sanitize_single_line(&reason)));
    }
    lines
}

/// How much of the pane's screen an adapter is shown.
const HARNESS_SCREEN_LINES: usize = 20;

fn copy_feedback(app: &mut App) {
    let Some(clipboard) = app.clipboard.clone() else {
        app.say(StatusKind::Warn, t::clipboard_unavailable(app.locale));
        return;
    };
    match app.session.copy_feedback(clipboard.as_ref()) {
        Ok(()) => app.say(StatusKind::Info, t::feedback_copied(app.locale)),
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
