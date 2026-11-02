//! Sentences of the command line.
#![allow(missing_docs)] // The name of each entry says what it is for.

// ---- shared pieces ------------------------------------------------------------------------------

texts! {
    const invalid_review_id { en: "invalid review id" }
    const cancelled { en: "cancelled" }
    /// Lower-case answers that mean yes, separated by spaces. `y` and `yes` are always accepted.
    const yes_answers { en: "y yes" }
    const yes_no_prompt { en: "[y/N]" }
}

messages! {
    fn cannot_load_review(id: &str) { en: "cannot load review {id}" }
    fn cannot_load_configuration(path: &str) { en: "cannot load configuration for {path}" }
    fn cannot_open_review(path: &str) { en: "cannot open a review of {path}" }
}

// ---- klaub review, reviews, feedback ------------------------------------------------------------

texts! {
    const review_needs_terminal { en: "the reviewer is interactive and needs a terminal; run it in your terminal or through `klaub wrap`" }
    const keys_table_invalid { en: "the [keys] table in the configuration is not valid" }
    const invalid_session_value { en: "invalid --session value" }
    const cannot_read_cwd { en: "cannot read the current directory" }
}

messages! {
    fn review_finished(id: &str, comments: usize, file: &str) { en: "review {id} finished: {comments} comment(s). Feedback: {file}" }
    fn review_closed_unfinished(comments: usize, file: &str) { en: "the review was closed without being finished; the draft ({comments} comment(s)) is in {file}" }
    fn cannot_load_session(id: &str) { en: "cannot load session {id}" }
    fn review_listing(id: &str, status: &str, comments: usize, updated: &str) { en: "{id}  {status:<9}  {comments:>4} comments  updated {updated}" }
    fn review_summary(id: &str, status: &str, comments: usize) { en: "{id}  {status}  {comments} comments" }
    fn review_not_finished(id: &str) { en: "review {id} is not finished; it has no feedback yet" }
    fn review_made_for_earlier(id: &str) { en: "review {id} was made for an earlier state of this worktree and the files have changed since. Open the current review with `klaub review`, or find it with `klaub reviews list`" }
    fn cannot_read_worktree_to_compare(id: &str) { en: "cannot read the worktree to compare it with {id}" }
}

// ---- klaub session, wrap ------------------------------------------------------------------------

texts! {
    const sessions_none { en: "no sessions" }
    const session_attach_none { en: "no running sessions; start one with `klaub wrap <command>`" }
    const session_kill_needs_yes { en: "refusing to end a session without confirmation; pass --yes" }
    const wrap_inside_tmux { en: "klaub wrap cannot run inside tmux: nested sessions are not supported. Detach from tmux first (or open a plain terminal) and run it again. `klaub review` works inside tmux." }
    const wrap_not_git { en: "not inside a Git repository; the reviewer needs one to compare changes" }
}

messages! {
    fn session_not_an_id(raw: &str) { en: "{raw:?} is not a session id" }
    fn session_several(ids: &str) { en: "several sessions are running ({ids}); pass one of them to `klaub session attach <id>`" }
    fn session_ended(id: &str) { en: "session {id} has ended; its record can be removed with `klaub session kill {id}`" }
    fn session_kill_prompt(id: &str, command: &str, answers: &str) { en: "End session {id} and the agent running in it (`{command}`)? {answers} " }
    fn session_killed(id: &str) { en: "ended session {id}" }
    fn session_started(id: &str) { en: "started session {id}" }
    fn session_attach_with(id: &str) { en: "attach with: klaub session attach {id}" }
}

// ---- klaub tests --------------------------------------------------------------------------------

texts! {
    const tests_not_confirmed { en: "not confirmed; nothing was run" }
    const tests_time_limit { en: "the test run hit its time limit" }
    const tests_run_cancelled { en: "the test run was cancelled" }
    const tests_nothing_to_run { en: "no association found; there is nothing to run" }
    const tests_several_commands { en: "the review spans more than one test command; pass --select to choose tests for one" }
    const tests_selected_several { en: "the selected tests need more than one test command; run them separately" }
    const tests_needs_terminal { en: "refusing to run tests without a terminal; pass --yes to confirm" }
    const tests_cannot_prompt { en: "cannot prompt for confirmation" }
    const tests_cannot_read_answer { en: "cannot read the confirmation" }
}

messages! {
    fn tests_exit_code(code: i32) { en: "exit code {code}" }
    fn tests_invalid_id(raw: &str) { en: "invalid test id {raw:?}" }
    fn tests_not_in_plan(id: &str) { en: "test {id} is not part of this review's plan" }
    fn tests_run_prompt(answers: &str) { en: "Run this? {answers} " }
}

// ---- klaub crashes, version ---------------------------------------------------------------------

texts! {
    const crashes_none_listed { en: "No crash reports." }
    const crashes_none { en: "there are no crash reports" }
    const crashes_read_first { en: "Read this before you share it: it holds the panic message, which can quote the data that caused it." }
    const version_no_embedded_tmux { en: "embedded tmux: none (uses tmux from PATH, or --tmux-path)" }
}

messages! {
    fn crashes_unknown(name: &str) { en: "there is no crash report called {name}; `klaub crashes list` shows them" }
    fn version_embedded_tmux(version: &str, sha: &str) { en: "embedded tmux {version} (sha256 {sha})" }
}

// ---- klaub doctor -------------------------------------------------------------------------------

texts! {
    const doctor_git_missing { en: "`git` was not found in PATH; Klaub drives the git executable" }
    const doctor_unset { en: "unset" }
    const doctor_inside_tmux { en: "; inside tmux: `klaub wrap` is unavailable here, `klaub review` works" }
    const doctor_ssh { en: "this is an SSH session: best effort. Copy feedback goes to the clipboard of this host, not of your machine, so use Show file (or `klaub feedback`) to read it" }
    const doctor_wsl { en: "this is WSL: not a supported target. Klaub runs the Linux build here, but the terminal and the tmux server are on different sides of the VM, so pane identity and paste are unproven" }
    const doctor_wsl_mount { en: ". The current directory is on the Windows file system (/mnt/<drive>): git is slow there and paths differ from the Linux side; keep the repository inside the Linux file system" }
    const doctor_cache_none { en: "no cache yet (created by the first review)" }
    const doctor_crashes_none { en: "no crash reports" }
}

messages! {
    fn doctor_tmux_embedded(version: &str, sha: &str) { en: "embedded {version}, sha256 {sha} verified" }
    fn doctor_tmux_explicit(path: &str) { en: "{path} (explicit path, not verified)" }
    fn doctor_tmux_system(path: &str) { en: "{path} (found on PATH, not verified)" }
    fn doctor_tmux_version(detail: &str, version: &str) { en: "{detail}; version {version}" }
    fn doctor_tmux_optional(error: &str) { en: "{error}. `klaub review` works without it" }
    fn doctor_clipboard_used(program: &str) { en: "`{program}` will be used" }
    fn doctor_clipboard_missing(programs: &str) { en: "none of {programs} was found; Copy feedback will say so and Show file still works" }
    fn doctor_grammars_ok(count: usize) { en: "{count} languages parse a sample" }
    fn doctor_grammars_broken(languages: &str) { en: "did not parse a sample: {languages}" }
    fn doctor_cache_ok(entries: u64) { en: "{entries} entries, integrity ok" }
    fn doctor_cache_degraded(integrity: &str, schema: &str) { en: "integrity {integrity}, schema {schema}: the next review rebuilds it, nothing is lost" }
    fn doctor_cache_unreadable(error: &str) { en: "unreadable ({error}): the next review replaces it, nothing is lost" }
    fn doctor_crashes_some(count: usize) { en: "{count} saved on this machine; `klaub crashes list` shows them. Nothing was sent" }
    fn doctor_dir_created_on_use(shown: &str) { en: "{shown} (created on first use)" }
    fn doctor_sessions(running: usize, recorded: usize) { en: "{running} running, {recorded} recorded" }
}
