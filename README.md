# Klaub: a code reviewer for AI coding agents

Klaub is a free, open-source terminal tool for reviewing code that an AI coding agent wrote. It works with Codex CLI, Claude Code, Aider or any interactive program. It shows each change by function and class instead of by line, ranks the changes by risk, shows who calls them, and lets you send comments back to the agent. It runs offline, uses no LLM, and never presses Enter in your agent for you.

<p align="center">
  <a href="https://github.com/myrk-kale/klaub/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/myrk-kale/klaub/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/myrk-kale/klaub/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/myrk-kale/klaub"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/platforms-linux%20%7C%20macOS-lightgrey.svg">
</p>

<p align="center">
  <a href="README.pt-BR.md">Português</a> ·
  <a href="#quickstart">Quickstart</a> ·
  <a href="#keyboard-shortcuts">Shortcuts</a> ·
  <a href="#faq">FAQ</a> ·
  <a href="#resources">Docs</a>
</p>

<p align="center">
  <img src="docs/media/klaub-review.gif" alt="Klaub reviewing a change in the terminal: a ranked queue, the reason for each priority, the old and new code, the contract view, the callers up to a route, and a comment on the exact line" width="860">
</p>

## Quickstart

Install Klaub, go to a Git repository where an agent has changed files, and open the reviewer.

```shell
brew tap myrk-kale/klaub && brew install klaub      # macOS and Linux with Homebrew
# or
curl -fsSL https://raw.githubusercontent.com/myrk-kale/klaub/main/scripts/install.sh | sh
# or
cargo install --git https://github.com/myrk-kale/klaub klaub --locked   # any platform with a Rust toolchain

cd my-project
klaub review                                        # review what changed since HEAD
```

To keep the agent and the reviewer side by side, start the agent through Klaub:

```shell
klaub wrap codex                 # or: klaub wrap -- claude --model <model>
```

Klaub runs the agent in a session of its own with a reviewer window next to it. Press `Ctrl+B` then `R` to open the reviewer, `Ctrl+B` then `A` to go back to the agent, and `Ctrl+B` then `D` to leave the session running in the background. `klaub session attach` brings it back.

Your first review takes six steps:

1. Open the queue with `klaub review`. The riskiest change is first.
2. Press `w` to see why it has that priority.
3. Press `Enter` to read the whole function. Press `v` to flip between the old and the new version.
4. Press `i` for the contract and `4` for the callers and the routes that reach the change.
5. Press `c` to comment on a line, and `r` to mark a unit as reviewed.
6. Press `F` to finish. Klaub saves the comments and prepares one short instruction for the agent.

Press `?` at any time for the key list.

### Install the agent skills

Klaub ships two skills for your coding agent: `klaub-flow` writes a flow diagram of a change, and `klaub-feedback` makes the agent address the comments of a finished review. They install with the [skills CLI](https://github.com/vercel-labs/skills) and work with Claude Code, Codex and the other agents it supports.

```shell
npx skills add myrk-kale/klaub                     # choose the skills and the agents interactively
npx skills add myrk-kale/klaub --skill klaub-flow --agent claude-code codex
```

Installing with `cargo` builds Klaub without the bundled session runtime, so `klaub wrap` needs one to be installed on your `PATH`. The [development guide](docs/development.md) has the details, and `klaub review` works without it.

## Why Klaub

A line diff is a poor way to read what an agent wrote. A new file has no hunks, a rewritten function lines up badly, and nothing tells you which public contract changed or who depends on it. Klaub reads the code the way the language does, so the review follows the code's structure.

- You see functions, classes and methods, not lines. A new file is shown whole, and a rewritten function is shown in full with the old and the new version in the same place.
- The queue is ordered by evidence. Each priority from P0 to P3 comes from an itemised score, and each finding says whether the change introduced it, it was already there, or it was removed.
- You see the impact. Klaub follows the calls upward from the changed function to the routes, commands and `main` functions that can reach it, and labels every edge as resolved, heuristic or unresolved.
- Your feedback reaches the agent. Comments are saved in the repository as versioned JSON, and Klaub can put a one-line instruction in the agent's input. You press Enter.
- Everything is local. There is no account, no API key, no telemetry and no network access. The same code gives the same review every time.
- You stay in charge. Klaub never approves, rejects or fixes code. It helps you answer what changed, what it can affect, what evidence exists and what you have not looked at yet.

## What you get

### A review queue ranked by risk

Every changed function, class and file is a unit in the queue, ordered P0 to P3. Press `w` on a unit to see the points that put it there: a changed contract, an entrypoint that reaches it, a sensitive area such as `auth`, a risk finding, a missing test. A sensitive word alone never produces a P0.

Klaub classifies each symbol as added, removed, modified, moved, renamed, signature changed, rewritten or format only. It follows a function that moved to another file. Findings are marked introduced, pre-existing, removed or unknown, so you can tell new problems from old ones.

The risk findings Klaub looks for are swallowed errors, queries built from dynamic text, secrets written in code, I/O inside loops, dynamic code execution, shell execution, unsafe blocks and disabled certificate checks. It also checks architecture rules you define, such as forbidden dependencies and new import cycles. If a finding is a false positive, press `w`, then the finding's number, and Klaub silences that rule for the review.

### Old and new code in the same place

The current version fills the screen. Press `v` to swap it for the previous version in the same viewport, and `[` and `]` to jump between changed regions. Press `o` to open the unit inside its whole file. In a terminal that reports key releases, `[ui] hold_to_peek` lets you hold `B` to look at the old version and release it to come back.

### The contract of a change

Press `i` to see what a caller would notice: parameters and return type, decorators and annotations, the routes the symbol serves, the exceptions it raises or declares, its observable effects, and the public members of a type. Each is shown as added and removed lines. The same key works on migrations (SQL, Alembic, Django, Rails), dependency manifests and settings files, and it never shows secret values.

### Impact you can follow

Press `4` to see the callers of the changed function, the functions it calls, and the entrypoints that can reach it, with the path and the confidence of each hop. You can open any of them and pivot the view on it.

### Comments and the feedback loop

Comments are anchored to a symbol and a version, and they follow the symbol when lines move. If the commented code disappears, the comment is marked stale instead of landing on the wrong function. When you finish, Klaub freezes the comments into a feedback file and offers to prepare a single-line instruction in the agent's input, to copy it, or to show the file. In the next round the previous comments come back, and you decide for each one whether it was resolved, partly resolved, unresolved or a regression.

`klaub review --wait` lets an agent run the reviewer itself and wait for you. It exits with 0 when you finish the review and 3 when you close it as a draft, and with `--format json` it prints the review and nothing else.

### Tests, coverage and context

Klaub links changed symbols to the tests that probably exercise them and says how it knows (a reference, a name, a file). It reads JUnit and Coverage.py reports and tells you when they are older than the change. `X` runs your configured test command after showing it and asking. A coverage view tracks which units you have not reviewed yet. You can import a brief with `--brief FILE` and tick its acceptance criteria yourself.

### Flow diagrams

Ask the agent for a diagram of the change with `p` in the Flow view. The agent writes it as JSON with the packaged `klaub-flow` skill, Klaub validates it against the snapshot, and you navigate the steps down to the real code. A diagram that is stale, malformed or points outside the repository is refused.

### Exports and automation

`klaub reviews export` writes a review as JSON, Markdown or SARIF 2.1.0, so comments can go to code scanning tools. `klaub feedback` prints the feedback of a finished review. The `klaub-flow` and `klaub-feedback` skills install with `npx skills add myrk-kale/klaub`.

## Supported languages

| Language | Extensions | What Klaub understands |
| --- | --- | --- |
| Python | `.py`, `.pyi` | Symbols, signatures, decorators, calls, imports, routes (FastAPI and Flask style), test functions, risk findings |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` | Symbols, signatures, calls, imports and re-exports, Express style routes, risk findings |
| TypeScript | `.ts`, `.tsx`, `.mts`, `.cts` | The same as JavaScript, plus interfaces, types and enums |
| Rust | `.rs` | Functions, impls, traits, structs, enums, `use` paths, `Result` errors, `unsafe`, `#[test]` |
| Go | `.go` | Functions, methods, structs, interfaces, packages, `error` returns, routes, `main` |
| Java | `.java` | Classes, interfaces, enums, records, methods, annotations, `throws`, Spring and JAX-RS routes, JUnit tests |

Files in other languages are listed, shown whole and compared as text, and Klaub says that no semantic analysis was done for them. Adding a language is a documented, step by step change: see [adding a language](docs/adding-a-language.md).

## Keyboard shortcuts

Every key below can be changed in `.klaub.toml` under `[keys]` ([configuration](docs/configuration.md)). Press `?` in the reader for the same list, `:` or `Ctrl+P` for the command palette, and `q` twice to close.

### Modes

| Key | Mode |
| --- | --- |
| `1` or `F2` | Queue |
| `2` or `F3` | Structure: modules, files and symbols |
| `3` or `F4` | Code |
| `4`, `g` or `F5` | Impact: callers, entrypoints, callees |
| `5` or `F6` | Comments |
| `6`, `b` or `F8` | Context: brief and acceptance criteria |
| `7`, `t` or `F7` | Tests |
| `8` or `F10` | Follow-up: decide on the previous round's comments |
| `9` | Coverage |
| `0`, `f` or `F9` | Flow |
| `Tab`, `Shift+Tab` | Next and previous mode |
| `Esc` | Go back |

### Moving and reading

| Key | Action |
| --- | --- |
| `j`, `k` or arrows | Move |
| `h`, `l` | Scroll sideways in code |
| `Enter` | Open the selected unit, symbol or step |
| `v` | Toggle old and new |
| `B` (hold) | Show the previous version while held, in supported terminals |
| `[`, `]` | Previous and next changed region |
| `o` | Open the unit in its whole file |
| `S` | Switch between the symbol and the whole file |
| `i` | Inspect the contract |
| `<`, `>` | Back and forward through the units you visited |
| `/` | Search |
| `+`, `-` | Show more or less in the structure tree |

### Reviewing

| Key | Action |
| --- | --- |
| `c` | Comment on the selection (`V` selects several lines) |
| `w` | Why this priority, and silence a false positive |
| `r` | Mark reviewed or unmark |
| `m`, `,` | Bookmark a unit, jump to the next bookmark |
| `n`, `P` | Next and previous unreviewed unit |
| `N` | Next unreviewed P0 or P1 unit |
| `e`, `d` | In Comments: edit or delete a draft |
| `y`, `p`, `u`, `X` | In Follow-up: resolved, partial, unresolved, regression |
| `F` | Finish the review |
| `R` | Take a new snapshot after the worktree changed |

### Filters and tools

| Key | Action |
| --- | --- |
| `H` | P0 and P1 only |
| `u` | Unreviewed only |
| `I` | Introduced by this change |
| `T` | Units without tests |
| `W` | New files |
| `s` | Minimum reach: local, module or global |
| `M` | Next module |
| `x` | Clear filters |
| `X` | Run the associated tests (asks first) |
| `U` | Reload test reports |
| `L` | Reload the flow diagram |
| `p` | In Impact: pivot on a symbol. In Flow: ask the agent for a diagram |

### Around the agent

These work after `klaub wrap`. Press the prefix (`Ctrl+B` by default), release it, then the key.

| Key | Action |
| --- | --- |
| `R` | Open the reviewer |
| `A` | Back to the agent |
| `I`, `C`, `T`, `B`, `L` | Open the reviewer on impact, comments, tests, brief or flow |
| `D` | Detach and leave the session running |
| `?` | Print these chords |

An optional mouse mode (`[ui] mouse = true`) adds wheel scrolling and clicks on tabs, rows and code lines.

## Command reference

| Command | What it does |
| --- | --- |
| `klaub review` | Open the reviewer. Options: `--base REV`, `--scope worktree\|staged\|unstaged\|branch`, `--brief FILE`, `--wait`, `--format json` |
| `klaub wrap <agent>` | Run an agent with a reviewer next to it. `--detach` starts it in the background |
| `klaub session list\|attach\|kill` | Manage wrapped sessions |
| `klaub reviews list\|show\|export` | List stored reviews, print one, export as JSON, Markdown or SARIF |
| `klaub feedback ID` | Print the feedback of a finished review |
| `klaub tests list\|run` | List test associations, or run the configured command after confirming |
| `klaub flow status\|context\|validate\|prompt` | Work with flow diagrams |
| `klaub doctor` | Check Git, the terminal, grammars, the cache, directories and SSH |
| `klaub crashes list\|show` | Read crash reports saved on this machine. Nothing is ever sent |
| `klaub completions SHELL` | Print a shell completion script |

## FAQ

### What is Klaub?

Klaub is a terminal application that reviews code changes by function and class. It was built for reading what AI coding agents write, but it reviews any Git change.

### How do I review the code that Claude Code or Codex wrote?

Run `klaub wrap claude` or `klaub wrap codex` in your repository, let the agent work, then press `Ctrl+B` and `R`. You can also run `klaub review` in a second terminal at any time. When you finish, Klaub can prepare your comments in the agent's input so the agent can address them.

### Does Klaub send my code anywhere?

No. Klaub makes no network requests, has no telemetry and needs no account. Comments and reviews are files under `.klaub/` in your repository, and crash reports stay on your machine.

### Does Klaub use an LLM?

No. The analysis is deterministic: it parses your code, matches symbols across versions, builds a call graph and applies fixed rules. The same change gives the same queue, and every score can be explained.

### Will Klaub press Enter or submit anything in my agent?

Never. Putting text in the agent's input always shows you the destination and the exact text first, and it sends one line as a bracketed paste, so it cannot be read as a submission. You press Enter yourself.

### Which AI agents does it work with?

Codex CLI, Claude Code and Aider are the targets, and any interactive terminal program runs through `klaub wrap`. You can also use `klaub review` with no agent at all.

### Which programming languages does it support?

Python, JavaScript, TypeScript (including TSX), Rust, Go and Java get full semantic analysis. Other files are shown whole and compared as text. See [supported languages](#supported-languages).

### How is the priority of a change decided?

Klaub adds points for evidence: a changed public contract, an entrypoint that reaches the symbol, a sensitive area, a risk finding, growing complexity, missing tests. Press `w` to see every term. The weights live in `[priority]` and are described in [configuration](docs/configuration.md).

### How do I silence a false positive?

Open "Why this priority" with `w`, press the finding's number and confirm. The rule is silenced for that review and the queue is recomputed. "Bring back silenced rules" in the palette undoes it.

### Where are my comments stored?

In `.klaub/reviews/` at the root of the repository, as versioned JSON. Klaub does not edit `.gitignore` and does not commit anything. The first review of a repository tells you this once.

### Can I change the keys?

Yes. Every action, and every key that only works in one mode, can be moved to another single character under `[keys]`. The footer and the help show the keys you chose.

### How do I use Klaub in CI or with code scanning?

Run `klaub reviews export ID --format sarif`. Each comment becomes a result in a SARIF 2.1.0 log, with its priority in the properties.

### Does it work over SSH?

Yes, as a terminal program. The clipboard of your own machine is out of reach from a remote session, so use "Show file" or `klaub feedback` there. `klaub doctor` tells you when it detects SSH. See [using Klaub over SSH](docs/ssh.md).

### Does it run on Windows?

Not yet. Linux and macOS (Intel and Apple Silicon) are supported, and Windows is a possible future extension.

### How fast is it?

On a Django checkout with 7,000 files, opening a review of a 266 file change takes about 2 seconds the first time and 1 second with a warm cache. Projects of a few hundred files open in 0.3 to 1.1 seconds. A key press in a list takes well under 100 ms. [Benchmarks](docs/benchmarks.md) has the method and the limits of these numbers.

### How do I uninstall it?

Remove the binary (`brew uninstall klaub`, or delete `~/.local/bin/klaub`). If you want, also remove the `.klaub/` folders in your repositories and Klaub's own state and cache directories, which `klaub doctor` prints.

## Resources

| | |
| --- | --- |
| [Configuration](docs/configuration.md) | Every `.klaub.toml` section, and how to move keys |
| [Architecture](docs/architecture.md) | How Klaub is built: crates, ports, data flow, the terminal layer |
| [Security model](docs/security.md) | Threats, controls and explicit limits |
| [Using Klaub over SSH](docs/ssh.md) | What works and what the clipboard cannot do |
| [Benchmarks](docs/benchmarks.md) | Latency on synthetic and real repositories |
| [Adding a language](docs/adding-a-language.md) | Every place a new grammar touches |
| [Development guide](docs/development.md) | Building from source, tooling and conventions |
| [Changelog](CHANGELOG.md) | What changed in each release |
| [Releasing](RELEASING.md) | How a release is cut |

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and read the [Code of Conduct](CODE_OF_CONDUCT.md). Report security issues as described in [SECURITY.md](SECURITY.md), and ask questions through [SUPPORT.md](SUPPORT.md).

## License

[MIT](LICENSE). The third-party components that ship inside releases keep their own licenses, listed in `THIRD-PARTY-NOTICES.md` in every release archive.
