# Contributing to Klaub

Thank you for considering a contribution. Klaub is a code reviewer for humans, and the project tries to be a pleasant place for humans to work on, too.

By participating you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to help

- **Try it and report what hurts.** Open an issue with the output of `klaub doctor`.
- **Add fixtures.** A small repository with a tricky change (a rename that Git misses, a rewritten function, an odd decorator) is worth more than a long description.
- **Improve a language adapter or a rule.** Both are evidence-driven; see below.
- **Fix documentation.** Typos, unclear steps, missing screenshots.

## Before you start

For anything larger than a fix, open an issue or a discussion first so we can agree on the approach. Read:

- [docs/architecture.md](docs/architecture.md) for the hexagonal layout and the dependency rules,
- [docs/development.md](docs/development.md) for tooling and conventions,
- the [ADRs](docs/adr/) for why things are the way they are,
- the [specification](docs/specification.pt-BR.md) (Portuguese) for product behaviour.

## Development workflow

1. Fork and create a branch: `feat/<topic>`, `fix/<topic>`, `docs/<topic>`.
2. Make focused commits using [Conventional Commits](https://www.conventionalcommits.org/).
3. Run `just ci` (or the `cargo` commands in the `justfile`).
4. Add an entry under *Unreleased* in [CHANGELOG.md](CHANGELOG.md).
5. Open a pull request. The template lists what reviewers look for.

### Principles that review decisions follow

1. **The human decides.** Nothing approves, rejects, submits or fixes automatically.
2. **Uncertainty is shown, not hidden.** Add a confidence or an `unknown` rather than guessing.
3. **Offline and deterministic.** No network, no LLM in the engine.
4. **Untrusted input.** Source code, file names, comments and flow documents can contain terminal escape sequences or hostile paths.
5. **The core stays pure.** If a change needs I/O in `klaub-core` or a domain-service crate, it belongs behind a port.

### Changing a rule or a threshold

Rules and matching thresholds are versioned constants. A change needs a fixture that demonstrates the false positive or negative it fixes, and the pull request description should state the effect on the existing fixtures.

### Tests

- Unit tests live beside the code; integration tests in each crate's `tests/`.
- Tests must not touch the real `~/.config`, the user's tmux server or the network. Use temporary directories and the private tmux socket helpers.
- UI changes include a render test at a small and a large terminal size.

## Reporting security issues

Do **not** open a public issue. Follow [SECURITY.md](SECURITY.md).

## Licence

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
