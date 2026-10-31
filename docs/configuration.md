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
| `ui.mouse` | `false` | The wheel scrolls, a click on a tab switches to it, a click on a row selects it and a second click opens it. Off by default because capturing the mouse stops the terminal's own text selection until you hold the modifier it offers (usually `Shift`, or `Option` in iTerm2). The mouse only does what a key already does. |
| `ui.hold_to_peek` | `false` | Hold `B` (the `peek_previous` key) while reading code to see the previous version, and let go to come back. It needs a terminal that reports key releases (the kitty keyboard protocol: kitty, Ghostty, WezTerm, recent iTerm2 and foot). Where that is missing, which includes tmux, the key says so and `v` still toggles. While it is on, the terminal sends keys as escape codes except inside text fields, which keep ordinary input so compose and `AltGr` characters still type. |
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

`queue`, `structure`, `code`, `impact`, `comments`, `context`, `tests`, `followup`, `coverage`, `flow`, `next_unreviewed`, `previous_unreviewed`, `next_critical`, `toggle_reviewed`, `toggle_bookmark`, `next_bookmark`, `comment`, `explain`, `toggle_version`, `peek_previous`, `next_change`, `previous_change`, `toggle_code_scope`, `search`, `filter_high`, `filter_unreviewed`, `filter_introduced`, `filter_no_tests`, `filter_new`, `cycle_scope`, `cycle_module`, `clear_filters`, `history_back`, `history_forward`, `deeper_tree`, `shallower_tree`, `run_tests`, `reload_reports`, `reload_flow`, `refresh`, `finish`, `help`.

The keys that only mean something inside some modes move the same way. A moved key answers to its new character in the modes it works in, and its old character stops acting there:

| Id | Default | Works in |
| --- | --- | --- |
| `down`, `up` | `j`, `k` | every list, scrolling view and the delivery list |
| `scroll_left`, `scroll_right` | `h`, `l` | Code |
| `select_lines` | `V` | Code |
| `open_file` | `o` | Code |
| `inspect_contract` | `i` | Code |
| `pivot_impact` | `p` | Impact |
| `request_flow` | `p` | Flow |
| `edit_intent`, `add_criterion` | `i`, `a` | Context |
| `edit_comment`, `delete_comment` | `e`, `d` | Comments |
| `decide_resolved`, `decide_partial`, `decide_unresolved`, `decide_regression` | `y`, `p`, `u`, `X` | Follow-up |

```toml
[keys]
down = "z"
up = "Z"
decide_resolved = "Y"
```

A mode-specific key may take a letter that another mode uses, but not one that is live in the same mode, and not one a global action already has. The footer and the help show the keys in use.

Some keys are deliberately not rebindable, because a terminal cannot be assumed to deliver anything else reliably: the arrow, page and function keys, `Esc`, `Enter`, `Tab`, the palette (`:`), quitting (`q`), `Space` and the confirmation keys (`y`/`n` inside a question). A binding on one of those is refused. So is a value that is not exactly one printable character, an id that does not exist, and two keys that would be live on one character. The reader will not open with an invalid `[keys]` table; it says which entry is wrong.

## `[analysis]`

| Key | Default | Meaning |
| --- | --- | --- |
| `languages` | python, javascript, typescript, rust, go, java | Languages that get semantic analysis. Others are shown as plain files. |
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
| `python_patterns`, `js_patterns`, `rust_patterns`, `go_patterns`, `java_patterns` | per language | Where test files live. Java's default is `**/src/test/**/*.java`, `**/*Test.java`, `**/*Tests.java` and `**/*IT.java`. |
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
