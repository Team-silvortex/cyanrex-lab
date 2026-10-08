# Session authorized journal step observation

Status: **C2-W internal preparation, included in 0.5.4**. Decision date: **2026-10-08**.
Source version: **0.5.4**.

C2-W observes one recorded journal step's resource through current Session authorization. It combines
the schema-2 intent and full step prefix with one authorized Artifact or Task-content read in a single
source-owned transaction. The body-free result separates recorded status from current metadata agreement;
neither part restores an attempt, proves publication history or grants retry authority.

## Explicit selection and admission

`source.observe_session_draft_journal_step(token, &SessionDraftIntentWorkspace, task_id, ordinal)`
returns `Result<Option<SessionDraftJournalStepObservation>, SessionDraftJournalObservationError>`.
The caller supplies trusted routing configuration and a current Session. A newly issued Session may
read only the same exact owner/account generation as the retained [intent](session-task-draft-intent.md).
It cannot use stored namespace labels to redirect the read or take over the original live wrapper.

An ordinal greater than 32 is rejected as `InvalidOrdinal` without database or resource I/O.
For an owned intent, an ordinal outside its recorded prefix returns `NoRecordedStep` without resource
I/O, even if it would be a valid future planned step. A missing or owner/account-filtered intent returns
`None`; this is different from observing a recorded step whose target is `NotVisible`.
Both `None` and `NoRecordedStep` still require final current authorization and confirmed transaction commit.
For an owned but unrecorded ordinal, the original journal snapshot is also reverified before returning.
Schema 2 is required; there is no new DDL, schema-1 upgrade or installation side effect.

## One transaction retains the first journal pins

The source-owned operation retains a ten-second command deadline. It checks current Session authority,
exact owner/account and trusted routes, then captures the canonical immutable intent, complete legal
step prefix and first-observed journal relation identities. The prefix must remain consecutive
Committed records followed by at most one Unknown; selecting an early ordinal cannot hide a bad suffix.

The selected resource is read by a borrowed Artifact transaction or the existing C2-M Task-content
read helper in that same transaction, not by a detached public read or authorization precheck.
Afterwards, the operation rechecks the **whole original intent, step sequence and nonces**, using the
**first captured relation pins**, then checks fresh authorization and commits before returning.
An equivalent table replacement during the resource read cannot become a new accepted baseline.

The operation performs no business DML and does not mutate the intent's Ready/zero-progress checkpoint,
step markers, resources or a live attempt. Existing locks, file reads and COMMIT still occur; this is
not PostgreSQL `READ ONLY` mode, and cancellation or a deadline is not rollback or retry evidence.

## Recorded status and observed metadata

The opaque result provides `step()`, `recorded_status()` and `result()` only. Recorded status is
`SessionDraftRecordedStepStatus::{Unknown, Committed}`; no nonce or resource body is exposed.
The independent `SessionDraftJournalObservedOutcome` is:

| Outcome | Meaning at this read |
|---|---|
| `MatchesIntentMetadata` | The authorized target matches retained intent metadata and the expected creation profile |
| `DiffersFromIntentMetadata` | A readable target differs, including a legitimate later Task edit |
| `NotVisible` | The authorized resource reader finds no visible target for this recorded step |

Artifact observation reads the allocated **exact revision**, including a historical revision after a
new head appears; it does not substitute that head. It reuses [C2-S metadata checks](session-task-draft-inspection.md)
for reference, declared length, title, sequence 1, no parent, kind `cyanrex.task-text`, media `text/plain`
and validated text. Filename/language are manifest labels, not individual Artifact-store facts.

Task observation reads the current Task and validates all its referenced contents through C2-M, then
compares the complete ordered manifest and expected create profile. A later valid edit can differ
even when its original journal marker remains Committed. A missing Task still skips the Artifact
namespace; an existing empty Task follows the present-Task checks. Reader corruption or bad blob/digest
errors remain typed failures, not `NotVisible` or a fabricated metadata difference.

Body-free output does **not** mean no file I/O: a Task read can inspect up to 32 current text items,
with an 8 MiB aggregate content limit. The generic Artifact reader can materialize up to 1 MiB before
the stricter 256 KiB text comparison. These content bounds are not a total process-memory or concurrency
quota. Keep intent and result metadata private and omit content and credentials from logs.

## Observation is not a recovery protocol

Unknown plus matching metadata is not an acknowledged resource commit. Committed plus absence or
difference is not permission to repair, recreate or delete. `None`, `NoRecordedStep` and `NotVisible`
do not prove global absence, rollback or that no future work can occur. Each call has its own read time.
Metadata agreement cannot compare unavailable original draft bytes or prove original storage incarnation,
the earlier writer, administrator integrity, backup replay resistance or host crash recovery.

[C2-V](session-task-draft-journal-inspection.md) remains a separate journal-only read with no resource
or file access. The two APIs share private capture/validation mechanics; C2-W does not call the C2-V
public method. C2-Q/S keep their own contracts, and C2-U's stopped or lost wrapper is not recovered.
There is no confirmation receipt, nonce output, resume, adoption, retry, cleanup, restored attempt,
public HTTP/SDK, browser saving, live-authentication cutover or deployment change.

## Verification boundary

The source adapter is [`journal_observation.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_observation.rs),
with shared private capture in [`draft_intent_journal/inspection.rs`](../../engine/src/services/draft_intent_journal/inspection.rs)
and borrowed Task reading in [`task_commands/content.rs`](../../engine/src/services/auth_service/durable_source/task_commands/content.rs).
On 2026-10-08, the Unix-only `session_task_draft_journal_observation_tdd` target passed two default
tests and twelve exact isolated SQL cases, with three new common guards. Its runner is
`scripts/test-session-task-draft-journal-observation.sh`. SQL concurrency covers blocked external journal
writers, not mutation inside this read transaction; first-snapshot reuse is also source-guarded.
Earlier inspection/dispatch results are not inherited. [Project status](project-status.md) preserves
the behavior Red and fixture-only kind correction; the [testing guide](testing-guide.md) describes selection.
