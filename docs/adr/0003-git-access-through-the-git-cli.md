# 3. Git access through the structured Git CLI

- Status: accepted
- Date: 2026-10-31

## Context

The specification names `gix` as the primary Git library and the Git CLI as a fallback "where `gix` coverage requires it". Klaub must capture a consolidated working tree snapshot: `HEAD` against index plus working files plus untracked, non-ignored files, with rename detection, merge-base resolution and the user's ignore rules. At the time of writing, `gix` does not offer a stable, complete equivalent of `git diff HEAD -M` combined with `git ls-files --others --exclude-standard` for this use. Reimplementing it would put correctness at risk for no user-visible gain.

## Decision

The `klaub-git` adapter runs the `git` executable with argument arrays (never a shell), `-z` output, and a scrubbed environment (`LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, no pager). It sits behind the `SnapshotProvider` port. A `gix`-based adapter can later implement the same port, one operation at a time, without changes elsewhere.

## Consequences

- Behaviour matches what users see from `git diff` and `git status`, including configured ignore rules.
- `git` must be on `PATH`; `klaub doctor` checks for it and reports its version.
- Process start-up cost is paid per Git call. Calls are batched (`git cat-file --batch`) and a cheap drift probe avoids recapturing unchanged worktrees.
