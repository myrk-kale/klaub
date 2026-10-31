# Development guide

## Prerequisites

- Rust **1.90** or newer (`rustup` recommended). The repository pins the *stable* channel in `rust-toolchain.toml`.
- `git` 2.30+ and `tmux` 3.2+ on `PATH`. Integration tests create throwaway Git repositories and talk to a real tmux server on a private socket; they skip with a message when `tmux` is missing.
- Optional: [`just`](https://github.com/casey/just), [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny), [`typos`](https://github.com/crate-ci/typos), [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov).

## Everyday commands

```shell
just ci          # fmt check, clippy, tests, docs, cargo-deny, typos
just test        # cargo test --workspace
just run review  # run the CLI from source
```

Without `just`, the equivalents are in the `justfile`; they are plain `cargo` invocations.

## Running with a system tmux

Development builds do not embed tmux. Point Klaub at one explicitly:

```shell
export KLAUB_TMUX_PATH="$(command -v tmux)"
cargo run -p klaub -- doctor
```

### Installing from crates.io

`cargo install klaub --locked` builds Klaub from the published crates. That build embeds no tmux, so Klaub uses the one you name with `--tmux-path` or `KLAUB_TMUX_PATH`, or else the first `tmux` (3.2 or newer) on `PATH`, and `klaub doctor` says which one and that it is not verified. `klaub review` does not need tmux at all; `klaub wrap` and `klaub session` do.

### Embedded tmux

Release builds embed a hash-verified tmux. To produce one locally, run `scripts/build-tmux.sh` and then `cargo build --release`. The script **downloads and compiles tmux, libevent and ncurses** from pinned, SHA-256-verified tarballs and writes `assets/tmux/<target>/`; CI does the same on disposable runners (`.github/workflows/tmux-asset.yml`), so you rarely need to. `klaub doctor` shows whether the binary in use is embedded and verified or an explicit path.

### Trying a wrapped session from source

```shell
export KLAUB_TMUX_PATH="$(command -v tmux)"
cargo build -p klaub
target/debug/klaub wrap -- sh        # any interactive program works as a stand-in agent
# Ctrl+B R  review window   Ctrl+B A  back to the agent   Ctrl+B d  detach
target/debug/klaub session list
target/debug/klaub session kill --yes <id>
```

Set `KLAUB_STATE_DIR`, `KLAUB_CACHE_DIR` and `KLAUB_CONFIG_DIR` to keep experiments out of your real directories. `KLAUB_LOG=debug` writes a log file for the review window.

## Conventions

- **Commits and PR titles** follow [Conventional Commits](https://www.conventionalcommits.org/): `feat(tui): ...`, `fix(git): ...`. Scopes are crate names without the `klaub-` prefix.
- **Branches**: `feat/...`, `fix/...`, `docs/...`, `chore/...`; merged through pull requests.
- **Errors**: library crates return `klaub_core::Result`; only `klaub-cli` uses `anyhow`.
- **No panics in library code**: `unwrap` and `expect` are linted outside tests. Prefer `?` and explicit error variants.
- **No `println!` in libraries**; use `tracing`. The CLI writes results to stdout and everything else to stderr. `--format json` prints JSON only on stdout.
- **Untrusted input**: file names, source code, comments and flow documents are untrusted. Strip terminal control sequences before rendering or pasting (`klaub_core::text`), validate paths with `RepoPath`, and pass subprocess arguments as arrays, never through a shell.
- **Documentation**: every public item has a doc comment; `cargo doc` runs with warnings denied.

## Adding a port or an adapter

1. Define the trait and the value types it exchanges in `klaub-core::ports`; keep it free of adapter vocabulary.
2. Add an in-memory fake next to the tests that need it.
3. Implement the adapter in its own crate, depending only on `klaub-core`.
4. Wire it in `klaub-cli`'s composition root.
5. Run `cargo test -p klaub --test architecture` to confirm the dependency rules still hold.

## Fixtures

Tests that need a repository build one on the fly with the helpers in `klaub-git`'s test support, so no `.git` directories are committed. Static source samples used by parser and matcher tests live under `fixtures/`.

## Checks that need a person

CI drives a real terminal multiplexer, real Git repositories and the real parser, but nothing in this repository runs Codex CLI, Claude Code, a particular terminal emulator or a real SSH connection. Before a release, someone with those installed should check:

- `klaub wrap codex` and `klaub wrap -- claude`: the agent starts, accepts keys, shows colours and follows a resize.
- Switching to the reviewer and back five times keeps the agent's scrollback and running task.
- Preparing feedback puts the one-line instruction in the agent's input, no `Enter` is sent, and it can be edited before submitting. This also holds while the input already has text and while the agent is answering.
- Detaching (`Prefix d`) and `klaub session attach` leaves both windows where they were.
- In your terminal (Ghostty, WezTerm, iTerm2, Kitty, Tabby): the reader draws, function keys arrive and bracketed paste works.
- With `[ui] hold_to_peek = true` in a terminal that supports the kitty keyboard protocol: holding `B` shows the old version until release, and accented or `AltGr` characters still type in a comment.
- With `[ui] mouse = true`: the wheel scrolls, and a click on a tab, a row and a code line does what it says.
- Over SSH: `klaub doctor` says so and the reader works ([ssh.md](ssh.md)).
- On a project of your own, the time to the first queue feels right ([benchmarks.md](benchmarks.md)).
- With the `klaub-flow` skill installed (`npx skills add myrk-kale/klaub --skill klaub-flow`): ask the agent for a diagram, wait, press `L`, and navigate it.

## Releases

Releases are cut from tags. See [RELEASING.md](../RELEASING.md).
