# Session authorized draft journal inspection

Status: **C2-V internal preparation, included in 0.5.4**. Decision date: **2026-10-08**.
Source version: **0.5.4**.

C2-V separately reads a retained intent and its recorded step prefix under current Session authority.
It requires the existing [C2-U schema 2 journal](session-task-draft-dispatch.md), but not a surviving
publication wrapper. Its result describes journal metadata, not current resource contents, a recovered
attempt or permission to retry an uncertain write.

## Explicit read and identity boundary

`source.inspect_session_draft_journal(token, &SessionDraftIntentWorkspace, task_id)` returns
`Result<Option<SessionDraftJournalInspection>, SessionDraftIntentError>`. The caller supplies the
trusted source/workspace configuration; stored namespace names cannot select another route.
A newly issued Session may read only when both its exact owner and account generation match the
[C2-T intent](session-task-draft-intent.md). A shared username or teacher role is not a substitute.

This is **schema 2 only**. It neither installs a namespace nor upgrades schema 1, and adds no DDL.
The existing source-owned transaction validates current authority, bound configuration, the immutable
intent and the full ordered step prefix. It rechecks the prefix and pinned relations, then checks
fresh authorization before committing and returning. Failures keep their typed errors; detected corruption,
expired authorization and incompatible storage do not become an absent result.

This read makes no business writes or file accesses. It still takes the existing transaction locks
and commits; “read-only” is not a claim that the transaction uses PostgreSQL `READ ONLY` mode.
It queries neither Task nor Artifact storage, so configured target namespace names are checked as
trusted routing labels, not as proof that the original resources or physical stores still exist.

## Recorded prefix rather than execution authority

The opaque result exposes only these accessors:

| Accessor | Meaning |
|---|---|
| `intent()` | The authorized immutable intent, whose checkpoint remains Ready with zero reported confirmations |
| `recorded_committed_step_count()` | Number of consecutive Committed step records, including the final Task step if present |
| `unknown_step()` | The optional next recorded Unknown target; no nonce is exposed |
| `all_steps_recorded_committed()` | Every planned Artifact step and the final Task step is recorded Committed |

A legal prefix consists of zero or more Committed records, optionally followed by one Unknown record.
Gaps, out-of-order targets, a later row after Unknown and excess rows are rejected rather than repaired
or truncated. At most 32 Artifact steps and one final Task step are possible. An empty payload still
needs its Task record: zero records do not mean publication is complete.

The result does not change the Ready/zero-progress checkpoint, append confirmations or update any
live wrapper. Metadata remains private; do not log titles, labels or references indiscriminately.
The count and flags report the journal that was read, not a resource reread, dispatch receipt or
proof of original publication history against an administrator, cloned storage or backup replay.

## Absence and uncertainty

`None` can mean a missing record or one not visible to the current owner/account generation.
It does not prove that no work was dispatched, that a previous transaction rolled back or that
the caller may register, publish or delete anything again. It is not proof of global absence or that
no future dispatch can occur; a later inspection has its own read time. This reader waits for the
existing protocol locks, with typed failure on timeout, rather than bypassing a writer's lock and
treating its uncommitted record as missing. Uncertain resource outcomes still do not authorize retry.

An Unknown record is not a rollback receipt. A fully recorded prefix does not reconstruct the text,
original attempt or sealed dispatch permit. Journal inspection cannot resume, adopt, retry, clean up,
repair or restore a stopped wrapper. Current transaction pins do not attest original storage
incarnations across restarts. Host crash durability and backup replay remain separate boundaries.

[C2-Q](session-task-draft-observation.md) still needs the original surviving attempt/token;
[C2-S](session-task-draft-inspection.md) separately reads an explicit checkpoint target.
Neither is invoked here. C2-R parsing remains pure data, and the original C2-P dispatcher remains
independent. No public HTTP/SDK, browser saving, live-authentication cutover or deployment change is added.

The separate [C2-W step observer](session-task-draft-journal-observation.md) reads actual resources
after capturing a journal in the same transaction. It shares private validation, not this public
method, and does not turn C2-V's metadata result into an execution or recovery permit.

## Verification boundary

The current-Session adapter is [`journal_inspection.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_inspection.rs);
the private journal reader is [`draft_intent_journal/inspection.rs`](../../engine/src/services/draft_intent_journal/inspection.rs).
The Unix-only `session_task_draft_journal_inspection_tdd` target and
`scripts/test-session-task-draft-journal-inspection.sh` keep this read separate from dispatch execution.

On 2026-10-08, two default Rust tests, ten exact isolated PostgreSQL cases and three common guards
passed. They cover recorded prefixes, current authority, schema/pin faults, waiting and cancellation,
not resource verification, restored publication or deployed recovery. Earlier C2-T/U runs are not
inherited evidence. [Project status](project-status.md) preserves the initial behavior Red and fixture
correction; the [testing guide](testing-guide.md) identifies the exact runner and selection scope.
