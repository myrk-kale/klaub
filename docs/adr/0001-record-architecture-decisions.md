# 1. Record architecture decisions

- Status: accepted
- Date: 2026-10-31

## Context

Klaub makes several decisions whose reasons are easy to forget: which crates form the hexagon, why Git is driven through its CLI, why tmux is embedded and verified, why nothing is ever pasted with `Enter`. New contributors, and our future selves, need those reasons in the repository, next to the code they shape.

## Decision

We record every architecturally significant decision as a short Markdown file in `docs/adr/`, numbered sequentially, using the structure *Context, Decision, Consequences*. A decision is never rewritten after it is accepted. If it changes, a new ADR supersedes the old one and the old one's status becomes `superseded by NNNN`.

The reference product specification lives in [`docs/specification.pt-BR.md`](../specification.pt-BR.md). ADRs record where the implementation had to choose, refine or deviate from it.

## Consequences

- Reviewers can ask "is there an ADR for this?" when a pull request changes a boundary.
- The cost is a small amount of writing per decision, paid once.
