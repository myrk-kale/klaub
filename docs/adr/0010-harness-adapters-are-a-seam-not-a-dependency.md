# 10. Harness adapters are a seam, and nothing depends on them

- Status: accepted
- Date: 2026-10-31
- Refines: [ADR 0005](0005-human-confirmed-paste-never-enter.md)

## Context

The specification (§14.4) allows adapters for Codex, Claude Code or Aider that read a program's screen to tell whether it is ready for input or whether text was submitted, and requires the basic flow to work without them. It also keeps real adapters out of the first versions (§23, decision 25) until each harness has its own readiness contract and its own acceptance.

Until now that was only a sentence in the architecture page. Nothing in the code showed where such an adapter would plug in, so the first one would have had to invent the shape under pressure.

## Decision

`klaub_core::harness` defines `HarnessAdapter`: a pure object that is asked, given a program name or the tail of its screen, whether it recognises the program, whether the program looks ready or busy, and whether text was submitted. `AgentChannel::screen_tail` is the one new port method, with a default that returns nothing, and `HarnessRegistry` picks an adapter by the name of the foreground process.

The only adapter that ships is `GenericHarness`. It recognises nothing, so it is only the fallback, and it answers "unknown" to everything. The reader asks the registry before it opens the paste confirmation, names the adapter in that confirmation, and shows a warning if an adapter reports the program as busy.

An adapter can only inform. It cannot paste, press a key, decide that a paste is allowed, or mark feedback as submitted on its own. `Unknown` is never treated as `Ready`.

## Consequences

- The paste contract of ADR 0005 and ADR 0009 is unchanged. With no adapter registered the only visible difference is one line in the confirmation: `harness: generic`.
- A new adapter is one type that implements three small methods and one `with` call where the reader is built. It needs its own tests against recorded screens and its own acceptance entry before it ships.
- A test registers a fake adapter and checks that its warning reaches the confirmation, so the seam is exercised even though no real adapter exists.
- Nothing here claims that Klaub understands Codex or Claude Code. That stays a manual check ([acceptance](../acceptance.md)).
