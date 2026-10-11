# Read only observation of catalogue draft targets

Status: **C2-AA internal preparation included in source release 0.5.5**. Decision date: **2026-10-11**.
Source release: **0.5.5** (**2026-10-11**). This Unix-only operation reads the exact
unconfirmed target of a surviving [C2-Z catalogue attempt](session-catalog-task-draft-publication.md).
It compares current readable data with the frozen definition and planned create values, without
confirming the earlier write, adopting progress, changing attempt state or enabling retry.

Dated `Unreleased` follow-ups and verification records retain their original source versions and
execution scope; inclusion in 0.5.5 does not rerun or extend that evidence.

## Operation and current authority

`attempt.observe_unconfirmed(&self, token)` returns
`Result<SessionDraftPublicationObservation, SessionDraftPublicationError>`, reusing the existing
body-free report rather than converting the catalogue attempt into a manual attempt. Only
`Unconfirmed` is admitted; Ready and Complete return `NoUnconfirmedStep`. A malformed or different
token returns `SessionChanged` before database access. Source, workspace, allocated targets and
the complete frozen definition cannot be supplied again or replaced.

The original canonical Session fingerprint binds the call to its surviving attempt, not to current
authority. The existing reader independently checks the current account, audited membership,
namespace pins and final Session in its transaction. Logout, expiry or suspended membership remains
a typed read error. A fresh login cannot take over the old attempt, and no detached authorization
precheck or fallback reader is introduced.

Each call selects one existing `read_session_artifact` or `get_session_catalog_content_task` from the
exact unconfirmed step. It adds no direct SQL or business write. The existing read transaction may
take locks and commit; this is not a new SQL `READ ONLY` mode. Existing per-command deadlines remain
in force, not a workflow-wide deadline, observation quota or combined snapshot across calls.

## Current comparison and typed failures

The report contains only `step` and `result`, with no draft text, returned payload, token or definition
body. `SessionDraftObservedOutcome` distinguishes these current observations:

| Result | What the authorized read found | What it does not establish |
|---|---|---|
| `MatchesPlannedCreate` | Current valid metadata and ordered text agree with the exact planned target; a Task also carries the complete frozen `Some` definition | Original publication provenance, acknowledged commit, durable confirmation or future agreement |
| `DiffersFromPlannedCreate` | Current valid data differs, such as a Task using another registered definition or a legitimate later replacement/status change | Permission to repair, overwrite, adopt, delete or retry |
| `NotVisible` | No owned target was visible to that authorized read at that time | Global absence, rollback, absence of an in-flight writer or safe retry |

Artifact observation reads the exact allocated revision and compares its complete planned metadata
and original bytes. That revision may still match after a later Artifact head advance. Task observation
instead reads the current catalogue Task snapshot and current ordered text, comparing the expected
Draft/revision/owner/manifest and full frozen definition, not only its reference. Bound and unbound
definitions remain catalogue `Some` values; neither invokes assessment.

Reader failures remain typed errors. A same-reference definition with changed metadata or bindings,
or a manual `None` record presented to the real catalogue reader, returns its typed `InvalidRecord`.
Damaged bytes and invalid namespaces also retain the reader's typed errors; none becomes
`DiffersFromPlannedCreate` or `NotVisible`. A legitimate different registered definition
can produce `DiffersFromPlannedCreate`; corrupting metadata under the same pin cannot model that case.

When a Task exists, the existing catalogue read validates its current definition and current referenced
bytes in one transaction. It does not separately reread the attempt's entire historical Artifact
prefix. When the Task is not visible, the existing early return checks the source and Task boundary
without inspecting the Artifact namespace. This is not a whole-workspace health check or proof of
original storage incarnation. A match may have been created by another writer at the allocated identity.

## State and recovery limits

Observation borrows the attempt immutably. A match, difference, invisibility, read error or cancellation
does not append confirmations, mark the Task complete, clear the unconfirmed target or allow `advance`.
Separate observations have separate read times; a pending writer may finish after `NotVisible`.
The metadata report is private, not a receipt, idempotency key or recovery permission.

The catalogue attempt remains caller-owned memory tied to its original Session. It still has no
checkpoint, journal, restore, retry, cleanup or conversion into the manual C2-P–W types. The old
[C2-Q observation](session-task-draft-observation.md) keeps its manual reader and `None` comparison;
sharing the report DTO does not share attempt state or publication authority. No HTTP route, issuer,
OpenAPI/SDK/browser workflow, provider assessment, schema migration or deployment connection is added.

## Verification boundary

Catalogue observation cases must separately exercise exact state/token admission, complete definition
and ordered-byte comparison, valid differences versus typed corruption, current Session rejection,
read scope, pending writers, cancellation and unchanged attempt/resource state. Structural guards and
pure checks do not replace disposable database/file execution. Current selection belongs in the
[testing guide](testing-guide.md), with actual dated results and omissions in [project status](project-status.md).

The [capability tensor](capability-maturity.md) extends W16 implementation references only. Its older
manual-observation evidence retains its original date and scope; catalogue sources do not inherit
those results, and no scores, evidence or topology are refreshed.
