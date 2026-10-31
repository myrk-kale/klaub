# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-10-31

### Added

- **Install with cargo.** The binary package is now called `klaub`, so `cargo install --git https://github.com/myrk-kale/klaub klaub --locked` works. A build without the bundled tmux uses the one on `PATH` and says so in `klaub doctor`. CI packages the whole workspace on every pull request, and a Trusted Publishing workflow for crates.io is in place; the crates are not published yet.
- The two skills install with the Vercel skills CLI: `npx skills add myrk-kale/klaub`. CI checks that the CLI finds both.

### Changed

- The `klaub-cli` package is now `klaub` (the library keeps the name `klaub_cli`). Use `-p klaub` where you used `-p klaub-cli`.
- The crate keywords no longer name the implementation.

### Removed

- `klaub skills install` and its `--harness` and `--force` options. Use `npx skills add myrk-kale/klaub`, which also covers agents beyond Codex and Claude Code.

### Fixed

- The `klaub-feedback` skill named a command that does not exist (`klaub feedback show`); it is `klaub feedback <review-id>`.

## [0.2.0] - 2026-10-31

### Added

- **Java.** Classes, interfaces, enums, records, methods and constructors with their annotations (kept as decorators), signatures without the annotations, visibility (package-private is `Internal`), `throw new X` and `throws`, calls with receivers, imports (classes, packages and static members), `public static void main`, Spring (`@GetMapping`, `@RequestMapping` with the class prefix and verb) and JAX-RS (`@GET` with `@Path`) routes, JUnit `@Test` methods, and the same risk markers as the other languages (swallowed `catch`, SQL built by concatenation or `String.format`, a shell started by `ProcessBuilder` or `Runtime.exec`, a secret in a string literal, I/O in a loop). Imports resolve to files under any source root. Test files are `src/test/**/*.java`, `*Test.java`, `*Tests.java` and `*IT.java` (`tests.java_patterns`). Reflection, dependency injection and code generation are not followed, so calls through them stay unresolved. Parser version 3.
- **Every mode-specific key is configurable.** `[keys]` also moves movement (`down`, `up`, `scroll_left`, `scroll_right`), `select_lines`, `open_file`, `inspect_contract`, `pivot_impact`, `request_flow`, the Context and Comments edit keys and the four follow-up decisions. A moved key works in the modes it belongs to and its old character stops acting there. The footer is built from the keymap, so it no longer names keys you moved.
- **Optional mouse** (`[ui] mouse = true`): the wheel scrolls, a click on a tab switches to it, a click on a row selects it, a second click opens it, a click on a code line moves the cursor. It only does what a key already does.
- **Hold to see the previous version** (`[ui] hold_to_peek = true`, `B`), where the terminal reports key releases (kitty keyboard protocol). Elsewhere, inside tmux included, the key says it cannot be held and `v` still toggles. The protocol is dropped while a text field is open so compose and `AltGr` characters still type.
- **A `HarnessAdapter` seam** (`klaub_core::harness`) with `GenericHarness` as the only adapter. The paste confirmation names the adapter and shows a busy warning if an adapter reports one. The paste contract is unchanged, and no Codex or Claude Code adapter ships.
- **Linux aarch64 and macOS Intel** builds join Linux x86_64 and macOS Apple Silicon in the release, the install script and the Homebrew formula. The macOS Intel build uses the system memory allocator, because a build with jemalloc crashes in the forked child on that platform. Source downloads in the tmux build fall back to mirrors, and every source is checked against its SHA-256.
- `klaub doctor` warns under WSL, and when the directory is on the Windows file system.
- `KLAUB_BENCH_REPO` runs the latency harness on real repositories; `docs/benchmarks.md` has a run on flask, django, tokio and vue core.
- **Contract inspector (`i`).** The declaration of a callable is split into parameters and return type for Python, JavaScript/TypeScript, Rust, Go and Java (where `throws` is listed with the exceptions). Decorators, the routes a symbol serves, the exceptions its body raises or throws (and the error a Rust `Result` or a Go `error` declares), observable effects (risk markers and I/O-looking calls) and, for a type, its public members and what kind of data type it looks like are shown as `+`/`-` lists before and after. The entrypoints that can reach the symbol are named with their certainty. A declaration it cannot read is reported as unknown.
- **Contracts for files without symbols.** The same key opens a contract for migrations (SQL, Alembic, Django, Rails), dependency manifests (`Cargo.toml`, `package.json`, `pyproject.toml`, `requirements*.txt`, `go.mod`) and settings files (JSON, TOML, YAML, `.env`): what was added, removed or changed. Secret values and every `.env` value are never shown. Anything it cannot read falls back to the textual diff and says so.
- `klaub-parse` records the exceptions a body raises and the I/O-looking calls it makes (parser version 2, so cached parses are refreshed).
- **Configurable keys.** `[keys]` moves any global reader action to another single character. The table is validated as a whole (unknown action, not one printable character, reserved key, two actions on one key) and the reader refuses to open with an invalid one. The help and the palette show the configured keys. See `docs/configuration.md`, which also documents every other configuration section.
- **A first-review notice.** The first review of a worktree with no `.klaub/` explains that the folder is local data, that Klaub does not add it to `.gitignore` or `.git/info/exclude`, never commits it, and always leaves it out of the changes it reviews.
- **False positives.** A finding can be silenced for one review from "Why this priority" (`w`): press its number, confirm, and its rule is silenced. The queue is recomputed on the same snapshot (the worktree is not read again), the choice is saved in the review and carried into the next round, and "Bring back silenced rules" in the palette undoes it. Only a rule that produced a finding in the review can be silenced.
- **Flow as a tree.** The Flow view indents branches under the step they leave, marks a step reached twice as `↺ shown above`, shows how each row was reached (`calls ◐`), folds and unfolds branches with `←`, `→` and `Space`, and shows a `path:` of breadcrumbs from the first step to the selected one. `Enter` opens the selected step's real symbol whatever is folded.
- **History.** `<` and `>` walk back and forward through the units visited, as in a browser. A new move forgets what was ahead; the history holds the last 64 units.
- **`klaub review --wait`** stays until the reader is closed and reports how the review ended: exit 0 when finished, 3 when closed as a draft, with the review export on standard output under `--format json` and nothing else there.
- **`klaub reviews export --format sarif`**: a SARIF 2.1.0 log, one result per comment. The level is a hint about attention (P0 `error`, P1 `warning`, the rest `note`), not a defect classification; the exact priority travels in `properties`.
- **`klaub doctor`** also proves every grammar parses a sample, inspects the parse cache read-only (a damaged one is reported and left alone), says which clipboard program Copy would use, notes an SSH session and what it means for the clipboard, and counts saved crash reports. See `docs/ssh.md`.
- **Local crash reports.** If Klaub panics it saves a short report (version, platform, subcommand name, a bounded panic message and the place in Klaub's source, with the home directory as `~`) under its state directory. `klaub crashes list` and `klaub crashes show` read them. Nothing is sent anywhere.
- Latency benchmarks for the project's targets: `cargo bench -p klaub-app --bench latency` and `cargo bench -p klaub-tui --bench ui`, with one recorded run and its caveats in `docs/benchmarks.md`.
- `fixtures/` with flow documents (one current, one stale, one with a missing symbol, four that must be refused) and JUnit and Coverage.py reports, each read by a test; and `docs/adding-a-language.md`.

### Changed

- `SnapshotProvider::read_project_files` reads many project files at once, and the Git adapter answers with one `git cat-file --batch`. Reviewing a branch or a commit range no longer starts a process per project file.
- Test association checks a file's imports once per file instead of once per test case.
- `o` now opens the current unit in its whole file and stays there, where `S` still toggles between the symbol and the file. The contract inspector, which had taken `o`, is now `i`.
- The confirmation before preparing text in the agent shows the process running in the pane and warns when it is a shell, so the text is not pasted onto a shell prompt by surprise.
- The `Security` workflow runs on every pull request instead of only those that touch a manifest, so its checks can be required on `main`.

### Fixed

- A symbol that moved from one file to another made the analysis read the wrong file's symbol table. On a real project (tokio) it panicked with an out-of-bounds index; on a small one it quietly showed the contract and the old code of the wrong symbol. Each side of a symbol is now read from the file it names, and the old code of a moved symbol is shown from the file it left.

## [0.1.0] - 2026-10-31

### Added

- `klaub-storage`: atomic, advisory-locked JSON reviews (symlinked `.klaub/` refused), a SQLite parse cache that heals itself, confined readers for briefs, reports and flows, and layered configuration loading.
- `klaub-app`: the analysis use case (parallel, cached parsing; matching; call graph; test evidence; rules; queue) and `ReviewSession` (comments with anchors, reviewed marks, brief, coverage, finishing, delivery to the agent only after confirmation, drift detection and refresh, next rounds with relocated comments, test panel, flow state, exports), plus a code view with change marks and syntax colours.
- `klaub-graph`: call and dependency graph with per-edge origin (`resolved`/`heuristic`), ambiguity counted instead of guessed, entrypoints from the parser and from `pyproject.toml`/`package.json`, reverse impact with entrypoint paths, scope and coverage, and module cycles.
- `klaub-rules`: explainable findings reconciled into `introduced`/`preexisting`/`removed`/`unknown`, an itemised score and the P0–P3 queue, where `P0` needs critical evidence; architecture rules (forbidden dependencies, new cycles).
- `klaub-review`: comments, anchors with relocation and stale detection, finishing into an idempotent feedback payload, human-confirmed delivery states, follow-up decisions with history, JSON and Markdown exports.
- `klaub-context`: brief import (never ticks criteria for you) and human review coverage with non-blocking warnings.
- `klaub-tests`: heuristic test association with method and confidence, JUnit and Coverage.py report import with staleness, honest evidence status, and a no-shell run request built only from configuration.
- `klaub-flow`: strict, bounded validation of agent-written flow documents (paths, terminal sequences, snapshot freshness, divergences from the index), request prompt and an indented layout for the TUI.
- `klaub-parse`: tree-sitter extraction of symbols, calls, imports, entrypoints and risk markers for Python, JavaScript, TypeScript/TSX, Rust and Go, behind the `SourceParser` port. Fault tolerant and depth-bounded; nested symbols never change the fingerprint of the symbol that contains them.
- `klaub-match`: temporal symbol matching with confidence and evidence (identity, similarity within a file, moves across files) that classifies each symbol as added, removed, modified, moved, renamed, signature-changed, rewritten or format-only, explains near misses it refuses to call moves, and computes the per-line diff for the reader.
- `klaub-git`: immutable, content-addressed snapshots through the Git CLI for the worktree, staged, unstaged, branch and commit-range scopes; rename detection, untracked and ignored-file handling, binary/oversized/symlink metadata, generated-file flags, a cheap drift probe, and project file listing and reading for call-graph building. Repositories are treated as untrusted: external diff drivers, textconv and fsmonitor never run, option-like revisions are rejected and symlinks cannot lead out of the worktree.
- `klaub wrap <command>` runs any interactive program in a private tmux session with an `agent` and a `review` window; `klaub session list|attach|kill`, `klaub doctor`, `klaub version`, `klaub completions`.
- `klaub-tmux`: verified embedded tmux (SHA-256 checked before every execution, private `0700` extraction), one private server per session, pane-identity checks, and a bracketed, single-line, never-`Enter` paste. Nine integration tests run against a real tmux server and assert the exact bytes the harness receives.
- `klaub-tui`: the fullscreen semantic reader (Elm-style state, input and view, drawn with Ratatui). Ten modes over the review queue, a command palette and help generated from one key table, and overlays for commenting, confirming and finishing. Every string that comes from the repository, a comment or a document is sanitised before it is drawn, and a terminal below 50×10 is told to resize instead of rendering a broken frame.
- `klaub review` now opens that reader over the worktree: it wires the Git, tree-sitter and storage adapters, resumes an existing review, and inside `klaub wrap` can prepare feedback in the agent's input after confirmation. On its own it can only copy or show the feedback. Running the configured tests goes through a shell-less process adapter with a time limit. `--base`, `--scope` and `--brief` choose the comparison and import a local brief.
- `klaub reviews list|show|export`, `klaub feedback`, `klaub tests list|run`, `klaub flow status|context|validate|prompt` and `klaub skills install`. `tests run` shows the exact command and asks before it starts. `flow prompt` only prints text. Installing a skill refuses to replace one the user has edited unless `--force` is given.
- tmux session chords `Prefix+I`, `Prefix+C`, `Prefix+T`, `Prefix+B` and `Prefix+L` open the reviewer directly on impact, comments, tests, context and flow.
- Packaged skills `klaub-flow` and `klaub-feedback`, installed on request into the Codex or Claude Code skills directory.
- Render tests for the reader: queue order, before/after, comments, reviewed marks, hostile terminal sequences, a too-small terminal, finishing without ever sending Enter, the drift banner, and the contract panel.
- A contract inspector (`o` in the code view) showing, for one changed symbol, the change kind, the confidence, the signature before and after, the effects named by markers the symbol already carries, and the direct callers. What the analysis cannot know is stated as unknown rather than guessed.
- Search also matches comment ids, test ids and acceptance-criterion text. `m` bookmarks a unit and `,` jumps to the next bookmark. Reopening a review returns to the unit you were on.
- The execution sort order ranks a change reached from more entrypoints first.
- Fixture measurements for symbol matching, temporal classification, test association and anchor stability (`crates/klaub-app/tests/metrics.rs`), plus notes on what has not been measured (`docs/benchmarks.md`).
- Release pipeline: `scripts/build-tmux.sh` (pinned tmux 3.8, libevent 2.1.13, ncurses 6.6), the `tmux asset` and `Release` workflows (checksums, build-provenance attestations, changelog-based notes), `scripts/install.sh`.
- `klaub-core`: the I/O-free domain model (snapshots, symbols, semantic changes, findings, reviews, test evidence, flow contract), the configuration schema with line/column errors and layered merging, terminal-text sanitisation, and the ports every adapter implements.
- Executable architecture rules (`crates/klaub-cli/tests/architecture.rs`): dependency direction between layers and an I/O-free core are enforced in CI.
- Project foundation: Cargo workspace, lint policy, continuous integration, security scanning, community files, architecture documentation and decision records.
