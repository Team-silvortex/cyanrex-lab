# Session authorized catalogue draft publication

Status: **C2-Z internal preparation included in source release 0.5.5**. Decision date: **2026-10-11**.
Source release: **0.5.5** (**2026-10-11**). This Unix-only create workflow publishes
the text items in a [C2-O plan](task-draft-publication.md), then creates a private catalogue Draft
through the existing [Session catalogue-content command](session-task-content.md). The caller advances
one write at a time. It is separate from the manual C2-P attempt and its observation/checkpoint/journal
workflows; it adds no HTTP route, browser save, assessment or recovery protocol.

Dated `Unreleased` follow-ups and verification records retain their original source versions and
execution scope; inclusion in 0.5.5 does not rerun or extend that evidence.

The separate **2026-10-11 Unreleased C2-AA follow-up** adds
[read-only catalogue target observation](session-catalog-task-draft-observation.md) to a surviving
unconfirmed attempt. It uses the original token and one existing current-Session reader, returns
body-free comparison metadata and never adopts progress, changes state or permits retry.

## Preparation and frozen definition

`DurableAuthSource::prepare_catalog_task_draft_publication(token, workspace, &reference, plan)` returns
an opaque `SessionCatalogTaskDraftPublication`. Its trusted `SessionCatalogTaskContentWorkspace`
selects the frozen catalogue and existing Task/Artifact routing. The reference is an exact
`TaskDefinitionRef`, not a full client definition, provider, owner, namespace or mode selector.

Preparation performs no database, file or network I/O. It checks canonical token shape and source/
workspace authority agreement, resolves the exact configured reference and freezes the complete
definition before allocating random UUIDv4 Task/Artifact/revision IDs and expected SHA-256 digests.
Unknown references and invalid structural preparation return `InvalidPreparation`; they do not
dispatch a Session command or fall back to manual work. Local editor IDs, filenames and text hashes
do not choose server IDs. The consumed plan retains C2-O's zero-to-32 text items and 256 KiB-per-item limits.

Composed packages and exact versions may supply definitions with both assessment bindings or neither.
Both remain catalogue work with `Some(TaskDefinition)`. Preparation does not authenticate the token,
verify current membership, install storage, execute a provider or grant Task acceptance.

The attempt fixes source, workspace, plan and definition for its lifetime. It keeps only a private
Session fingerprint in a domain separate from the manual attempt, not the raw token. The caller
supplies the original token again to `advance(&mut self, token)`; a different or malformed token returns
`SessionChanged` before dispatch and leaves a Ready attempt Ready. Fingerprint agreement is not authority:
each actual write independently checks the current Session and owner in the existing transaction.

Read-only accessors expose `definition`, `state`, `task_ref`, `allocated_artifacts`,
`confirmed_artifacts` and `confirmed_task`. The complete definition is frozen metadata, not an evaluator
or reusable permission. Allocated targets are not published records, and confirmed metadata does not
guarantee that bytes or authority remain valid later. Keep these private; never log tokens or draft text.

## Explicit publication steps

The attempt uses the existing `SessionDraftPublicationState`, `SessionDraftPublicationProgress` and
typed errors for reporting, without becoming the old manual attempt.

| State or result | Catalogue workflow meaning |
|---|---|
| `Ready` | The next write may be explicitly advanced with the original token |
| `Unconfirmed(Artifact { index, reference })` | The exact planned Artifact target was marked before the first await |
| `ArtifactConfirmed { index }` | The existing Session Artifact command returned matching complete metadata; the confirmed prefix grows and state returns to Ready |
| `Unconfirmed(Task { reference })` | The final catalogue Task create was marked before the first await |
| `TaskConfirmed` and `Complete` | The catalogue command returned the exact expected Draft/manifest and frozen complete definition; further advances return `Stopped` |

Each advance calls at most one existing current-Session command. Artifact steps publish fresh text
with fixed kind `cyanrex.task-text`, media type `text/plain`, the plan title and exact planned bytes.
Equal text items still receive distinct references. The last Artifact step never also creates the Task;
an empty plan starts directly with the final catalogue create and needs no dummy Artifact or owner.
This workflow does not reuse caller-supplied revisions, revise Artifacts or replace an existing Task.

Artifact confirmations must match their allocated references, authority, common owner, sequence one,
absent parent, kind/title/media type and byte length. Once every item is confirmed, the shared pure
manifest helper requires exactly one confirmation per planned item and binds those receipts with
the original text. This is supplied-value consistency, not a new file read or reusable authorization.

The final `create_session_catalog_content_task` independently rereads actual owned text and verifies
the current Session, namespace pins and full configured definition in its own transaction. Confirmation
requires the expected Task reference, owner, Draft state, revision one, manifest and
`snapshot.task.definition == Some(frozen_complete_definition)`. Matching only its reference or accepting
`None` is insufficient; binding choice, summaries and all other definition metadata remain significant.
No Evidence validation, assessment, Review, execution or automatic acceptance is dispatched.

## Cancellation and uncertain results

Keep the attempt outside the future that may be cancelled. Dropping an unpolled advance dispatches
nothing. A dispatched step is marked `Unconfirmed` before awaiting its command; success records the
complete returned confirmation without another await. A dispatched error, cancellation or mismatching
confirmation retains the confirmed Artifact prefix and exact unconfirmed target, and blocks further
advance. `InvalidConfirmation` is not adopted progress and does not reverse a potentially committed write.

Artifact publications and final Task creation remain separate transactions. A failed Task step does
not revoke earlier confirmed publications. No step deletes immutable files, undoes earlier writes,
retries an unknown target or reports whole-draft rollback. Per-command deadlines/lock limits remain
unchanged; there is no workflow-wide deadline, concurrent-attempt quota or global storage bound.

Destroying the attempt or exiting the process loses its in-memory bookkeeping. Confirmed-prefix
metadata is not a durable receipt, idempotency key, exactly-once guarantee or recovery permission.
A separately prepared attempt may create duplicate logical work.

## Separation from manual and journal workflows

The opaque catalogue attempt has no `Clone`, `Debug`, `Serialize` or `Deserialize`, no conversion into
`SessionTaskDraftPublication`, and no checkpoint, journal, restore or retry method. Its separate
C2-AA observation does not convert it into the manual attempt or adopt confirmations; it reuses the
existing body-free report DTO.
The existing C2-Q–W functions still accept their original manual attempt/checkpoint/journal types;
shared state/error/progress reporting cannot bridge that type boundary.

Only two pure checks are shared with C2-P: Artifact receipt matching and manifest construction.
The manual final-Task matcher still requires `definition == None`; the catalogue matcher separately
requires the complete frozen `Some` definition. Existing manual methods, checkpoint wire formats,
journal schemas and dispatch/inspection/observation semantics remain unchanged.

C2-N, C2-X and C2-Y HTTP routers do not call this stepper. There is no normal-app, public OpenAPI/SDK,
Session issuer, browser editor, migration or deployment composition. Publishing this plan is not an
atomic batch, authenticated checkpoint history or restart-recovery acceptance.

## Verification boundary

The dedicated default target and exact disposable PostgreSQL runner select catalogue preparation,
stepwise confirmation and uncertain-result boundaries independently from C2-P. Source guards check
the opaque type and absent manual/journal/public wiring; pure checks do not replace real Session,
transaction or file tests. Current selection is maintained in the [testing guide](testing-guide.md),
with actual dated results and omissions in [project status](project-status.md).

The [capability tensor](capability-maturity.md) extends W15's implementation references only. It does
not change coordinates, scores, evidence or topology, and the new catalogue sources do not inherit
the older manual publication's execution or deployment evidence.
