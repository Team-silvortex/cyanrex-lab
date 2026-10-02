---
description: Explicit local provisioning and post-commit secret delivery boundaries
applyTo: 'engine/src/bin/cyanrex-provision/**/*.rs'
---

## Learnings

SQLx `new_without_pgpass` still reads PG environment variables. Clear PG* only in this standalone
process before creating any runtime threads, supply every connection coordinate explicitly and never
load Engine/.env defaults. Keep the loopback/socket restriction, private bounded config and generic
errors; raw SQL errors or argument values can contain credentials.

Plan/inspect must remain catalog-only read-only transactions, not hidden schema installers or healthy
authority assertions. Confirm the reviewed physical target and fixed intent before dispatch, and pin
replacement connections. C1-I remains the only bootstrap transaction authority; the digest is not
authentication, a reservation or a replay receipt. Do not add privileged recovery by reinitialization.

Reserve and sync a non-secret private output marker before mutation. Publish TOTP only after confirmed
COMMIT, through the held descriptor with path/parent identity checks. Never adopt existing output or
delete uncertain evidence. Commit uncertainty and confirmed-commit delivery failure are different:
neither automatically retries, and file failure cannot undo database commit. Preserve subprocess
concurrency, cancellation, replacement and real short-write tests in `provision_postgres_tdd`.

Reconcile is a separate bounded read-only observer, not an expanded inspect or bootstrap replay.
Accept config only; never open enrollment/password files or expose auth secret/token columns.
Keep physical reconnect pinning and omit the initial username as a lifecycle filter/survivor claim.
Report only a complete consistent snapshot, never repair authority or infer permission to retry.
Preserve the database-enforced read-only/column-privilege regression in `durable_reconciliation_tdd`.
