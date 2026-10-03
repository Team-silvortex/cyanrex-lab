# ADR-018 Session authorized private artifacts

Status: **C2-F included in source release 0.4.8**. Decision date: **2026-10-03**.

Current durable Sessions now authorize a member's own Artifact creation, revision and exact-content
reads. The same source transaction checks identity/membership, changes metadata/events, rechecks the
Session and confirms commit before returning metadata or bytes. This extends private-work authorization
without teaching roles, Task types or Runs. This Artifact-only slice does not itself enable public
uploads, sharing, Review commands, Task acceptance, live authentication cutover or migration.
This slice first appeared in 0.4.8; the separate Review adapter included in 0.4.9 is described below.

## Configuration and operations

Trusted server composition calls `DurableAuthSource::open_artifact_workspace(namespace, scope, root)`.
It opens an existing private Unix directory using the [ArtifactStore safeguards](artifact-revision-store.md),
without creating it, changing permissions, querying a database or installing a schema. The returned
`SessionArtifactWorkspace` has private fields and no deserializer. Its namespace, Workspace and opened
root are server configuration, never request-controlled paths or an asserted owner. The store handle
uses only borrowed-transaction helpers during authorized operations; it never acquires its own pool.

Source accounts/Sessions and C1 registry remain in one private schema. Artifact metadata occupies a
different explicit schema in the same database, selected on the source connection. Both must already
be initialized by trusted composition. Ordinary commands do not adopt, upgrade or repair them.

| Source operation | Inputs beyond Session/configuration | Result after confirmed commit |
|---|---|---|
| `create_session_artifact` | Artifact ID, revision ID, bounded `ArtifactDraft` | First immutable revision |
| `revise_session_artifact` | Exact current parent, new revision ID, bounded draft | New revision; original bytes remain unchanged |
| `read_session_artifact` | Exact `ArtifactRef`, including digest | Owned revision and verified bytes, or none |

Owner, digest, sequence, parent and publication metadata come from the server. Unknown/other-owned
objects are not accessible even if the caller knows their exact IDs or digest. Another owner's missing
blob cannot turn an ownership refusal into a blob error. No global content deduplication grants access.
Drafts preserve the existing one-MiB limit and metadata bounds; these are not aggregate disk quotas.

## Shared private work authorization

Task and Artifact adapters now share a crate-private `private_work` guard. It preserves C2-E's exact
account incarnation, active audited Human, active Workspace and audited active membership policy.
Private work requires neither teaching roles nor deployment grants, and such roles grant no access to
another member's content. Unbound, suspended, retired, expired, revoked or audit-inconsistent actors
fail closed. Recreated usernames cannot inherit old ownership.

The guard preserves namespace name/OID checks, source relation validation and source → registry metadata
→ authority → resource metadata/head lock order. All operations, including reads, hold authority locks;
same-authority commands serialize. After resource work, it verifies the target, restores and verifies
the source, rechecks actor/membership and finally checks the exact Session against fresh database time.
Only a subsequent confirmed commit releases a pending result to the caller. There is no detached
`validate_session` snapshot used as permission for a later transaction.

Borrowed Artifact reads lock the head before reading related revisions/events. This matters because
the source transaction uses READ COMMITTED: copying the standalone reader's unlocked head lookup could
mix pre- and post-revision statements. Direct trusted-store reads still use their original read-only
REPEATABLE READ snapshot. Historical exact reads remain valid after a newer revision commits; there
is no floating `latest` reference or cached authorization. A Session expiring while a read waits cannot
receive its pending bytes.

## Files and database publication

This is **not** a filesystem/database atomic transaction. Authorization, scope, known owner/type/parent
and ID uniqueness checks precede new-file creation. Publication retains exclusive private file creation,
sync, digest/readback verification, metadata/outbox checks and immutable history. Source checks happen
again after these operations, before commit. Search-path changes are transaction-local and must not
remain in the source pool after success, failure or cancellation.

A file written before an outbox failure, final authorization failure or deferred commit failure may
remain without committed metadata. Reads cannot adopt it or return it without valid metadata and current
authorization. Cancellation can leave a blocking file worker running; the ten-second async deadline
does not terminate that worker. Database failure does not authorize overwriting, deleting, adopting or
retrying an unpublished file. Tests deliberately confirm retained files alongside rolled-back metadata.

Timeout, cancellation and missing commit acknowledgment are not rollback proof. There are no command
receipts, automatic retries, garbage collection, recovery, aggregate quotas, global event delivery or
crash/disk-full guarantees. Corrupt/replaced roots or changed bytes fail closed. Checks are not protection
against a same-UID/privileged host operator or a complete database schema/history attestation.

## Verification and remaining boundary

The original C2-F suite includes three default tests covering explicit configuration, directory refusal
without repair, unavailable source without file writes and no detached/live composition. Eighteen explicit PostgreSQL cases cover exact
history, cross-owner/scope/digest isolation, revision races and collisions, unbound/revoked/retired/recreated
accounts, schema/audit refusal, suppressed/deferred writes, post-write authority changes, RLS/temp shadows,
valid cloned-namespace substitution, corrupt roots/bytes, both logout orders, expiry during write/read
waits, cancellation, membership revocation, pooled path restoration and reads behind a trusted revision.

`scripts/test-session-artifact-storage.sh` checks every exact name before running it on an explicitly
disposable database; CI uses that runner and tests the full selection. Never inject these faults into
deployed data. See [project status](project-status.md) for executed checks and omissions.

Direct trusted ArtifactStore and TaskStore writers remain available without a cutover fence; existing
events cannot prove they all passed a Session adapter. C2-E manual Task commands still reject Artifact
inputs and domain definitions. [C2-G](session-task-inputs.md) separately composes private
exact-input authorization with Task commands on one source transaction. [C2-H](session-review-commands.md) adds private human Review commands and exact owned target/evidence
checks on that same source transaction, instead of treating an earlier Artifact read as later authority.
Direct ReviewStore calls still trust attribution and do not validate content access. C2-H's configured
policy label proves neither policy execution nor historical Session authorship. Cross-user review
rights, domain policy, Task acceptance, legacy-writer fencing, public HTTP/CSRF/UI, offline migration
and retention remain separate gates; C2 and C-M2/C-M5 are not complete.
