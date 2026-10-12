# Current publisher owned file sharing

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API **0.3.0**, **2026-10-12**.
This adapter follows [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2),
[recipient grants](large-file-recipient-grants.md) and [Session publication](large-file-session-publication.md).
A current publisher may issue or revoke recipient grants for its own exact file. This is sharing
metadata, not download admission, byte health, transport or application mounting.

## Explicit configuration and exact selectors

`DurableAuthSource::file_sharing_workspace(publication, publishers, recipients)` freezes a
`SessionFileSharingWorkspace` from a trusted `FileUploadPublicationStore`,
`SessionFilePublisherGrantWorkspace` and `SessionFileRecipientGrantWorkspace`. All three must have
the exact same workspace/authority and distinct restricted namespaces. Construction does no IO,
installation, migration or role conversion. The source transaction uses the configured metadata,
not independent store pools or request-selected installations.

`SessionFileRecipientGrantIssue` contains `grant_id`, `publisher_grant_id`, `upload_id`,
`expected_file`, `recipient` and `validity`. These typed values are selectors, not permission or issuer
attribution. `expected_file` must equal every field of the version projected from the selected trusted
Published manifest: workspace/authority, file ID, version ID, byte length and SHA-256. Matching a digest
alone is insufficient. Neither the request nor a copied old grant selects a publisher account.

The command derives both issuer and publisher from the current audited active Human/member and
exact account generation. Teacher/deployment status, a configured publisher-grant manager or merely
owning a file cannot substitute for active publisher permission when creating a share. This is not
a global file manager or delegation policy. The old trusted recipient store still accepts independent
issuer/publisher attribution and remains separately callable by trusted code.
The ledger has no authenticated-origin flag; issuer/publisher equality alone does not prove Session issuance.

## Issuance in one authorized transaction

`issue_session_file_recipient_grant(token, workspace, issue)` uses one source-owned READ COMMITTED
transaction with a ten-second outer bound, retaining the configured READ ONLY policy:

1. Cheap token, source and selector-scope checks precede DB work. Current audited identity determines
   the actor. An own current Active publisher grant and fresh database time permit new sharing.
   Its complete selected history and original three-table SHARE fence remain held.
2. Borrowed Published selection filters by the exact actor/account before decoding. Original
   installation, relations, full private row and fingerprint remain retained with marker/row
   `FOR SHARE` locks. A borrowed record view is not a publication acknowledgement.
3. All expected version fields must equal the server-derived version. The intended recipient must
   currently be an audited active Human/member with the exact source account generation.
4. The recipient writer acquires SHARE ROW EXCLUSIVE directly, records issuer/publisher as the
   actor, validates the bounded whole ledger and retains the original record/history/expected delta.
5. The caller restores synchronous commit, flushes `SET CONSTRAINTS ALL IMMEDIATE`, rechecks the
   original mutation and current recipient, then rechecks original Published and publisher-grant
   records. Final source/Registry/actor checks precede one fresh clock sample covering BOTH Session
   expiry and the original publisher permission window. No later SQL precedes COMMIT.
6. Only source COMMIT acknowledgement returns `StoredFileRecipientGrant`. No helper independently
   begins, commits, changes transaction policy or turns metadata into byte access.

New recipient validity is independent from the publisher permission window; it can be future or
expired metadata. Publisher expiry/revocation does not automatically end existing recipient grants.
The permission sample is immediately before requesting commit, not at acknowledgement or continuous
authorization of a future download. This command reads no files and does not inspect current chunks,
root health or recreate the original publisher's byte receipt.

## Stopping an existing share

`revoke_session_file_recipient_grant(token, workspace, id, expected_revision)` derives the current
actor and matches it to the stored grant's original publisher/account inside the initial write fence,
before any UPDATE. It never obtains a read SHARE fence and then upgrades it to a write fence.
The original issuer may differ from the publisher and remains unchanged.

Only the revision-1 Active to revision-2 Revoked CAS transition is allowed. Missing/foreign grants
return PermissionDenied without distinguishing their existence; stale CAS on an owned grant returns
Recipient Conflict. The full original record/history and whole expected ledger are rechecked after
deferred work, then current Session authorization and COMMIT acknowledgement complete the command.

Revocation does not read publisher-grant or Published storage and does not require their runtime
schema, active permission, byte health or the recipient's continued existence/membership. Thus a
publisher whose publishing permission has been revoked can still stop its old shares. The publisher
itself must still be a current audited active Human/member with the exact original account generation.
Recreated accounts cannot inherit this authority; an operator acting for a deleted/retired publisher
needs a future explicit policy, not implicit teacher authority.

## Failure and delivery boundaries

`SessionFileSharingError` distinguishes Session, Publisher, Recipient, Publication, InvalidNamespace
and PermissionDenied. Ineligible publisher permissions, missing/foreign publications or mismatched
version selectors reject issuance. Current recipient lifecycle, canonical history, original OIDs,
installation, quotas and complete allowed deltas retain their independent typed failures.

Both commands preserve retained quotas, permanent grant-ID conflicts and immutable bindings. They
add no adoption, renewal, replay success, automatic retry, cleanup or state restoration. Definite
pre-commit errors can roll back; cancellation, timeout or lost acknowledgement is not rollback proof.
There is no Ready/Unknown publication attempt state in this metadata-only command.

`session_file_sharing_tdd` uses closed-pool configuration controls and opt-in real PostgreSQL cases.
`scripts/test-session-file-sharing.sh` selects each compiled ignored case exactly; `fileSharing`
common guards check composition/inventory, not actual locking or authority. Test setup may publish
real bytes through the old publisher to establish a trusted record; sharing itself does no file/byte IO.

No shared collaboration model or table schema, startup installer, public Session issuer, HTTP, OpenAPI/SDK/UI, deployment,
live-auth cutover, download command, transport, pairing or restart recovery is added. Download admission
must separately validate the current recipient, exact trusted Published version, Active grant and fresh
time, then bound actual byte-work admission and in-flight revocation. Dated maturity evidence is unchanged.

The separate [download-manifest observation](large-file-download-manifest.md) now connects a current
recipient's own Active grant to exact Published metadata and final fresh expiry. It returns no bytes
or reusable permission and does not change this sharing adapter or its original execution record below.

## Recorded execution · 2026-10-12

The exact baseline compiled, established a real 17-byte Session publication, then failed on the
unimplemented sharing command returning Recipient `UnsupportedSchema`: **0 passed / 1 failed** after
fixture cleanup. This was behavioral Red, not a missing symbol. The implemented baseline then passed
**1 / 0**; both runs had zero compiler warnings. An early inventory guard ran before final test/runner
wiring and failed; that composition check was not counted as SQL behavioral Red. The wired focused
sharing/recipient guards subsequently passed **10 / 0**.

The first complete sharing matrix passed all ten SQL cases. Independent review then tightened test
precision without changing production or inventory: successful head/history rows must identify the
target grant and canonical payload; issuance expectations come from the original command and server
actor, and revocation preserves the immutable tuple and issuer captured before the command. The full
strengthened matrix again passed all ten cases with zero failures and warnings.

Final explicit PostgreSQL 16 runs used a fresh private Unix socket instance with no TCP listener or
ambient application database. Each runner checked its compiled exact ignored inventory before execution:

| Runner | Passed SQL cases |
| --- | ---: |
| `test-session-file-sharing.sh` | 10 |
| `test-file-recipient-grants.sh` | 12 |
| `test-session-file-recipient-grants.sh` | 7 |
| `test-file-publisher-grants.sh` | 12 |
| `test-session-file-publisher-grants.sh` | 7 |
| `test-session-file-grant-management.sh` | 10 |
| `test-session-file-metadata.sh` | 10 |
| `test-session-file-publication.sh` | 10 |
| `test-file-upload-publication.sh` | 13 |

These **91 distinct explicitly executed SQL cases** passed with zero failures and warnings; repeated
baseline/matrix runs were not counted twice. New sharing defaults separately passed 3 cases with 10
ignored. The complete backend quality gate passed **853 default Rust cases**, with **810 ignored cases
separately accounted for**, across 82 result groups including three compile-fail doc tests. All **271
common guards** passed, with zero compiler warnings. Ignored inventory was not treated as executed SQL.

Actual database blockers, not elapsed time alone, establish queued quota/CAS writers and late-expiry
waits. Deferred faults verify rollback without automatic retry. Pre-COMMIT cancellation plus an
independent drained rollback observation is not a general lost-acknowledgement simulation. Tests show
fresh pre-commit authorization and synchronous-commit settings, not continuous download permission,
power-loss durability or acknowledgement-time expiry checks. Source reviews found no additional
substantive production defect; the documentation review clarified file/byte IO versus database IO.

All 106 historical status sections, 22 pre-existing detailed execution tails and the dated machine
capability-tensor SHA-256 were preserved. No private test namespaces or other database clients remained.
The fresh PostgreSQL instance was stopped and its reproducible data/socket removed (about 165 MiB);
verification logs were retained. No application database, product/API version, commit or push changed.
