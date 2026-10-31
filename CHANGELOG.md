# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `klaub review --wait` stays until the reader is closed and reports how the review ended: exit 0 when it was finished, 3 when it was closed as a draft, with the review export on standard output under `--format json` (and nothing else there). An agent can run it and wait for the human; it needs a terminal like any review.
- The confirmation before preparing text in the agent now shows the process running in the pane and warns when it is a shell, so the text is not pasted onto a shell prompt by surprise.

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
- **Phase 0** (the gate): `klaub wrap <command>` runs any interactive program in a private tmux session with an `agent` and a `review` window; `klaub session list|attach|kill`, `klaub doctor`, `klaub version`, `klaub completions`.
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
- Fixture measurements for symbol matching, temporal classification, test association and anchor stability (`crates/klaub-app/tests/metrics.rs`), plus notes on what has not been measured (`docs/benchmarks.md`) and on Windows support (`docs/windows.md`).
- Release pipeline: `scripts/build-tmux.sh` (pinned tmux 3.8, libevent 2.1.13, ncurses 6.6), the `tmux asset` and `Release` workflows (checksums, build-provenance attestations, changelog-based notes), `scripts/install.sh`.
- ADRs 0008 (one private server per session) and 0009 (the pasted instruction is one line).
- `klaub-core`: the I/O-free domain model (snapshots, symbols, semantic changes, findings, reviews, test evidence, flow contract), the configuration schema with line/column errors and layered merging, terminal-text sanitisation, and the ports every adapter implements.
- Executable architecture rules (`crates/klaub-cli/tests/architecture.rs`): dependency direction between layers and an I/O-free core are enforced in CI.
- Project foundation: Cargo workspace, lint policy, continuous integration, security scanning, community files, architecture documentation and decision records.
