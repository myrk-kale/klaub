//! Sentences of the interactive reader.
#![allow(missing_docs)] // The name of each entry says what it is for.

// ---- words for states and kinds (shown in lists, chips and the status bar) ----------------------

texts! {
    const mode_queue { en: "Queue" }
    const mode_structure { en: "Structure" }
    const mode_code { en: "Code" }
    const mode_impact { en: "Impact" }
    const mode_comments { en: "Comments" }
    const mode_context { en: "Context" }
    const mode_tests { en: "Tests" }
    const mode_followup { en: "Follow-up" }
    const mode_coverage { en: "Coverage" }
    const mode_flow { en: "Flow" }

    const origin_resolved { en: "resolved" }
    const origin_heuristic { en: "heuristic" }
    const origin_unresolved { en: "unresolved" }

    const entrypoint_cli { en: "cli" }
    const entrypoint_main { en: "main" }
    const entrypoint_worker { en: "worker" }
    const entrypoint_export { en: "export" }
    const entrypoint_external { en: "external" }
    const entrypoint_internal { en: "internal" }

    const edge_calls { en: "calls" }
    const edge_imports { en: "imports" }
    const edge_exports { en: "exports" }
    const edge_implements { en: "implements" }
    const edge_extends { en: "extends" }
    const edge_references { en: "references" }

    const comment_draft { en: "draft" }
    const comment_prepared { en: "prepared" }
    const comment_submitted { en: "submitted" }
    const comment_addressed { en: "addressed" }
    const comment_unresolved { en: "unresolved" }
    const comment_stale { en: "stale" }

    const criterion_not_checked { en: "not checked" }
    const criterion_verified { en: "verified" }
    const criterion_pending { en: "pending" }
    const criterion_not_applicable { en: "n/a" }

    const outcome_passed { en: "passed" }
    const outcome_failed { en: "FAILED" }
    const outcome_skipped { en: "skipped" }
    const outcome_not_run { en: "not run" }

    const review_draft { en: "draft" }
    const review_finished { en: "finished" }
    const review_prepared { en: "prepared" }
    const review_submitted { en: "submitted" }

    const change_added { en: "added" }
    const change_removed { en: "removed" }
    const change_modified { en: "modified" }
    const change_moved { en: "moved" }
    const change_renamed { en: "renamed" }
    const change_signature { en: "signature" }
    const change_rewritten { en: "rewritten" }
    const change_format { en: "format" }
    const change_unknown { en: "unknown" }

    const tag_contract { en: "contract" }
    const tag_new { en: "new" }
    const tag_removed { en: "removed" }
    const tag_rewrite { en: "rewrite" }
    const tag_moved { en: "moved" }
    const tag_format { en: "format" }
    const tag_no_tests { en: "no-tests" }
    const tag_unknown { en: "unknown" }
    const tag_generated { en: "generated" }

    const component_contract { en: "contract" }
    const component_reach { en: "reach" }
    const component_criticality { en: "criticality" }
    const component_complexity { en: "complexity" }
    const component_risk { en: "risk" }
    const component_tests { en: "tests" }
    const component_architecture { en: "architecture" }
    const component_mitigation { en: "mitigation" }

    const reach_local { en: "local" }
    const reach_module { en: "module" }
    const reach_cross_module { en: "cross-module" }
    const reach_global { en: "global" }

    const temporal_introduced { en: "introduced" }
    const temporal_preexisting { en: "preexisting" }
    const temporal_removed { en: "removed" }
    const temporal_unknown { en: "unknown origin" }

    const evidence_no_association { en: "no association found" }
    const evidence_not_run { en: "not run" }
    const evidence_passed { en: "passed" }
    const evidence_failed { en: "failed" }
    const evidence_stale { en: "stale" }

    const link_name { en: "name" }
    const link_import { en: "import" }
    const link_reference { en: "reference" }
    const link_configured { en: "configured" }

    const followup_pending { en: "pending" }
    const followup_resolved { en: "resolved" }
    const followup_partial { en: "partial" }
    const followup_unresolved { en: "unresolved" }
    const followup_regression { en: "regression" }
    const followup_stale { en: "stale" }

    const flow_current { en: "current" }
    const flow_missing { en: "missing" }
    const flow_invalid { en: "invalid" }
    const flow_stale { en: "stale" }

    const theme_light { en: "Light" }
    const theme_high_contrast { en: "High contrast" }

    const severity_none { en: "none" }
    const severity_label { en: "severity " }
}

// ---- queue filters ------------------------------------------------------------------------------

texts! {
    const filter_unreviewed { en: "unreviewed" }
    const filter_introduced { en: "introduced" }
    const filter_no_tests { en: "no-tests" }
    const filter_new { en: "new" }
}

messages! {
    fn filter_scope(scope: &str) { en: "scope ≥{scope}" }
    fn filter_module(module: &str) { en: "module {module}" }
}

// ---- main area: queue ---------------------------------------------------------------------------

texts! {
    const queue_nothing_changed { en: "Nothing changed relative to the base: there is nothing to review." }
    const queue_no_match { en: "No unit matches the filters. x clears them." }
    const queue_details { en: "Details" }
    const explain_why { en: "Why this priority" }
    const explain_no_factor { en: "  no contributing factor" }
    const explain_findings { en: "Findings" }
    const explain_false_positive { en: "A false positive? Press the finding's number to silence its rule for this review." }
}

messages! {
    fn queue_title(visible: usize, total: usize, summary: &str) { en: "Queue · {visible} of {total} unit(s){summary}" }
    fn queue_filter_summary(filters: &str) { en: " · filter: {filters}" }
    fn explain_score(score: i32, label: &str) { en: " score {score} · {label}" }
    fn explain_silenced(rules: &str) { en: "Silenced here: {rules}" }
    fn explain_reach(scope: &str, entrypoints: u32) { en: "Reach: {scope} · {entrypoints} entrypoint(s)" }
}

// ---- main area: structure, code -----------------------------------------------------------------

texts! {
    const structure_hint { en: "modules → files → symbols (+ / - to open or close levels)" }
    const structure_empty { en: "Nothing to show with the current filters." }
    const code_empty { en: "No code is open. Pick a unit in the Queue and press Enter." }
    const code_new { en: "NEW CODE" }
    const code_old { en: "OLD CODE" }
    const code_symbol { en: "symbol" }
}

messages! {
    fn code_lines(from: u32, to: u32) { en: " · lines {from}–{to}" }
    fn code_more_comments(count: usize) { en: " (+{count} more)" }
}

// ---- main area: impact --------------------------------------------------------------------------

texts! {
    const impact_no_subject { en: "no symbol" }
    const impact_whole_file { en: "This unit is a whole file, not a symbol, so it has no call graph." }
    const impact_not_indexed { en: "This symbol is not in the call graph (not indexed, or in an unsupported language)." }
    const impact_depth_limit { en: " · depth limit reached" }
    const impact_entrypoints { en: "Entrypoints that can reach it" }
    const impact_no_entrypoints { en: "none found (it may be reached in ways the analysis cannot see)" }
    const impact_callers { en: "Callers" }
    const impact_no_callers { en: "no callers found" }
    const impact_calls { en: "Calls" }
    const impact_no_calls { en: "no calls into the project" }
}

messages! {
    fn impact_title(subject: &str) { en: "Impact · {subject}" }
    fn impact_summary(scope: &str, direct: usize, entrypoints: usize, external: usize) {
        en: "{scope} · {direct} direct caller(s) · {entrypoints} entrypoint(s), {external} external"
    }
    fn impact_evidence(resolved: u32, heuristic: u32, unresolved: u32, tail: &str) {
        en: "evidence: {resolved} resolved · {heuristic} heuristic · {unresolved} unresolved call(s){tail}"
    }
    fn impact_cannot_show(symbol: &str) { en: "Cannot show {symbol}: the file is unreadable or not indexed." }
}

// ---- main area: comments, context ---------------------------------------------------------------

texts! {
    const comments_empty { en: "No comments yet. Open some code and press c." }
    const comment_target_file { en: "file" }
    const context_hint { en: "intent and acceptance criteria (you decide what is verified)" }
    const context_intent { en: "Intent  " }
    const context_intent_unset { en: "(not set — Enter to write it)" }
    const context_untrusted { en: "Text from a brief file is untrusted data. Enter changes status, a adds a criterion, i edits the intent." }
}

messages! {
    fn comments_title(total: usize) { en: "Comments · {total}" }
    fn comments_hint(edit: char, delete: char) { en: "(Enter opens the code, {edit} edit, {delete} delete drafts)" }
    fn comment_detail(status: &str, confidence: &str, target: &str) { en: "{status} · anchor confidence {confidence}% · {target}" }
}

// ---- main area: tests ---------------------------------------------------------------------------

texts! {
    const tests_select_unit { en: "Select a unit in the Queue first." }
    const tests_hint { en: "(X runs them after asking, U reloads reports)" }
    const tests_evidence { en: "Evidence: " }
    const tests_none_linked { en: "No test was linked to this symbol. That does not prove there is none: Klaub only links tests it can see calling or importing it." }
    const tests_not_run { en: "Tests are associated, but no imported report has a result for them." }
    const tests_passed { en: "Every associated test with a result passed in an imported report." }
    const tests_failed { en: "At least one associated test failed in an imported report." }
    const tests_stale { en: "The report is older than the newest change; do not trust it." }
    const tests_associated { en: "Associated tests" }
    const tests_imported { en: "Imported results" }
    const tests_no_report { en: "No test report is configured or found (tests.report_paths)." }
    const tests_stale_suffix { en: " · STALE" }
    const tests_output { en: "Output of the last run (tail)" }
}

messages! {
    fn tests_title(unit: &str) { en: "Tests · {unit}" }
    fn tests_link(name: &str, file: &str, line: u32, method: &str, confidence: &str) {
        en: "  {name} ({file}:{line}) via {method} · {confidence}%"
    }
    fn tests_result(name: &str, outcome: &str) { en: "  {name} — {outcome}" }
    fn tests_coverage(file: &str, covered: usize, missed: usize) {
        en: "Coverage of {file}: {covered} line(s) covered, {missed} missed (file level, from an imported report)"
    }
    fn tests_report(path: &str, stale: &str) { en: "Report: {path}{stale}" }
}

// ---- main area: follow-up, coverage, flow -------------------------------------------------------

texts! {
    const followup_hint { en: "(y resolved · p partial · u unresolved · X regression)" }
    const followup_empty { en: "No comments were carried over. After you hand feedback to the agent and it changes the code, opening a new review continues from here." }
    const coverage_hint { en: "what you marked as reviewed (never an approval)" }
    const coverage_overall { en: "Overall  " }
    const flow_hint { en: "(Enter opens a step · ←/→/Space fold · p asks the agent · u reloads)" }
    const flow_not_found { en: "Flow diagram not found" }
    const flow_press_p { en: "Press p to prepare a request for the agent (you confirm first, and Enter is never sent)." }
    const flow_works_without { en: "The review works fully without a diagram." }
    const flow_unusable { en: "The flow file cannot be used" }
    const flow_fix { en: "Fix the file or ask the agent again (p). It is never used partially." }
    const flow_external { en: "external" }
    const flow_symbol_missing { en: " ⚠ symbol not found" }
    const flow_changed { en: " [changed]" }
    const flow_shown_above { en: " ↺ shown above" }
}

messages! {
    fn followup_title(count: usize) { en: "Follow-up · {count} comment(s) from the previous round" }
    fn coverage_unknown(count: usize) { en: "  {count} unit(s) could not be analysed semantically; they are counted, not hidden." }
    fn coverage_pending_critical(count: usize) { en: "  {count} P0/P1 unit(s) not reviewed yet (N jumps to the next)" }
    fn coverage_notice(note: &str) { en: "  notice: {note}" }
    fn flow_title(state: &str) { en: "Flow · {state}" }
    fn flow_expected(path: &str) { en: "Expected at {path}" }
    fn flow_snapshot_mismatch(found: &str) { en: "Written for snapshot {found} — not this one. Treat it as background only." }
    fn flow_written_by(author: &str) { en: "Written by {author} — an explanation, not proof. ● resolved ◐ heuristic ○ unresolved" }
    fn flow_path(crumbs: &str) { en: "path: {crumbs}" }
}

// ---- frame: title bar, tabs, status bar ---------------------------------------------------------

texts! {
    const drift_banner { en: " worktree changed since this snapshot, so impact may mix versions. R takes a new one (asks first)" }
}

messages! {
    fn too_small(width: u16, height: u16, min_width: u16, min_height: u16) {
        en: "Terminal too small ({width}×{height}, need at least {min_width}×{min_height}). Resize it, or press q twice to leave."
    }
    fn title_progress(progress: &str, state: &str) { en: " {progress} reviewed · {state} " }
}

// ---- key hints in the status bar ----------------------------------------------------------------

texts! {
    const hint_move { en: "move" }
    const hint_scroll { en: "scroll" }
    const hint_open { en: "open" }
    const hint_comment { en: "comment" }
    const hint_reviewed { en: "reviewed" }
    const hint_why { en: "why" }
    const hint_search { en: "search" }
    const hint_clear_filters { en: "clear filters" }
    const hint_help { en: "help" }
    const hint_quit { en: "quit" }
    const hint_depth { en: "depth" }
    const hint_contract { en: "contract" }
    const hint_file { en: "file" }
    const hint_old_new { en: "old/new" }
    const hint_change { en: "change" }
    const hint_select { en: "select" }
    const hint_back_forward { en: "back/forward" }
    const hint_open_symbol { en: "open the symbol" }
    const hint_back_to_queue { en: "back to the queue" }
    const hint_open_code { en: "open the code" }
    const hint_edit { en: "edit" }
    const hint_delete_draft { en: "delete a draft" }
    const hint_set_intent { en: "set intent" }
    const hint_add_criterion { en: "add criterion" }
    const hint_run_tests { en: "run (asks first)" }
    const hint_reload_reports { en: "reload reports" }
    const hint_resolved { en: "resolved" }
    const hint_partial { en: "partial" }
    const hint_not_resolved { en: "not resolved" }
    const hint_regression { en: "regression" }
    const hint_open_pending { en: "open a pending unit" }
    const hint_ask_diagram { en: "ask the agent for a diagram" }
    const hint_reload { en: "reload" }
}

// ---- overlays -----------------------------------------------------------------------------------

texts! {
    const help_title { en: "Help" }
    const help_close { en: "Keys — press any key to close" }
    const help_palette_key { en: "palette" }
    const palette_title { en: "Actions" }
    const palette_no_match { en: "no match" }
    const search_title { en: "Search" }
    const hint_filters_queue { en: " filters the queue · " }
    const hint_cancels { en: " cancels" }
    const hint_saves { en: " saves · " }
    const hint_new_line { en: " new line · " }
    const hint_severity { en: " severity · " }
    const hint_confirms { en: " confirms · " }
    const hint_cancels_nothing_sent { en: " cancels · nothing is sent on its own" }
    const hint_finishes { en: " finishes · " }
    const hint_goes_back { en: " goes back" }
    const hint_chooses { en: " chooses · " }
    const hint_closes_finished { en: " closes (the review stays finished)" }
    const editor_empty { en: "(empty)" }
    const notice_continue { en: "Press any key to continue." }
    const explain_gone { en: "This unit is no longer in the queue." }
    const explain_title { en: "Priority" }
    const contract_title { en: "Contract" }
    const contract_none { en: "This unit has no contract to inspect: it is not a symbol, a migration, a dependency manifest or a settings file." }
    const contract_any_key { en: "any key closes" }
    const contract_change { en: "change" }
    const contract_confidence { en: "confidence" }
    const contract_data_type { en: "data type" }
    const contract_before { en: "before" }
    const contract_after { en: "after" }
    const contract_parameters { en: "parameters" }
    const contract_returns { en: "returns" }
    const contract_visibility { en: "visibility" }
    const contract_decorators { en: "decorators" }
    const contract_endpoints { en: "endpoints" }
    const contract_raises { en: "raises" }
    const contract_effects { en: "effects" }
    const contract_members { en: "members" }
    const contract_reachable_from { en: "reachable from" }
    const contract_no_entrypoint { en: "no entrypoint found (absence of a path is not proof of no impact)" }
    const contract_direct_callers { en: "direct callers" }
    const contract_evidence { en: "evidence" }
    const contract_none_word { en: "none" }
    const contract_file { en: "file" }
    const contract_kind { en: "kind" }
    const contract_read { en: "read" }
    const contract_yes { en: "yes" }
    const contract_no_textual { en: "no, use the textual diff" }
    const contract_added { en: "added" }
    const contract_removed { en: "removed" }
    const contract_changed { en: "changed" }
    const contract_notes { en: "notes" }
    const file_kind_migration { en: "migration" }
    const file_kind_manifest { en: "dependency manifest" }
    const file_kind_config { en: "configuration" }
    const finish_title { en: "Finish the review" }
    const finish_intro { en: "Finishing freezes the comments into a feedback file. It sends nothing." }
    const finish_no_warnings { en: "✓ No warnings." }
    const finish_warnings { en: "Warnings (they do not block):" }
    const deliver_title { en: "Deliver the feedback" }
    const deliver_pasted { en: "The instruction that would be pasted:" }
    const delivery_prepare { en: "Prepare in agent" }
    const delivery_copy { en: "Copy feedback" }
    const delivery_show { en: "Show file" }
    const delivery_close { en: "Close" }
}

messages! {
    fn clip_more_lines(count: usize) { en: "… {count} more lines; make the terminal taller" }
    fn deliver_saved_as(id: &str) { en: "Saved as {id}" }
}

// ---- opening the reader ---------------------------------------------------------------------------

texts! {
    const continuing_round { en: "Continuing a previous round: decide each carried-over comment in Follow-up (D)." }
    const storage_notice_title { en: "Where Klaub keeps its files" }
    const storage_notice_text { en: "Your comments, briefs and flow diagrams are saved in a .klaub/ folder inside this repository. It is local data.\n\nKlaub does not add it to .gitignore or .git/info/exclude, and it never commits it. If you do not want .klaub/ in git, add it to .gitignore yourself; if you want to share a flow diagram, commit .klaub/flows/ yourself.\n\nKlaub always leaves .klaub/ out of the changes it reviews, so saving a comment never starts a new snapshot." }
}

// ---- action names (palette and help) ------------------------------------------------------------

texts! {
    const action_go_queue { en: "Go to Queue" }
    const action_go_structure { en: "Go to Structure" }
    const action_go_code { en: "Go to Code" }
    const action_go_impact { en: "Go to Impact (callers, entrypoints)" }
    const action_go_comments { en: "Go to Comments" }
    const action_go_context { en: "Go to Context (brief, criteria)" }
    const action_go_tests { en: "Go to Tests" }
    const action_go_followup { en: "Go to Follow-up" }
    const action_go_coverage { en: "Go to Coverage" }
    const action_go_flow { en: "Go to Flow" }
    const action_next_unreviewed { en: "Next unreviewed" }
    const action_prev_unreviewed { en: "Previous unreviewed" }
    const action_next_critical { en: "Next unreviewed P0/P1" }
    const action_toggle_reviewed { en: "Mark reviewed / unmark" }
    const action_toggle_bookmark { en: "Bookmark this unit" }
    const action_next_bookmark { en: "Next bookmark" }
    const action_comment { en: "Comment on selection" }
    const action_explain { en: "Why this priority?" }
    const action_restore_rules { en: "Bring back silenced rules" }
    const action_inspect_contract { en: "Inspect the contract" }
    const action_open_whole_file { en: "Open the unit in the whole file" }
    const action_history_back { en: "Back to the previous unit" }
    const action_history_forward { en: "Forward to the next unit in the history" }
    const action_toggle_version { en: "Toggle before / after" }
    const action_peek_previous { en: "Hold to see the previous version" }
    const action_next_change { en: "Next change" }
    const action_prev_change { en: "Previous change" }
    const action_toggle_code_scope { en: "Code: symbol / whole file" }
    const action_search { en: "Search" }
    const action_filter_high { en: "Filter: P0/P1 only" }
    const action_filter_unreviewed { en: "Filter: unreviewed only" }
    const action_filter_introduced { en: "Filter: introduced by this change" }
    const action_filter_no_tests { en: "Filter: no associated tests" }
    const action_filter_new { en: "Filter: new files" }
    const action_cycle_scope { en: "Filter: minimum reach (local/module/global)" }
    const action_cycle_module { en: "Filter: next module" }
    const action_clear_filters { en: "Clear filters" }
    const action_deeper_tree { en: "Structure: show more" }
    const action_shallower_tree { en: "Structure: show less" }
    const action_run_tests { en: "Run associated tests (asks first)" }
    const action_reload_reports { en: "Reload test reports" }
    const action_reload_flow { en: "Reload flow" }
    const action_request_flow { en: "Prepare flow request in agent" }
    const action_refresh { en: "Refresh snapshot" }
    const action_finish { en: "Finish review…" }
    const action_copy_feedback { en: "Copy feedback instruction" }
    const action_confirm_submitted { en: "I submitted the feedback in the agent" }
    const action_next_theme { en: "Switch colour theme" }
    const action_help { en: "Help" }
    const action_quit { en: "Close the reader" }
}

// ---- messages in the status bar and questions to confirm ----------------------------------------

texts! {
    const select_unit_first { en: "Select a unit first." }
    const open_code_first { en: "Open some code first." }
    const restore_rules_title { en: "Bring back the silenced rules?" }
    const restore_rules_explain { en: "Their findings return and the queue is recomputed on this snapshot." }
    const no_rule_silenced { en: "No rule is silenced in this review." }
    const history_no_earlier { en: "No earlier unit in the history." }
    const history_no_later { en: "No later unit in the history." }
    const filters_cleared { en: "Filters cleared." }
    const queue_is_empty { en: "Nothing to show: the queue is empty." }
    const no_unreviewed_critical { en: "No unreviewed P0/P1 unit left in view." }
    const no_unreviewed { en: "No unreviewed unit left in view." }
    const mark_removed { en: "Mark removed." }
    const bookmark_removed { en: "Bookmark removed." }
    const bookmarks_none { en: "No bookmarks yet. m marks the current unit." }
    const bookmark_only_one { en: "Already on the only bookmark." }
    const bookmark_none_in_queue { en: "No bookmark is still in the queue." }
    const comment_detached { en: "This code is not part of the change; comments attach to changed code only." }
    const comment_side_old { en: "old" }
    const comment_side_new { en: "new" }
    const comment_place_unit { en: "whole unit" }
    const refresh_title { en: "Take a new snapshot?" }
    const refresh_line_analyse { en: "The changes are analysed again from the current worktree." }
    const refresh_line_comments { en: "Comments are moved to where their code went; uncertain ones become stale." }
    const refresh_line_reviewed { en: "Units marked reviewed stay marked only if their code did not change." }
    const version_detached { en: "This code is not part of the change; there is no other version." }
    const version_removed { en: "this code was removed; there is no new version" }
    const version_new_only { en: "NEW · there is no previous version to compare with" }
    const change_none_further { en: "No further change in this code. n goes to the next unreviewed unit." }
    const change_none_earlier { en: "No earlier change in this code." }
    const run_tests_title { en: "Run these tests?" }
    const run_tests_source { en: "The command comes from your .klaub.toml; Klaub never invents one." }
    const run_tests_busy { en: "The screen is busy until it finishes or times out." }
    const diagram_request_title { en: "Prepare the diagram request in the agent?" }
    const feedback_request_title { en: "Prepare the feedback in the agent?" }
    const destination_none_unmanaged { en: "none — Klaub is not managing an agent here; use Copy instead" }
    const destination_none_pane { en: "none — no agent pane was found; use Copy instead" }
    const paste_text_heading { en: "text to paste (Klaub never presses Enter; you decide when to send it):" }
    const paste_appended { en: "If the agent's input already has text, the paste may be appended to it." }
    const clipboard_unavailable { en: "No clipboard is available here. Use Show file instead." }
    const feedback_copied { en: "Feedback instruction copied. Paste it into the agent yourself." }
    const recorded_submitted { en: "Recorded: you submitted the feedback." }
}

messages! {
    fn theme_switched(name: &str, key: &str) { en: "Theme: {name}. Set ui.theme = \"{key}\" to keep it." }
    fn peek_hint(hold: char, toggle: char) { en: "Hold {hold} while reading code to see the previous version; {toggle} toggles it." }
    fn restore_rules_silenced(rules: &str) { en: "silenced here: {rules}" }
    fn reports_reloaded(count: usize) { en: "Test reports reloaded ({count} source(s))." }
    fn filter_status(summary: &str, count: usize) { en: "Filter: {summary} — {count} unit(s)." }
    fn marked_reviewed(done: usize, total: usize) { en: "Marked reviewed ({done}/{total}). Press r again to undo, n for the next." }
    fn bookmarked(count: usize) { en: "Bookmarked ({count}). , jumps to the next." }
    fn comment_title(unit: &str, side: &str, place: &str) { en: "Comment on {unit} ({side} code, {place})" }
    fn comment_place_line(line: u32) { en: "line {line}" }
    fn comment_place_lines(from: u32, to: u32) { en: "lines {from}–{to}" }
    fn run_tests_command(command: &str) { en: "command: {command}" }
    fn run_tests_cwd(dir: &str) { en: "in:      {dir}" }
    fn run_tests_limit(seconds: u64) { en: "limit:   {seconds} s" }
    fn flow_reloaded(state: &str) { en: "Flow reloaded: {state}." }
    fn destination_unverified(why: &str) { en: "unverified — {why}" }
    fn paste_destination(destination: &str) { en: "destination: {destination}" }
    fn paste_running_there(command: &str) { en: "running there: {command}" }
    fn paste_shell_warning(command: &str) { en: "⚠ {command} is a shell. If your agent is not the one reading the keyboard, the text lands on a shell prompt." }
    fn paste_harness(name: &str) { en: "harness: {name}" }
}

// ---- messages after input (update) --------------------------------------------------------------

texts! {
    const peek_already_previous { en: "Already showing the previous version." }
    const drift_detected { en: "The worktree changed since this snapshot. Press R to take a new one." }
    const tests_timed_out { en: "timed out" }
    const tests_cancelled { en: "was cancelled" }
    const tests_passed_exit { en: "passed (exit 0)" }
    const tests_no_exit_code { en: "ended without an exit code" }
    const snapshot_taken { en: "New snapshot taken; comments were relocated." }
    const quit_again { en: "Press q again to close the reader. Everything is already saved." }
    const impact_pivoted { en: "Showing the impact of that symbol. Esc goes back." }
    const impact_pick_target { en: "Select a caller, callee or entrypoint to pivot on." }
    const comment_code_missing { en: "The code this comment was about is not in this snapshot." }
    const comment_edit_title { en: "Edit comment" }
    const comment_edit_only_drafts { en: "Only drafts can be edited; this one was already handed over." }
    const comment_delete_title { en: "Delete this comment?" }
    const comment_delete_only_drafts { en: "Only drafts can be deleted; this one was already handed over." }
    const intent_title { en: "Intent of the change (what the author wanted)" }
    const criterion_title { en: "New acceptance criterion" }
    const followup_code_missing { en: "That code is not in this snapshot; the comment is stale." }
    const coverage_showing_pending { en: "Showing what is left to review in that module. x clears the filters." }
    const flow_step_no_symbol { en: "This step does not point to a symbol in the project." }
    const search_cancelled { en: "Cancelled. Nothing was changed." }
    const mute_title { en: "Silence this rule for this review?" }
    const mute_explain_1 { en: "Use it for a false positive. Every finding of this rule disappears from this review" }
    const mute_explain_2 { en: "and the scores that counted it drop. The code under review does not change." }
    const mute_restore_hint { en: "Bring it back from the palette: \"Bring back silenced rules\"." }
    const saved_comment { en: "Comment saved." }
    const saved_comment_updated { en: "Comment updated." }
    const saved_intent { en: "Intent saved." }
    const saved_criterion { en: "Criterion added." }
    const tests_running { en: "Running tests… the screen is busy until they finish or time out." }
    const snapshot_taking { en: "Taking a new snapshot…" }
    const rules_restored { en: "Silenced rules are back." }
    const agent_none_feedback { en: "No agent pane is managed here. Use Copy feedback or Show file." }
    const agent_prepared_feedback { en: "Prepared in the agent's input. Switch to it (prefix + A), read it, and press Enter yourself." }
    const agent_none_diagram { en: "No agent pane is managed here. Copy the request from the Flow screen instead." }
    const agent_prepared_diagram { en: "Diagram request prepared in the agent. Press Enter there when you want it sent; then U/L reloads the flow." }
    const comment_deleted { en: "Comment deleted." }
    const finished_delivery_again { en: "Review finished and saved. F offers the delivery options again." }
    const finished_saved { en: "Review finished and saved." }
}

messages! {
    fn peek_unsupported(key: char, toggle: char) { en: "This terminal does not report key releases, so {key} cannot be held. {toggle} toggles old and new." }
    fn tests_finished(what: &str, seconds: f32) { en: "Tests {what} in {seconds:.1}s. Reports were re-read." }
    fn tests_failed_exit(code: i32) { en: "failed (exit {code})" }
    fn module_only(module: &str) { en: "Showing module {module} only. x clears the filter." }
    fn followup_decision_title(status: &str) { en: "Your decision: {status} — optional note" }
    fn search_matches(count: usize) { en: "{count} unit(s) match. x clears the search." }
    fn mute_rule_line(rule: &str) { en: "rule:     {rule}" }
    fn mute_finding_line(title: &str) { en: "finding:  {title}" }
    fn recorded_decision(status: &str) { en: "Recorded: {status}." }
    fn rule_silenced(rule: &str) { en: "Rule {rule} is silenced for this review." }
    fn agent_could_not_prepare_feedback(why: &str) { en: "Could not prepare it: {why}. Use Copy feedback or Show file." }
    fn agent_could_not_prepare(why: &str) { en: "Could not prepare it: {why}." }
    fn feedback_file(file: &str, prompt: &str) { en: "Feedback file: {file} — tell the agent: {prompt}" }
}

// ---- warnings before finishing a review -----------------------------------------------------------

texts! {
    const finish_warning_drift { en: "The worktree changed after this snapshot was taken, so the comments and the impact refer to code that has moved on. R takes a new snapshot." }
}

messages! {
    fn finish_warning_pending(count: usize) { en: "{count} P0/P1 unit(s) are not marked as reviewed" }
    fn finish_warning_criteria(count: usize) { en: "{count} acceptance criteria are still unchecked" }
    fn finish_warning_untested(count: usize) { en: "{count} changed symbol(s) have no associated test (which does not prove there is none)" }
}
