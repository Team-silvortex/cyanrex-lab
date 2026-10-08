# Session authorized draft intent records

Status: **C2-T internal preparation, included in 0.5.4**. Decision date: **2026-10-07**.
Source version: **0.5.4**.

C2-T explicitly registers immutable publication intent metadata from a real, unstarted
[C2-P attempt](session-task-draft-publication.md), then permits a separately authorized read of that
record. It does not publish content or connect the journal to the original C2-P `advance`. Registration is not evidence
that an Artifact or Task was dispatched or committed, and is not a recovery protocol.

## Explicit installation and registration

The trusted caller constructs `DraftIntentJournal::new(pool, scope, installation_id)?` and invokes
`install_empty_namespace()` for an empty, separate journal namespace. Journal schema **1** is a new
storage format, not a product version or a migration of existing stores. Its installation identifier
must be a configured UUIDv4. No startup hook, installer CLI or ambient namespace adoption is added.

The later [C2-U wrapper](session-task-draft-dispatch.md) requires the separate fresh-only schema 2
installer. Intent registration/read accepts both formats, while the original installer still creates
schema 1. Nothing upgrades an existing schema 1 namespace or enables dispatch through registration alone.

`attempt.register_intent(original_token, &SessionDraftIntentWorkspace)` accepts only the real
attempt's **Ready** state with **zero confirmed artifacts**, using its original Session fingerprint.
It does not accept a parsed checkpoint as a substitute for that attempt. In a current-Session
transaction it derives both the exact `PrincipalRef` and `LegacyAccountId`, rather than trusting a
caller-supplied owner. The transaction pins the current source, identity registry and journal.

The record contains the strict Ready/zero-progress [C2-R checkpoint](session-task-draft-checkpoint.md)
in canonical form, stored as `bytea` with a **64 KiB** bound, together with the bound owner/account
generation and configured source, Task and Artifact namespace names. It contains no draft text,
raw token or Session fingerprint. Titles, labels, references and digests remain private metadata.
Registration does not query the target Task or Artifact stores.

Reads bound every namespace name to 63 bytes before driver decoding, as well as the checkpoint limit.
Both commands verify permanent ordinary tables, exact primary keys and required column shapes;
first-observed table identities are retained and rechecked within the transaction. Renaming a valid
constraint alone does not invalidate it. These checks reject drift, not privileged administrator access.

A repeated target reference is **Conflict**, including a repeat with identical metadata. It does not
adopt an existing record or return idempotent success. Journal writes explicitly use
`SET LOCAL synchronous_commit = 'on'`; a successful acknowledgement is the database's COMMIT boundary,
not proof of host storage configuration, crash recovery or backup durability. A timeout, cancellation
or missing acknowledgement is not proof of rollback and does not permit blind registration retry.

## Reading a retained record

`source.get_session_draft_intent(token, workspace, task_id)` uses current Session authorization.
A newly issued Session may read only when **both the owner and exact account generation match** the
stored binding. Sharing a username or obtaining a teacher role does not substitute for those checks.
The trusted workspace supplies configuration; the record cannot select an arbitrary source or store.

The result is an opaque `SessionDraftPublicationIntent`, not a live publication attempt. Reading
does not observe the Task or Artifact, restore progress, append confirmation, advance a write or retry
anything. C2-Q still requires its surviving original attempt/token, C2-S remains a separate explicit
target inspection, and C2-R parsing still returns caller data rather than authority.

## What the record proves and what remains separate

The journal installation ID identifies the configured journal. Recorded namespace names do not
prove the original physical incarnation of the source, Task or Artifact storage. Neither the record
nor its installation ID supplies protection against a privileged database administrator, authenticated
history across backup replay, or proof of past dispatch and resource commits.

Existing C2-P `advance` still works without a journal. This slice therefore does **not** enforce
durable-before-dispatch ordering. The later C2-U wrapper separately commits an Unknown step before
its resource transaction and couples the marker with the resource SQL under exact bound-identity
checks. It is not merely journal COMMIT followed by the old publication call. That opt-in wrapper
requires the original live attempt; registration/read still grants no automatic retry, cleanup,
migration, restart recovery or attempt adoption. The 2026-10-07 C2-T evidence does not cover C2-U.

No route, `AppState` field, public SDK, provider transport, browser saving or automatic installation is
added. Source/registry formats and the existing Task/Artifact authorization boundaries are unchanged.

## Verification boundary

The implementation lives in [`draft_intent_journal`](../../engine/src/services/draft_intent_journal/)
and [`draft_publication/intent.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/intent.rs).
The `session_task_draft_intent_tdd` target and `scripts/test-session-task-draft-intent.sh` keep
registration/read SQL checks separate from C2-P execution and C2-R's pure codec checks. Admission,
duplicate conflicts, current authorization, exact account generations, namespace pins and uncertain
outcomes need their own evidence; they do not inherit earlier publication tests. See the
[testing guide](testing-guide.md) and dated [project status](project-status.md) for execution scope.
