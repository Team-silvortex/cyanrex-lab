# Read-only observation of draft publication targets

Status: **C2-Q internal preparation, included in 0.5.3**. Decision date: **2026-10-07**.
This Unix-only operation observes the exact target of a surviving [C2-P attempt](session-task-draft-publication.md)
whose state is `Unconfirmed`. It compares currently readable data with the planned create operation.
It does not confirm the earlier write, change attempt state, resume publication or establish rollback.

## Operation and current authority

`attempt.observe_unconfirmed(&self, token)` returns
`Result<SessionDraftPublicationObservation, SessionDraftPublicationError>`. Ready and Complete attempts
return `NoUnconfirmedStep`. The original canonical Session fingerprint must match; a different token
returns `SessionChanged` without database access. The fixed source and workspace cannot be replaced.
A matching fingerprint is not authorization: the original Session must still pass the existing read
command's current account, audited membership, namespace and final Session checks. An expired or
revoked Session returns its typed error; logging in again does not take over this attempt.

Each call performs just one existing `read_session_artifact` or `get_session_content_task`, selected
by the attempt's exact unconfirmed step. It adds no direct SQL, authorization guard or business write.
The existing transaction still takes locks and commits; this is not a new SQL `READ ONLY` setting.
Its per-command deadline remains in force, not a global workflow or observation-quota guarantee.

## Observation results

The report contains `step` and `result`, with no payload body. `SessionDraftObservedOutcome` has three
values; read failures remain errors rather than being converted into a result.

| Result | What was observed | What it does not establish |
|---|---|---|
| `MatchesPlannedCreate` | Readable current content matches the planned create target and expected metadata | That this attempt created it, that the earlier commit was acknowledged, or that it remains unchanged |
| `DiffersFromPlannedCreate` | Readable valid data differs from the planned create; a later legitimate Task edit can produce this result | Permission to replace, repair, delete or adopt it |
| `NotVisible` | The authorized read returned no visible target at that read's time | Global absence, rollback, absence of an in-flight writer or safe retry |

Another writer could have created matching data at the allocated identity. A match is therefore a
current observation, not publication provenance or an upgrade to C2-P's historical confirmations.
The metadata report can itself be sensitive; keep it private and omit draft text and tokens from logs.

Artifact observation reads the allocated exact revision, which can still match after the Artifact
head advances; it does not require that revision to remain the current head. Task observation instead
reads the current Task snapshot and contents.

When a Task exists, the one existing C2-M read transaction validates the current Task and its current
references against actual bytes. It does not separately revalidate the attempt's entire historical
Artifact prefix. When the Task is not visible, the existing early-return path validates source and
Task boundaries but does not inspect the Artifact namespace. `NotVisible` is not a full-workspace
health result. Namespace pins protect each transaction, not privileged same-name clones installed
before the call or an entire database history.

## State and recovery limits

Observation borrows the attempt immutably. Success, mismatch, invisibility, read error and cancellation
do not append confirmations, mark the Task complete, unlock `Unconfirmed` or enable `advance`.
Separate observations occur at separate read times; a writer may finish after an absence observation.
There is no combined snapshot across calls or promise that an observation remains current.

The attempt remains caller-owned memory with its original Session. Destroying it or exiting the
process loses that context. No durable journal, receipt import, new-login recovery, retry, deletion,
cleanup, source installation, public HTTP, browser integration or deployment authority is provided.

0.5.3 [C2-R metadata checkpoints](session-task-draft-checkpoint.md) do not change this admission
boundary: parsing a checkpoint cannot recreate the original attempt or invoke an observation.

The separate [C2-S inspection](session-task-draft-inspection.md) uses caller-supplied current authority,
not C2-Q's original fingerprint; it cannot replace this operation or verify the historical caller.

## Verification boundary

On **2026-10-07**, **seven default Rust units, twelve exact disposable PostgreSQL cases and three
common guards passed**. They separately exercise the comparisons, current authorization, precise
read scope, typed failures, pending creators and cancelled reads. These 22 new checks establish
neither the past write's provenance nor a connected browser/deployment workflow.

Pure tests and disposable PostgreSQL cases must separately cover state/token admission, all observed
results, typed read errors, unchanged attempt state, current Session rejection and exact read scope.
The exact SQL entry is `scripts/test-session-task-draft-observation.sh`, selecting the
`session_task_draft_observation_tdd` target. Dated execution evidence belongs in
[project status](project-status.md); the [testing guide](testing-guide.md) separates these observations
from mutation, browser and deployed-system acceptance. C2-P's earlier records retain their original scope.
