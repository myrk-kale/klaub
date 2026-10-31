## What and why

<!-- What does this change, and what problem does it solve? Link issues with "Closes #123". -->

## How it was tested

<!-- Unit tests, integration tests, manual steps. Paste relevant output when it helps. -->

## Checklist

- [ ] The PR title follows [Conventional Commits](https://www.conventionalcommits.org/) (`feat(tui): ...`).
- [ ] `just ci` passes locally (format, clippy, tests, docs).
- [ ] New behaviour has tests; bug fixes include a regression test.
- [ ] Public items are documented and `CHANGELOG.md` has an entry under *Unreleased*.
- [ ] The change respects the hexagonal dependency rules (see [docs/architecture.md](../docs/architecture.md)).
- [ ] Security-relevant input (paths, terminal output, subprocess arguments) is validated or sanitised.
