# 7. Deterministic analysis with explicit confidence

- Status: accepted
- Date: 2026-10-31

## Context

Klaub is a reviewer for humans, not a reviewing agent. A priority or an impact claim that looks certain but is a guess misleads the reader more than a missing one.

## Decision

Every derived statement carries its origin: edges are `resolved`, `heuristic` or `unresolved`; symbol matches have a numeric confidence with the bands *strong* (≥ 0.85), *probable* (0.65–0.84) and *unresolved* (< 0.65); findings are `introduced`, `preexisting`, `removed` or `unknown` relative to the base. Missing data yields `unknown`, never "safe". Priorities P0–P3 are an ordering of review work backed by an itemised score; they are not verdicts. No part of the engine calls an LLM or the network.

## Consequences

- The UI has to show uncertainty (labels such as `probable`, `unknown`) instead of hiding it.
- Thresholds are versioned constants changed only with fixture or benchmark evidence.
