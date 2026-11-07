//! Things the reader can do. Every key and every palette entry maps to an [`Action`], so the two
//! cannot drift apart and the help screen is generated from the same table.

use crate::domain::finding::Priority;
use crate::domain::graph::ImpactScope;
use crate::domain::i18n::{Locale, tui as t};
use crate::domain::review::Version;

use crate::tui::app::{App, Confirm, ConfirmAction, Depth, Mode, Overlay, StatusKind};
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
            Action::NextUnreviewed => t::action_next_unreviewed(locale),
            Action::PrevUnreviewed => t::action_prev_unreviewed(locale),
            Action::NextCritical => t::action_next_critical(locale),
            Action::ToggleReviewed => t::action_toggle_reviewed(locale),
            Action::ToggleBookmark => t::action_toggle_bookmark(locale),
            Action::NextBookmark => t::action_next_bookmark(locale),
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
            Action::Refresh => t::action_refresh(locale),
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
        action: Action::Refresh,
        key: "R",
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
    app.go(mode);
}

/// Makes `unit` the current one; the code view follows when it is on screen.
pub(crate) fn focus_unit(app: &mut App, unit: &crate::domain::ids::UnitId) {
    app.adopt(unit.clone());
    app.select_in_queue(unit);
    // The code pane of a list follows the cursor by itself; only the focused code is rebuilt here.
    if app.mode == Mode::Code {
        app.open_unit(unit, None);
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
    if app.code.view.is_none() {
        return;
    }
    app.code.scope = Some(crate::app::CodeScope::File);
    app.rebuild_code();
}

fn toggle_code_scope(app: &mut App) {
    if app.code.view.is_none() {
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
