# Security policy

## Supported versions

Klaub is pre-1.0. Security fixes are made on the latest minor release only.

| Version | Supported |
| --- | --- |
| latest `0.x` | yes |
| older | no |

## Reporting a vulnerability

Please report vulnerabilities **privately** through GitHub: <https://github.com/myrk-kale/klaub/security/advisories/new>. Do not open a public issue or discussion.

Include a description, the affected version (`klaub version`), steps to reproduce, and the impact you expect. You can expect an acknowledgement within a few days and a status update at least every two weeks until the report is resolved. We credit reporters in the advisory unless they prefer otherwise.

## Scope

Klaub reads untrusted data (source code, file names, review comments, flow documents produced by agents) and drives other programs (Git, tmux, the harness, the user's test command). Reports of the following kinds are in scope:

- terminal escape-sequence injection through any rendered or pasted text,
- path traversal or symlink escape when reading briefs, reports, flows or source files,
- shell or argument injection in subprocess invocation,
- bypass of the verification of the embedded tmux binary, or use of an unverified binary,
- pasting into a pane other than the recorded one, or sending `Enter` without a user action,
- information disclosure through logs, diagnostics or exports.

The threat model is described in [docs/security.md](docs/security.md). Running a user-configured test command executes project code by design and is documented as such; it is not a vulnerability in itself.
