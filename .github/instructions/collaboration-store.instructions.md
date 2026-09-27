---
description: Locking and audit consistency when changing the collaboration identity and policy staging stores
applyTo: 'engine/src/services/collaboration_identity_store/**/*.rs'
---

## Learnings

Acquire schema metadata, then the authority row (`FOR UPDATE` for mutations, `FOR SHARE` for coherent
snapshot reads), before identity/policy/audit rows; a child-row lock cannot protect an absent binding.
For example, an audited identity lookup must wait behind a binding paused at receipt insertion, rather
than independently reading missing identity/history rows and reporting absence; preserve the regression
in [the lifecycle fault tests](../../engine/tests/collaboration_identity_audit/faults.rs).
Only the in-transaction writer/readback may use raw records before appending its receipt; ordinary reads
must compare durable state with the current audit head, and no result should escape before commit.

For auth-source composition, follow the additional
[source-first locking rule](durable-auth-source.instructions.md); do not acquire the source lock from
inside a registry-first command.
Crate-internal command composition accepts a borrowed SQL transaction, not an autocommit connection
or another pool, and returns only a pending receipt; the outer owner must confirm commit before publication.
