<h1 align="center">Klaub</h1>

<p align="center">
  <strong>A semantic code reviewer for humans that wraps any coding agent.</strong><br>
  Read the real code, find what deserves attention, and hand precise feedback back to the agent, without leaving the terminal or losing the session.
</p>

<p align="center">
  <a href="https://github.com/myrk-kale/klaub/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/myrk-kale/klaub/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <img alt="MSRV 1.90" src="https://img.shields.io/badge/MSRV-1.90-orange.svg">
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-linux%20%7C%20macOS-lightgrey.svg">
  <a href="CONTRIBUTING.md"><img alt="PRs welcome" src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg"></a>
</p>

<p align="center">
  <a href="README.pt-BR.md">Português</a> ·
  <a href="docs/architecture.md">Architecture</a> ·
  <a href="docs/specification.pt-BR.md">Specification</a> ·
  <a href="CONTRIBUTING.md">Contributing</a>
</p>

---

> **Status: under active development.** Phase 0 and phase 1 are implemented. The semantic reader has a queue, structure, code with before and after, impact, comments, context, tests, follow-up, coverage and flow. Phase 2 and phase 3 are not done. A manual check against a real Codex or Claude Code session is still pending. The first tagged release is v0.1.0. See the [changelog](CHANGELOG.md).

## Why Klaub

Reviewing what a coding agent wrote is slow with a line diff: new files have no hunks, rewrites misalign, side-by-side views waste width, and nothing tells you *what contract changed* or *who calls it*. Klaub is built for that moment.

- **Semantic, not line-based.** Navigate by module, file, class, function and block. A new file is shown whole; a rewritten function is shown in full, `before` and `after` in the same viewport.
- **A review queue, not an alphabetical list.** P0–P3 priorities come from itemised, explainable evidence, and every finding says whether it was *introduced*, *preexisting*, *removed* or *unknown*.
- **Impact you can follow.** A reverse call graph from the changed symbol up to the entrypoints that can reach it, with the confidence of every edge (`resolved`, `heuristic`, `unresolved`).
- **Same terminal, same session.** `klaub wrap codex` (or `claude`, or any interactive program) runs the agent in a private tmux session; one key chord flips between the agent and the reviewer.
- **Feedback that lands where it belongs.** Comments are saved as a versioned JSON file in the repository. Klaub can *prepare* a short instruction in the agent's input. It never presses `Enter` for you.
- **Offline, deterministic, no LLM.** No account, no API key, no telemetry, no network.
- **Human in charge.** Klaub never approves, rejects, certifies or fixes code. It helps you answer: *what changed, why, what can be affected, what evidence exists, and what have I not inspected yet?*

## How it works

```mermaid
flowchart LR
    You([You]) --> Wrap["klaub wrap codex"]
    Wrap --> Tmux["private tmux session"]
    Tmux --> Agent["window: agent<br/>(Codex, Claude Code, ...)"]
    Tmux --> Review["window: review<br/>(Klaub TUI)"]
    Review --> Engine["Git snapshot → AST → semantic diff<br/>→ call graph → rules → queue"]
    Review --> File[(".klaub/reviews/*.json")]
    File -. "Prepare in agent<br/>(paste, no Enter)" .-> Agent
```

## Install

The first tagged release is v0.1.0. Prebuilt binaries exist for Linux x86_64 and macOS Apple Silicon. Intel macOS is not in the release matrix. On an Intel Mac, build from source.

### Homebrew

Works on macOS and on Linux where Homebrew is installed. The formula lives in the `myrk-kale/homebrew-klaub` tap. There is no prebuilt bottle for Intel macOS. Those machines build from source.

```shell
brew tap myrk-kale/klaub
brew install klaub
```

### curl

macOS and Linux. The script checks the SHA-256 of the archive before it installs anything. It supports Linux x86_64 and macOS Apple Silicon only.

`KLAUB_VERSION` picks a version (`0.1.0`). `KLAUB_INSTALL_DIR` picks the directory (default `~/.local/bin`).

```shell
curl -fsSL https://raw.githubusercontent.com/myrk-kale/klaub/main/scripts/install.sh | sh
```

### From source

Needs Rust 1.90+, git, and a system tmux 3.2 or newer on `PATH` until the release embeds one. Point `KLAUB_TMUX_PATH` at that tmux.

```shell
git clone https://github.com/myrk-kale/klaub && cd klaub
cargo install --path crates/klaub-cli --locked
export KLAUB_TMUX_PATH="$(command -v tmux)"
klaub doctor
```

## Quick start

```shell
cd my-project
klaub wrap codex                 # or: klaub wrap -- claude --model <model>
# Ctrl+B, R   open the reviewer     Ctrl+B, A   back to the agent     Ctrl+B, d   detach
# Ctrl+B, I   impact   C   comments   T   tests   B   brief   L   flow
klaub session list               # later: klaub session attach
```

The prefix is `Ctrl+B` unless you set `--prefix` or `KLAUB_TMUX_PREFIX`. `Prefix+?` prints the chords. `Prefix+R` and `Prefix+A` switch windows. `Prefix+I`, `C`, `T`, `B` and `L` open the reviewer and send `F5` through `F9` to it (impact, comments, tests, brief, flow).

`klaub review` opens the semantic reader, including from inside the wrap. A review of the worktree is the default.

Other commands:

- `klaub review --base REV --scope worktree|staged|unstaged|branch --brief FILE` compares against a revision, a scope, or a local brief file.
- `klaub review --wait [--format json]` stays until the reader is closed and says how it ended: exit 0 for a finished review, 3 for a draft. With `--format json` standard output is the review export and nothing else. It is for an agent that runs the reviewer and waits for the human, and it needs a terminal like any review.
- `klaub reviews list` lists stored reviews. `show ID` prints one. `export ID` writes JSON or Markdown.
- `klaub feedback ID` prints the feedback of a finished review.
- `klaub tests list --review ID` lists test associations. `run --review ID` shows the command and asks before running it.
- `klaub flow status` reports whether the snapshot has a flow diagram. `context` prints the snapshot context as JSON. `validate FILE` checks a flow document. `prompt` prints the instruction that asks an agent for one, and sends nothing.
- `klaub skills install flow|feedback --harness codex|claude` copies a packaged skill into that agent's configuration.

## Documentation

| | |
| --- | --- |
| [Architecture](docs/architecture.md) | Hexagonal layout, crates, ports, data flow |
| [Architecture decisions](docs/adr/) | Why things are the way they are |
| [Security model](docs/security.md) | Threats, controls, explicit limits |
| [Development guide](docs/development.md) | Tooling, conventions, adding adapters |
| [Configuration](docs/configuration.md) | Every `.klaub.toml` section, and how to move the reader's keys |
| [Specification](docs/specification.pt-BR.md) | Product behaviour and acceptance criteria (Portuguese) |
| [Acceptance](docs/acceptance.md) | Which acceptance checks CI runs and which need a person with a real agent |
| [Benchmarks](docs/benchmarks.md) | Latency against the specification's targets, and what the numbers do not say |
| [Using Klaub over SSH](docs/ssh.md) | What works, and what the clipboard cannot do |
| [Adding a language](docs/adding-a-language.md) | Every place a new tree-sitter grammar touches |

## Roadmap

| Phase | Scope | Status |
| --- | --- | --- |
| Foundation | Workspace, lints, CI, community files, ADRs | ✅ |
| 0 | Embedded tmux, `wrap`, pane identity, safe bracketed paste, minimal TUI ([gate](docs/phase-0-gate.md)) | ✅ implemented; manual check with real harnesses pending |
| 1 | Git snapshots, Python and JS/TS, semantic matching, queue, reverse call graph, comments, brief, tests evidence, incremental risk | ✅ implemented |
| 2 | Architecture rules, contract inspector, follow-up, review coverage, `Flow` and the `klaub-flow` skill | ✅ implemented, including contracts for migrations, manifests and settings, silencing a false positive, a foldable flow tree with breadcrumbs, configurable keys and `review --wait` |
| 3 | More languages, general feedback skill, export formats, harness adapter seam | ✅ Rust, Go and Java beyond the specification's list, the `klaub-feedback` skill, JSON, Markdown and SARIF exports, [a guide to adding a language](docs/adding-a-language.md), and the [`HarnessAdapter` seam](docs/adr/0010-harness-adapters-are-a-seam-not-a-dependency.md) with only the generic adapter behind it. |
| Future | Windows and WSL | Not scheduled. Today Klaub supports Linux and macOS; [docs/windows.md](docs/windows.md) lists what would have to be proven first. |

Done by tests is not the same as done on your machine. The checks the specification makes against real Codex CLI and Claude Code sessions, real terminals and SSH are listed in [docs/acceptance.md](docs/acceptance.md) as manual, and none of them has been run by an automated test.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md); please read the [Code of Conduct](CODE_OF_CONDUCT.md). Security issues go through [SECURITY.md](SECURITY.md).

## Licence

[MIT](LICENSE). The embedded tmux, libevent and ncurses keep their own licences, shipped with every release in `THIRD-PARTY-NOTICES.md`.
