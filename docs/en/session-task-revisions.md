# ADR-022 Session authorized task input replacement

Status: **C2-J included in source release 0.4.9**. Decision date: **2026-10-03**.

An owner can explicitly replace a Draft task's ordered Artifact references after publishing new
immutable content. This connects the backend preparation layers for task payload editing; it does
not connect the browser draft to a server or introduce public HTTP commands. The source version is 0.4.9.

## Command contract

`replace_session_input_task_inputs` accepts a current Session, trusted `SessionTaskInputsWorkspace`,
Task ID, expected Task revision and 1–32 exact Artifact references. The task must be manual and
already have inputs. `replace_session_catalog_task_inputs` uses `SessionCatalogTaskWorkspace` and
allows 0–32 references, including changing between empty and nonempty inputs. It compares the full
stored definition with the configured exact entry before and after the write, without invoking a
provider. The existing reference-free manual adapter does not gain a replacement bypass.

Both commands derive the owner from current identity and membership. A teacher role does not grant
access to another member's private task. Only Draft tasks can change inputs; stale revisions, identical
ordered lists, duplicate Artifact/revision coordinates, oversized lists and invalid scopes are rejected.
Reordering distinct references is a change. Title, definition, owner, creation time and status stay
unchanged; one successful replacement increments the Task revision once and updates its timestamp.

The new list contains exact Workspace, Artifact ID, revision ID and digest pins, not text, filenames,
local editor IDs or a request to use the latest version. Publication of new Artifact content is a
separate authorized operation. A failed replacement does not delete, adopt or roll back that content,
and does not authorize automatic retry. Previous Artifact versions and Review judgments remain intact.

## Authorization and transaction

Replacement reuses the [Task input transaction](session-task-inputs.md). Source and registry locks
precede Task metadata and the Task row. All old and new references, at most 64, are then verified in
one global Artifact lock order. References removed by the replacement are still checked. Matching
coordinates with different digests must not be deduplicated: valid old content cannot hide a forged
new reference. Mutations discard verified content one item at a time rather than returning its bytes.

Task update and `cyanrex.task.inputs_replaced` outbox insertion occur in the same transaction. After
the writes, the command rechecks old and new content and rereads the pending Task. The shared guard
then verifies all original namespace identities, the current account incarnation, audited active
membership and exact Session using fresh database time. A result is returned only after confirmed
commit. Competing saves using the same expected revision cannot both succeed.

The ten-second command deadline and existing statement/lock timeouts apply. Cancellation, timeout
or lost acknowledgement around commit does not prove rollback. No command receipt or outcome
reconciliation is added. Content checks cannot prevent same-UID modification after verification,
and Session validity during an arbitrarily delayed commit is not guaranteed.

## Storage compatibility

The Task storage schema is now **2** for explicit installation into an empty dedicated namespace.
Schema 1 is rejected by reads and writes; there is no automatic upgrade, adoption or repair. Existing
deployments and teaching databases are not touched. The shared `CoreSchemaVersion` remains 1.

The new schema admits the distinct replacement event and Draft snapshots above revision 1. Creation
still requires Draft revision 1; later Draft heads require `cyanrex.task.inputs_replaced`, and other
statuses require `cyanrex.task.status_changed`. The lifecycle graph has no transition back to Draft.
These checks establish current Task/outbox-head consistency, not complete history or writer provenance.
The replacement store operation is crate-internal and cannot serve as a new public owner-supplied API.

## Verification and next boundary

Unit tests cover immutable fields, revision overflow, ordered no-ops, input bounds and event selection.
Explicit disposable PostgreSQL cases cover real Artifact revisions, catalogue empty inputs, old and
new content validation, private ownership, schema compatibility, lifecycle revocation, competing writes,
trigger faults, deferred commit failure, expiry/logout and cancellation. The CI runner verifies exact
test names before executing them. Actual results are recorded in [project status](project-status.md).

HTTP authentication and CSRF integration, server-backed payload metadata, browser saving/conflict UI,
binary attachments, migration and live cutover remain separate work. This does not execute code,
validate typed domain evidence, issue rule Reviews, grant cross-user access or accept a Task.
