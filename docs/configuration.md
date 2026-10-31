# Configuration

Klaub reads two optional TOML files and merges them, the project's over the user's:

1. the user's `config.toml` in Klaub's config directory (`klaub doctor` prints the path), then
2. `.klaub.toml` at the root of the worktree.

Tables merge key by key. A scalar or an array in the project file replaces the user's value. An unknown key, a wrong type or a value out of range is an error that names the file, the line and the column. Nothing is ever written to either file by Klaub.

Every key has a default, so an empty file (or none) is valid. The schema lives in [`crates/klaub-core/src/config.rs`](../crates/klaub-core/src/config.rs); this page is the tour.

## `[review]`

| Key | Default | Meaning |
| --- | --- | --- |
| `base` | `"HEAD"` | The revision a review compares against. `klaub review --base` overrides it. |
| `default_scope` | `"worktree"` | `worktree`, `staged`, `unstaged` or `branch`. `klaub review --scope` overrides it. |
| `sort` | `"priority"` | How the queue is ordered: `priority`, `module` or `execution`. |
| `include_untracked` | `true` | Whether untracked, non-ignored files are part of the change. `.klaub/` never is. |
| `max_file_bytes` | `2097152` | Larger files are listed with metadata and not parsed. |

## `[ui]`, `[integration]`

| Key | Default | Meaning |
| --- | --- | --- |
| `ui.theme` | `"dark"` | `dark`, `light` or `high_contrast`. |
| `ui.show_hints` | `true` | The key hints in the status line. |
| `integration.submit_mode` | `"manual"` | The only mode. Klaub never presses `Enter` in the agent. |
| `integration.paste_requires_confirmation` | `true` | Putting text in the agent's input always asks first. |
| `integration.tmux_prefix` | `"C-b"` | The prefix of the session chords (`Prefix+R`, `Prefix+A`, `Prefix+I`, `Prefix+C`, `Prefix+T`, `Prefix+B`, `Prefix+L`, `Prefix+?`). |

## `[keys]`

Moves a reader action to another single character. An action that is moved gives up all of its default characters, so the help never lists a key that stopped working.

```toml
[keys]
toggle_reviewed = "e"
comment = "C"
```

The ids are:

`queue`, `structure`, `code`, `impact`, `comments`, `context`, `tests`, `followup`, `coverage`, `flow`, `next_unreviewed`, `previous_unreviewed`, `next_critical`, `toggle_reviewed`, `toggle_bookmark`, `next_bookmark`, `comment`, `explain`, `toggle_version`, `next_change`, `previous_change`, `toggle_code_scope`, `search`, `filter_high`, `filter_unreviewed`, `filter_introduced`, `filter_no_tests`, `filter_new`, `cycle_scope`, `cycle_module`, `clear_filters`, `history_back`, `history_forward`, `deeper_tree`, `shallower_tree`, `run_tests`, `reload_reports`, `reload_flow`, `refresh`, `finish`, `help`.

Some keys are deliberately not rebindable, because a terminal cannot be assumed to deliver anything else reliably: the arrow, page and function keys, `Esc`, `Enter`, `Tab`, the palette (`:`), quitting (`q`), movement (`j`, `k`, `h`, `l`) and the keys that mean something only inside one mode (the follow-up decisions, the contract inspector and the whole-file key, the flow request, line selection). A binding on one of those is refused. So is a value that is not exactly one printable character, an id that does not exist, and two actions on one character. The reader will not open with an invalid `[keys]` table; it says which entry is wrong.

## `[analysis]`

| Key | Default | Meaning |
| --- | --- | --- |
| `languages` | python, javascript, typescript, rust, go | Languages that get semantic analysis. Others are shown as plain files. |
| `max_call_depth` | `8` | How far the reverse call graph is followed. |
| `match_strong_threshold` | `0.85` | A symbol match at or above this is strong. |
| `match_probable_threshold` | `0.65` | At or above this it is probable; below it the match is left unresolved. |

## `[context]`

`brief_file` is a local Markdown file imported as the brief. It is read only when set (or passed with `klaub review --brief`), and a copy is kept in the review.

## `[tests]`

| Key | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Look for tests associated with changed symbols. |
| `run_mode` | `"manual"` | The only mode. Nothing runs without you. |
| `report_paths` | `[]` | JUnit XML and Coverage.py XML reports to read. |
| `python_patterns`, `js_patterns`, `rust_patterns`, `go_patterns` | per language | Where test files live. |
| `command` | unset | The test command, as an array (`["pytest", "-q"]`). Never inferred from the repository, never run through a shell. |
| `timeout_secs` | `600` | Time limit for a run. |

## `[architecture]`

```toml
[architecture]
modules = ["src/api", "src/domain", "src/infra"]

[[architecture.forbidden_dependencies]]
from = "src/domain"
to = "src/infra"
```

Without `modules`, packages and directories are used and the rules say they are guesses.

## `[priority]`

The weights of the explainable score. Every key is an integer and the review shows each term. The names are `contract_change`, `entrypoint_external`, `entrypoint_internal`, `fan_in`, `cross_module`, `global_scope`, `criticality`, `complexity`, `complexity_growth`, `risk_per_finding`, `risk_critical`, `risk_cap`, `no_tests`, `architecture_violation`, `format_only`, `test_file`, `generated` and `moved`, plus the thresholds `fan_in_threshold` and `complexity_threshold`.

## `[rules]`

`disabled` is a list of rule ids that are off, the way to silence a rule everywhere.

## `[coverage]`, `[flows]`

| Key | Default | Meaning |
| --- | --- | --- |
| `coverage.count_unit` | `"symbol"` | What the review progress counts: `symbol` or `file`. |
| `coverage.warn_on_unreviewed_p0_p1` | `true` | The non-blocking warning when finishing. |
| `flows.enabled` | `true` | Look for `.klaub/flows/<snapshot-id>.json`. |
| `flows.directory` | `".klaub/flows"` | Relative to the worktree. |
| `flows.max_file_bytes` | `1048576` | Larger flow files are rejected. |
| `flows.missing_prompt` | `"confirm"` | `confirm`: a missing diagram offers a request for the agent, prepared only after you confirm. `never`: no offer. |
