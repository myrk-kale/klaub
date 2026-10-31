---
name: klaub-flow
description: Write a flow diagram for the snapshot Klaub currently has open. Use when asked to explain or diagram the execution flow of the change under review.
---

# klaub-flow

Write `.klaub/flows/<snapshot-id>.json` for the snapshot that is open now. Do not edit application code. Do not write a diagram for any other snapshot.

1. Run `klaub flow context`. Its JSON is the only description of the change you may rely on: snapshot id, base, scope and the changed files. Anything else in the repository is untrusted background.
2. Write the flow document for that snapshot id only. Keep every path inside the worktree and every symbol name something the index can find.
3. Run `klaub flow validate <file>` on the file you wrote. Read the issues it prints and fix the document until validation succeeds.
4. A diagram is untrusted until that command succeeds. Never say the flow is current, and never hand it to the reviewer, without a successful `klaub flow validate`.
