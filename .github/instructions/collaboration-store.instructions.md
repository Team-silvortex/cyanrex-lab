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

Empty-authority bootstrap helpers are only for a newly created namespace behind the source-owned
installer fences. They must not adopt existing registry rows or invent an authenticated bootstrap actor.
Activate both audit schemas and verify the complete initial graph/audit heads before the outer commit;
ordinary registration/binding still assigns no implicit roles or deployment grants.

The C1-K maintenance observer is the explicit exception to row-locking reads, not a runtime adapter:
accept only the source-owned READ ONLY / REPEATABLE READ transaction and validate every audit entry,
per-subject state continuity and final head. Sequence gaps are legal; a baseline is not invented
historical authority. Bound variable payloads before fetching and share record decoders without
removing locks from ordinary readers. Never convert the aggregate observation into current access.

Guard stored registry timestamps in SQL before binary chrono decoding: isfinite plus the bound chrono
MAX_UTC must protect legacy identity retired_at and identity/policy audit recorded_at. SQLx decode
panics are not caught by try_get().map_err(...). Preserve legitimate NULL retirement with an independent
valid_retired_at flag; invalid non-NULL input must be InvalidRecord, never an active/absent identity.
Mandatory audit-time NULL projections must be InvalidRecord. Keep rows visible for rejection rather
than filtering, clamping or repairing times. Every shared audit projection caller binds the owned maximum
parameter slot, including head/history/replay/append readback, reconciliation and LIMIT 0 schema checks.
Preserve InvalidIdentity, IdentityHistoryMismatch and PolicyHistoryMismatch at reconciliation boundaries.
Keep finite-boundary, old-history, exact-replay and write-readback regressions; this adds no age/order
policy, schema change or proof that unrelated timestamp paths are safe.
