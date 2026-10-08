# Journaled Session draft publication

Status: **C2-U internal preparation, included in 0.5.4**. Decision date: **2026-10-08**.
Source version: **0.5.4**.

C2-U adds an explicit journaled wrapper around a surviving [C2-P publication attempt](session-task-draft-publication.md).
Each resource step first commits an Unknown journal entry, then commits its resource SQL and completion
marker together in a separately authorized transaction. This can create a multi-item Task through
successive calls, but is neither a whole-draft transaction nor a restart or retry protocol.

## Separate installation and ownership

`DraftIntentJournal::install_empty_dispatch_namespace()` explicitly installs **journal schema 2** in
a fresh empty namespace. The original schema 1 installer, registration and record reads remain supported;
schema 1 cannot dispatch and is never automatically upgraded. There is no migration or startup installer.

Register the pristine attempt's [C2-T intent](session-task-draft-intent.md), then call
`attempt.into_journaled(workspace)` to consume it into `SessionJournaledTaskDraftPublication`.
Conversion is a pure Ready/zero-confirmed ownership check, not a database read or proof that registration
committed. It retains the original source, trusted workspace, Session fingerprint, allocated references
and text. The wrapper has no `Clone`, importer, unwrap or path back to the unjournaled attempt.
Keep its metadata private and omit draft text and tokens from logs.

Only `wrapper.advance(original_token)` dispatches a step. It checks the original token and advances at
most one allocated Artifact, or the final Task. Each database transaction independently checks current
Session authority and the intent's exact owner and account generation. A loaded intent or parsed
checkpoint cannot construct the wrapper or grant a dispatch permit.
An incorrect token is rejected before dispatch without consuming the wrapper's Ready state.

The immutable intent keeps its canonical Ready/zero-progress checkpoint; progress is stored separately
as ordered step records. At most 32 Artifact steps and one Task step are possible. An occupied ordinal
is not adopted, and a matching nonce is not permission to retry.

## One step has two transaction boundaries

| Phase | Required checks and changes | Confirmed boundary |
|---|---|---|
| A reserve | Current Session, exact bound intent and original checkpoint, and complete Committed prefix; append the next ordinal with a fresh random nonce and Unknown state | Separate synchronous journal COMMIT; without its acknowledgement, do not dispatch B |
| B begin | Reauthorize and recheck exact intent, owner/account, ordinal and nonce; conditionally set the marker to Committed inside the uncommitted resource transaction | This change is not yet externally committed and occurs before business validation |
| B resource | Use the existing borrowed Artifact transaction, or C2-M content validation and Task creation within this same transaction | Resource metadata/outbox and the marker share one database transaction |
| B finish | Read back the exact intent and full step/nonce prefix, verify relation pins, enforce synchronous commit and recheck fresh authorization | Only a confirmed COMMIT advances the wrapper's in-memory confirmed prefix |

Setting the marker before business validation ensures its database triggers run before those checks.
The final journal check only reads: it does not repair data or perform another completion write after
business validation. Previous committed nonces observed for B remain part of its exact prefix comparison, not just
the current ordinal. Task creation still rereads the referenced content bytes through C2-M.

The journal does not combine Artifact publication with later Task creation. Each successful call
commits one resource; earlier commits survive a later failed or dropped step. An empty draft goes
directly through the Task step, with the same journal and authorization boundaries.

## Uncertainty stops the live wrapper

Once a step is dispatched, a failure, cancellation or dropped future leaves the live attempt stopped
at an unconfirmed step. If A's COMMIT is unconfirmed, B is not called. If B does not commit, A's durable
Unknown record remains; private Artifact files may remain as well, and neither is automatically deleted.
An uncertain B COMMIT may already have committed both the resource and marker, so an error or missing
acknowledgement does not prove that the stored marker is still Unknown or that the write rolled back.

There is no automatic retry, repeated-ordinal adoption, file cleanup, state repair or completion based
on a later observation. Only the still-owned wrapper can continue after a confirmed successful step.
Dropping it or losing the process loses that continuation ability; reading an intent or checkpoint
does not recover the text, original attempt or execution authority.

## Limits and existing paths

Original C2-P `advance` remains a separate, unjournaled internal API. This wrapper does not silently
require a journal for all callers or connect public/browser saving. Journal namespace names and
within-transaction identity pins do not prove physical source continuity between A and B or original
storage incarnations across restart, protect against a privileged administrator, or prevent backup replay. Synchronous COMMIT is a database
acknowledgement boundary, not host fsync, failover or crash-recovery acceptance.

There is no public HTTP/SDK route, browser connection, live-authentication cutover, restored attempt
or new deployment authority. C2-T's schema 1 evidence remains scoped to registration/read, and C2-R
parsing and C2-Q/S observation remain data/read operations rather than recovery permits.
A fresh Session's ability to read a C2-T record does not replace this wrapper's original-token requirement.

[C2-V journal inspection](session-task-draft-journal-inspection.md) separately lets a currently
authorized Session read the recorded prefix without a surviving wrapper. It reads no resource,
exposes no nonce and supplies neither a dispatch receipt nor a way to resume, adopt or retry this wrapper.

## Verification boundary

The wrapper lives in [`draft_publication/dispatch.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/dispatch.rs)
and the step journal in [`draft_intent_journal/dispatch.rs`](../../engine/src/services/draft_intent_journal/dispatch.rs).
The source-owned [A/B transaction adapter](../../engine/src/services/auth_service/durable_source/draft_publication/dispatch_transaction.rs)
uses a private sealed permit; stored data cannot construct one.
`session_task_draft_dispatch_tdd` keeps schema admission, two-transaction ordering, exact prefixes,
resource faults, cancellation and authorization changes separate from earlier intent-record tests.
Execution records belong in [project status](project-status.md); the [testing guide](testing-guide.md)
distinguishes these internal checks from public saving and deployed recovery.

On 2026-10-08, fourteen exact isolated PostgreSQL cases and three common guards passed. These cover
the journaled dispatch boundary, not default/full-gate or adjacent-suite results, imported recovery,
physical source continuity across A/B, browser saving or deployment acceptance.
