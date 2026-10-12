# Recipient download manifest observation

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API **0.3.0**, **2026-10-12**.
This follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) connects
[recipient grants](large-file-recipient-grants.md) to exact trusted Published metadata. A current
eligible recipient can read the complete manifest for its granted version, including ordered chunk
digests. The returned manifest is sensitive metadata, not a reusable download permit, current byte
health, a publication acknowledgement or permission to disclose bytes later.

## Trusted configuration and exact request

`DurableAuthSource::file_download_workspace(publication, recipients)` freezes a
`SessionFileDownloadWorkspace` from a trusted `FileUploadPublicationStore` and
`SessionFileRecipientGrantWorkspace`. Scope/authority must match exactly, and both restricted
namespaces must be distinct. Construction does no IO, installation, migration or role conversion.
Only the source-owned transaction is used; independent store pools need not be open.

`SessionFileDownloadRequest` contains `grant_id`, `upload_id` and `expected_file: FileVersionRef`.
These are selectors, not a supplied grant, recipient, publisher, installation or permission. The
recipient is derived from the current audited active Human/member and exact account generation.
Teacher, deployment, grant-manager and file-owner status cannot substitute for a recipient grant;
self-download also requires an explicit grant.

## One current manifest observation

`get_session_file_download_manifest(token, workspace, request)` returns a `FileTransferManifest`
only after one source-owned READ COMMITTED transaction, bounded to ten seconds and retaining the
configured READ ONLY policy:

1. Cheap token/source/request-scope checks precede DB work. Borrowed own-recipient selection filters
   by the current exact recipient/account before decoding; its original schema, complete selected
   history and fingerprint remain held under a three-table SHARE fence and marker/row `FOR SHARE`.
2. The selected grant must be Active and name the complete requested `FileVersionRef`. Its stored
   publisher/account, not the recipient or request, selects the Published owner.
3. Exact upload, publisher/account and file ID/version ID predicates precede publication-body
   decoding. The full version projected from the trusted manifest must also equal the grant/request,
   including authority/workspace, byte length and SHA-256. Original schema, hidden bindings, nonce,
   full canonical manifest and fingerprint remain retained through marker/row locks.
4. Both original pending records and namespace/relation identities are rechecked. Final current
   source/Registry/recipient checks precede one fresh database-clock sample covering BOTH the
   original recipient grant's half-open validity window and exact Session expiry. No SQL follows
   that sample before COMMIT. The manifest escapes only after source COMMIT acknowledgement.

Eligibility is evaluated at that final sample: a future grant may become eligible while waiting;
an expired-at-final or revoked grant cannot return a manifest. This is not freshness at acknowledgement
or continuous authorization after return. Publisher permission expiry/revocation and publisher
retirement/deletion do not implicitly revoke an independent recipient grant. The Published record must
still match its recorded original publisher/account; recreated accounts do not inherit that binding.

The returned immutable manifest has the existing geometry and metadata bounds, including up to
16,384 chunks. It contains no live permission, original file descriptor or restored publication receipt.
The command reads no files and does not query upload/chunk-journal or publisher-grant storage.

## Failures and remaining byte boundary

`SessionFileDownloadError` distinguishes Session, Recipient, Publication, InvalidNamespace and
PermissionDenied. Missing/foreign recipient grants, inactive grants, wrong exact version selectors,
missing/nonmatching publications and invalid final grant windows deny observation. Foreign recipient
bodies and publications outside the granted owner/file/version are filtered before decoding; selected
corrupt metadata remains a typed error rather than being disguised as absence. This is not a
constant-time or complete timing-oracle guarantee.

The old own-recipient and own-publication readers remain metadata APIs with their original policies;
their snapshots cannot replace this current eligibility check. No writes, quota release, cleanup,
retry, adoption, recovery, shared model/table schema, startup installer or live-auth cutover is added.
Cancellation or timeout yields no manifest and is not general proof about COMMIT outcome.

Actual download needs a separate single-chunk reader: private bounded bytes, verified length/digest,
original descriptor/root/source bindings, independently retained worker/response budgets, and fresh
current authorization before disclosure. A generic checked copy may write a prefix before discovering
a bad digest; it must not stream directly to a client as if the whole chunk were already verified.
No byte command, HTTP, OpenAPI/SDK/UI, transport, client save, restart resumption or desktop pairing is
connected here. Dated capability scores and historical execution records remain unchanged.

`session_file_download_tdd` and `scripts/test-session-file-download.sh` cover explicit disposable SQL;
`fileDownloadManifest` common guards check composition/inventory, not real authority, waits or delivery.

The later [verified chunk reader](large-file-chunk-read.md) separately implements private up-to-4-MiB
byte verification and original-authority reentry. It does not reuse this returned manifest as permission
or connect HTTP/client delivery. The manifest-only command and recorded execution below are unchanged.

## Recorded execution · 2026-10-12

The compiled baseline established a real 17-byte Session publication and Session-issued share, then
failed on the missing manifest behavior with Recipient `UnsupportedSchema`: **0 passed / 1 failed**,
with zero warnings, after fixture cleanup. This was not a missing-symbol or zero-test result. The same
baseline then passed **1 / 0**, with one temporary unused-import warning in the new cancellation test.
Removing only that unused import made the final default run **3 passed / 0 failed / 10 ignored**, with
zero warnings. The complete exact SQL matrix then passed all **10 distinct cases**, with zero failures
and warnings; the separate baseline Green was a repeated case, not an eleventh SQL case.

Actual same-publisher publications isolate file ID and version ID separately, with identical real
17-byte content and orthogonal coordinates; both healthy and corrupt foreign bodies are filtered.
Current recipient, account generation, Active state, half-open expiry and selected canonical/history
corruption have independent rejection and healthy controls. Original publisher suspension, retirement,
deletion and publisher-permission revocation do not silently revoke independent recipient access to
the metadata. Closed installer pools and an unavailable configured chunk pathname do not affect the
source-owned manifest observation; they do not prove healthy downloadable bytes.

Real backend blockers establish queued revocation, history append and exclusive DDL writers behind
the original reader. The DDL probes restore original physical tables/OIDs before health controls;
generic table copies are not treated as valid authentication installations. Review moved the unchanged
snapshot before the exclusive writer queue to avoid the fixture blocking its own reader release.
Database time after an actual wait proves Session expiry, grant expiry and not-before arrival at the
final authorization point. Five selected row/marker cancellation waits independently drain locks and
preserve complete metadata/file snapshots. This is not lost-COMMIT acknowledgement, restart recovery,
current byte-health, download throughput or client-delivery acceptance.

Final PostgreSQL 16 execution used a fresh private Unix socket instance, no TCP listener and no
ambient application database. Every runner checked the compiled exact ignored inventory before running:

| Runner | Passed SQL cases |
| --- | ---: |
| `test-session-file-download.sh` | 10 |
| `test-file-recipient-grants.sh` | 12 |
| `test-session-file-recipient-grants.sh` | 7 |
| `test-file-publisher-grants.sh` | 12 |
| `test-session-file-publisher-grants.sh` | 7 |
| `test-session-file-grant-management.sh` | 10 |
| `test-session-file-metadata.sh` | 10 |
| `test-session-file-publication.sh` | 10 |
| `test-file-upload-publication.sh` | 13 |
| `test-session-file-sharing.sh` | 10 |

These **101 distinct explicitly executed SQL cases** passed with zero failures and warnings. After
the private database stopped, the backend-only quality gate passed **856 default Rust cases**, with
**820 ignored cases separately accounted for**, across 83 result groups including three compile-fail
doc tests. All **276 common guards** passed, with zero compiler warnings. Ignored inventory and the
separate baseline rerun were not counted as additional executed SQL cases.

Independent source/test/evidence reviews found no additional substantive production defect. Bilingual
sources and generated course copies are synchronized. All **106 historical status sections**, **24
pre-existing detailed execution tails** and the dated machine capability-tensor SHA-256 remain unchanged.
No private test namespaces or other database clients remained before stopping PostgreSQL; its
reproducible data/socket were removed (about **182 MiB**), with verification logs retained under
`/tmp/cyanrex-file-download-8QpeUo`. No application database, product/API version, commit or push changed.
