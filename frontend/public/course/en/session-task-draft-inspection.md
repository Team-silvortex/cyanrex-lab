# Explicit inspection of checkpoint targets

Status: **C2-S internal preparation, included in 0.5.3**. Decision date: **2026-10-07**.
This Unix-only operation compares one reported unconfirmed target from a
[C2-R checkpoint](session-task-draft-checkpoint.md) with currently authorized storage. It needs no
surviving publication attempt, but the caller must supply a trusted workspace and a currently valid
Session. A comparison does not authenticate checkpoint history, recover an attempt or confirm a write.

## Explicit inputs and current authority

`source.inspect_task_draft_checkpoint(token, &trusted_workspace, &checkpoint)` is a separate operation
after parsing. The checkpoint must report `unconfirmed`; its scope must agree with the explicit
workspace and source authority before any read. Its reported count selects exactly one Artifact
reference or the Task, as defined by C2-R. The checkpoint cannot select a database, namespace or path.

The return type is `Result<SessionDraftCheckpointObservation, SessionDraftCheckpointInspectionError>`.
Admission errors are `NoUnconfirmedStep` or `ScopeMismatch`; reader failures retain the existing
`Artifact(SessionArtifactError)` or `Task(SessionTaskError)` variant.

The existing Session reader independently checks the supplied token, current account and audited
membership, owner access, namespace and final Session validity. Checkpoint fields are not credentials
or permission. They contain no original Session fingerprint, owner or source incarnation, so this
operation cannot establish that the current caller was the original publisher. A different valid
Session is evaluated under its own current authority, not adopted as the original one.

This differs from [C2-Q](session-task-draft-observation.md), which still requires its surviving opaque
attempt and original Session fingerprint. C2-S neither restores that attempt nor bypasses its stopped
state. C2-R parsing and `reported_unconfirmed_step()` remain pure; neither triggers inspection.

## One read and a metadata comparison

The operation calls exactly one existing C2-F `read_session_artifact` or C2-M
`get_session_content_task`. It introduces no direct SQL or detached authorization check. The result
contains the selected `step` and `result: SessionDraftCheckpointObservedOutcome`, not content bytes:

| Outcome | Meaning at this read |
|---|---|
| `MatchesCheckpointMetadata` | The authorized target matches the checkpoint metadata and expected create profile |
| `DiffersFromCheckpointMetadata` | The readable target does not match those expectations; a later legitimate Task edit can cause this |
| `NotVisible` | The existing authorized reader returned no visible target |

Existing reader errors retain their types, including corrupt or digest-mismatching content. They are
not converted into `NotVisible`. An absence is not proof of rollback, no in-flight writer or safe retry;
different calls observe different times. Another writer may have produced matching data at those IDs.

Artifact inspection reads the allocated exact revision, possibly a historical revision after its head
has advanced. A match requires the exact reference and declared length, checkpoint title, sequence 1,
no parent, kind `cyanrex.task-text` and media type `text/plain`. Text validation checks actual digest,
length, UTF-8, allowed controls and the 256 KiB text limit. Filename and language are manifest labels,
not Artifact-store facts; they do not participate in the individual Artifact comparison.
Readable generic binary or disallowed-control content fails this text comparison as
`DiffersFromCheckpointMetadata`; reader corruption errors remain typed failures.

Task inspection reads the current Task snapshot and referenced contents in one existing transaction.
It compares the exact Task reference, full ordered manifest, manual Draft revision 1 and each returned
content's create metadata, length, validated text and agreement with the current Task owner. That owner
comes from the authorized reader, not the checkpoint. It does not separately audit the reported confirmed prefix.
If the Task is missing, the existing early return checks source and Task boundaries but skips the
Artifact namespace. An existing empty Task still follows the present-Task path and its namespace checks.

No original draft text is available. Digest/length and current metadata agreement are not a byte-for-byte
comparison with the original plan, proof of who wrote it or proof of the original storage incarnation.
Namespace pins protect each read transaction, not privileged same-name clones installed before it.

## Limits and unchanged state

Inspection performs no business write, but its existing readers still acquire locks and commit;
it is not a SQL `READ ONLY` mode. Their current deadlines and lock limits remain in force. The generic
Artifact reader may first materialize up to its 1 MiB read limit before applying the stricter text
contract: 256 KiB is not a total inspection-memory bound or a process-wide concurrency quota.

Success, difference, absence, error and cancellation do not change the checkpoint, confirm history,
adopt a target, mark a Task complete, recreate an attempt, retry a write or delete content. This is a
single-target read, not a whole-workspace audit or a durable outcome protocol. Checkpoint and result
metadata may be sensitive; keep both private and keep credentials and content out of logs.

There is no checkpoint save/load, fsync, intent journal, migration, public route, browser connection,
Session issuer or live-source installation. A caller may separately retain metadata, but C2-S neither
guarantees that retention nor verifies the original caller after restart. Durable intent, recovery
identity and retry policy still require their own contracts.

## Verification boundary

Default tests and explicit isolated PostgreSQL cases separately cover state/scope admission, current
authorization, precise Artifact/Task comparisons, typed failures, missing versus empty Tasks and
unchanged business data after reads or cancellation. The exact SQL entry is
`scripts/test-session-task-draft-inspection.sh`, selecting `session_task_draft_inspection_tdd`.
Actual dated results belong in [project status](project-status.md); earlier C2-P/Q/R evidence retains
its own scope and is not execution evidence for this separate entry point.
