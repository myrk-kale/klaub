# 6. Reviews as versioned JSON files, cache outside the repository

- Status: accepted
- Date: 2026-10-31

## Context

Comments are the product of a review and must survive crashes, terminal closes and cache invalidation. Agents must be able to read them without Klaub. Parse results, on the other hand, are derived data that can always be recomputed.

## Decision

- The source of truth is one JSON document per review at `.klaub/reviews/<review-id>.json` with `schema_version: 1`. Writes go to a temporary file in the same directory, are flushed, renamed over the target and the directory is synced, under an advisory lock.
- Derived data (parse results) lives in a SQLite database in the user's private cache directory, keyed by content hash and parser version. Deleting it loses nothing.
- Klaub never edits `.gitignore` or `.git/info/exclude`, never commits `.klaub/`, and always excludes `.klaub/` from snapshots so writing a comment cannot create a new snapshot.
- Readers must ignore unknown fields and check `schema_version`.

## Consequences

- The file is the contract with the agent and with other tools (`klaub reviews show --format json` prints the same schema).
- Schema evolution needs explicit migrations and fixtures for old versions.
