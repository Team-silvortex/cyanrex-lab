# Session authorized draft publication

Status: **C2-P internal preparation, included in 0.5.3**. Decision date: **2026-10-07**.
This Unix-only create workflow publishes the text items in a [C2-O plan](task-draft-publication.md) through
existing current-Session Artifact commands, then creates a private manual Draft through
[C2-M](session-task-content.md). The caller explicitly advances one step at a time. It is not a
whole-draft transaction, public upload route, browser save adapter or live-authentication switch.

## Preparation and ownership

`DurableAuthSource::prepare_task_draft_publication(token, workspace, plan)` returns an opaque
`SessionTaskDraftPublication`. Preparation performs no database or file I/O: it fixes the source
handle and trusted `SessionTaskContentWorkspace`, validates the canonical token shape, and allocates
the complete set of random UUIDv4 Task/Artifact/revision IDs and expected content digests. IDs are
not derived from local editing IDs, filenames or text hashes. The title and text limits remain C2-O's.

The attempt stores only a private domain-separated Session fingerprint, not the raw token. The caller
supplies that token again to `advance(&mut self, token)`. A different token is rejected before marking
a ready step as dispatched and leaves it Ready; matching the fingerprint is not authentication.
Every actual write independently uses the existing current-Session command and its transaction checks.
The attempt cannot change source, workspace or plan between steps and does not expose a caller-supplied owner.

The attempt has no `Clone`, `Deserialize` or `Debug` implementation. Its read-only accessors expose
`state`, `task_ref`, `allocated_artifacts`, `confirmed_artifacts` and `confirmed_task`; they do not let a
caller inject confirmations or restore an attempt from a snapshot. Accessors expose historical
metadata, not current authorization; keep it private and omit draft text and tokens from logs.
Authentication material belongs outside task records and exports.

## One step at a time

| State or result | Meaning |
|---|---|
| `Ready` | The next step may be explicitly advanced with the original Session token |
| `Unconfirmed(Artifact { index, reference })` | This exact publication has been marked before its first await; confirmation is not available |
| `ArtifactConfirmed { index }` | The existing Artifact command returned the expected confirmed metadata; it has been appended synchronously and the attempt is Ready for the next step |
| `Unconfirmed(Task { reference })` | The final create operation has been marked before its first await; Task confirmation is not available |
| `TaskConfirmed` and `Complete` | C2-M returned the expected confirmed Task/manifest metadata, retained in the attempt; further advances are refused |

Each advance dispatches at most one existing authorized write. Artifact publications remain separate
transactions; the last Artifact step does not also create the Task. Even identical text items retain
distinct allocated references. This first workflow creates fresh content and a fresh Task only: it
does not reuse supplied revisions, revise an Artifact or replace an existing Task.

Confirmed revision metadata plus the attempt's original text are passed to C2-O's pure binding check.
That is consistency checking, not an additional file read. C2-M independently rereads the actual blobs
and rechecks exact references, owner, namespace and current Session in its own transaction before
confirming the Task. An empty plan creates an empty manifest directly; C2-M derives the real owner,
without a fabricated Principal or dummy Artifact.

## Cancellation and partial outcomes

The caller must keep the attempt outside the future it may cancel. A step marks `Unconfirmed` before
awaiting the existing command; successful confirmation is recorded without another await. Dropping
an unpolled advance does not dispatch work. Dropping it after dispatch, or receiving a dispatched error,
retains the confirmed Artifact prefix and the exact unconfirmed step, and blocks further advancement.
`Stopped` does not mean rolled back. Existing errors remain typed, but even a conflict is not treated
as evidence that a prior attempt succeeded or that retry is safe.

No operation deletes published content, undoes prior transactions, retries an unknown step or repairs
an attempt. Files may remain after failed publication. A failed final Task create does not revoke its
confirmed Artifact publications. The original Artifact/C2-M command deadlines and lock limits remain
per-command controls, not one workflow deadline or a bound on process memory, concurrent attempts or
durable storage.

An unexpected returned snapshot is `InvalidConfirmation`, not accepted progress. The attempt retains
its unconfirmed step and cannot advance; this check does not reverse a command that may have committed.

Progress is only in memory. If the caller destroys the entire attempt, aborts a task that owns it, or
the process exits, this bookkeeping is lost. Read-only snapshots are not a durable receipt, idempotency
key, recovery protocol or proof of exactly-once creation. A separately created attempt can create
duplicate logical work; this slice provides no automatic outcome reconciliation.

0.5.3 [C2-Q](session-task-draft-observation.md) adds an explicit read-only observation of the
unconfirmed target using the same attempt and original Session. A current match, difference or invisible
result never appends confirmation, unlocks this state or permits retry. The earlier C2-P evidence below
does not cover that later read operation.

0.5.3 [C2-R](session-task-draft-checkpoint.md) exports bounded checkpoint metadata without text or
authentication material. It does not save data or deserialize this attempt. Parsed progress is a
caller claim, not a confirmed receipt; stale snapshots do not permit continuation or retry.

## Remaining boundaries and verification

The [C2-N router](task-content-http.md) remains unmounted and does not invoke this workflow or gain
an 8 MiB upload allowance. No Session issuer, CLI, AppState wiring, schema migration, deployment
authority, public OpenAPI or SDK surface is added. Browser saving, authenticated installation and
outcome recovery remain separate work. Teaching runtime and local draft behavior are unchanged.

On 2026-10-07, 14 default preparation/confirmation/cancellation units, 12 exact disposable PostgreSQL
cases and three common guards passed. SQL cases cover empty and multi-item success, token mismatch,
unpolled advances, partial publication, cancellation, final commit failure, conflicting targets,
revoked/expired authority and changed blobs. These are scoped runs, not browser or deployment acceptance.
Dated results
belong in [project status](project-status.md), with exact selection in the [testing guide](testing-guide.md).
The [capability tensor](capability-maturity.md) gives this internal composition its own coordinate;
it does not raise the missing browser-save connection score.
