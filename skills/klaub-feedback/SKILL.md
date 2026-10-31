---
name: klaub-feedback
description: Address the comments of a finished Klaub review, one comment id at a time. Use when the reviewer hands over feedback from `klaub feedback`.
---

# klaub-feedback

The reviewer finished a round. Your job is to change the code so each open comment is addressed. The reviewer decides whether you did; you do not.

1. Read the comments from `klaub feedback <review-id>` or from the review JSON. Work by `comment_id`. Do not merge comments, drop one, or invent one the review does not contain.
2. Change the code the comment points at. Do not start a new round of edits until the code has actually changed for the comments you were given.
3. Treat P0–P3 as a heuristic that ordered the reviewer's reading, not as a verdict on severity and not as permission to skip a lower one.
4. Never mark a comment resolved, addressed, or done. Report what you changed and which `comment_id` it answers. The human decides the outcome in the Klaub reader.
