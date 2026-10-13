# Recipient manifest HTTP

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-12**.
This follow-up is outside the published 0.5.7 tag and follows
[proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2). It projects the existing
[current-recipient manifest command](large-file-download-manifest.md) through an explicitly built,
unmounted HTTP router. The response is private metadata, not byte health or reusable transfer authority.

## Trusted composition and exact GET

`platform_http::FileDownloadManifestHttpState::{new, with_capacity}` accepts an existing
`DurableAuthSource`, `SessionFileDownloadWorkspace` and canonical trusted browser Origin.
`build_file_download_manifest_router(state)` constructs only this independent router. The cached
`workspace.file_scope()` is crate-private and comes from its original publication store; a request
cannot choose the authority, workspace, owner, recipient, storage namespace or filesystem root.
Construction installs nothing, performs no SQL/file IO and issues no Session.

Only exact **`GET /platform/v1/file-manifests`** is supported. The raw query requires exactly six
selectors: `grant_id`, `upload_id`, `file_id`, `version_id`, `byte_length`, `sha256`.
Order may vary. Missing, duplicate, unknown, empty or malformed fields fail before admission/SQL.
Raw queries are at most **512 bytes**, with no percent encoding, plus signs or extra equals signs.
IDs are non-nil canonical lowercase hyphenated UUIDs; length is canonical ASCII decimal within
**64 GiB**, and the digest obeys the shared lowercase SHA-256/version contract. Empty length requires
the actual empty digest. `chunk_index`, supplied scope/owner, paths, Range and all `if-*` selectors
are rejected rather than ignored. The six values identify a target; none establish permission.

Other paths are private 404; other methods are 405 with `Allow: GET`. HEAD remains a refusal with an
empty body and length zero. Endpoint/query checks precede credentials and admission, without body polling.
The existing credential policy requires `x-cyanrex-platform-request: 1` and one canonical non-nil
`cyanrex_platform_session` cookie. GET may omit Origin; a supplied Origin must exactly match the
configured browser Origin. Authorization headers, duplicate target cookies and invalid credentials
are rejected. This is not CORS, a Session issuer or permission derived from a transport hostname.

After admission, the actual request must be empty. The shared **two-second** collection deadline,
media/encoding checks, trailer/error refusal and cooperative empty-frame handling remain unchanged.
Declared Content-Length is not trusted as evidence of actual empty EOF.

## Current observation and JSON response

Exactly one `get_session_file_download_manifest` call owns current recipient/account derivation,
exact independent Active grant and Published record selection, original source/Registry/record
rechecks and final database-clock Session/grant eligibility. The existing bounded ten-second,
READ COMMITTED source transaction and configured READ ONLY policy are unchanged. Teacher,
deployment, publisher and manager roles cannot replace the exact recipient grant.

Success returns the **direct complete `FileTransferManifest` JSON**, including its existing
`schema_version: 1`, scope/version, inert display name, geometry, ordered chunk digests and whole
digest. It does not wrap a supplied manifest or expose a grant, account, hidden binding, nonce, path
or publication receipt. `Content-Type` is `application/json`; explicit Content-Length is the actual
serialized byte count. The shared **1152 KiB** manifest ceiling is checked after typed serialization.
The underlying **16,384-chunk** and display-name bounds also constrain this private materialization;
the length check is not fallible-allocation recovery, measured RSS or a global memory quota.

Every response uses `no-store`, `no-cache` and `nosniff`. There are no response cookies, redirects,
CORS, retry hints, binary chunk headers, Content-Disposition, ranges, ETags or conditional cache state.
Missing/foreign/revoked, invalid-at-final-time or inexact grant/version targets share private 404
`not_found`. Invalid Session maps to 401; the mapped current-identity denials use 403; selected corruption/configuration/storage
failures remain sanitized 503 rather than becoming an empty or missing manifest.
Current publisher permission and publisher retirement/deletion do not implicitly revoke an independent
recipient grant; the original recorded publisher/account still selects the exact Published record.

The existing schema-1 error envelope distinguishes pre-dispatch `not_attempted` from post-dispatch
`unconfirmed`. It contains static codes, never raw SQL diagnostics, tokens, owners or hidden keys.
Read-only metadata errors still do not establish a general rollback or retry guarantee.

## Response lifetime and remaining connections

HTTP admission defaults to **4**, trusted range **1–16**. State clones, including independently built
routers sharing those clones, share slots through request collection, Source work, serialization and
the unsent JSON response. A full gate returns 503 `capacity_exhausted` / `not_attempted` before body
polling/SQL. Admitted error bodies also retain a slot. Last frame/end, error, body drop or cancelled
request releases it. Independently constructed states have additive HTTP budgets; manifest admission
remains separate from the existing chunk/download gates.

Unlike the binary adapter, this metadata body has no Source byte-worker lease or ten-second first-frame
window. Retaining JSON Bytes after body completion does not retain HTTP admission. Cancellation does
not hard-kill the network/database or prove rollback; no detached timer or continuation is introduced.
A returned snapshot can survive later revocation. Eligibility is the Source's final sampled check,
not body-polling time, delivery acknowledgement or continuous permission for subsequent chunks.

The original [ordered download entry](large-file-download-client.md) still requires a caller-supplied
manifest. A separate [exact manifest client](large-file-manifest-client.md) now fetches this endpoint
and can compose metadata with ordered private assembly. The [binary chunk route](large-file-chunk-http.md)
continues to authorize each later request independently. No file bytes are read or privately staged
by this metadata adapter. Neither route is mounted in normal AppState/application startup.
Session issuance, discovery/listing, public OpenAPI/SDK/UI, user-selected final save, resume/recovery,
live migration/cutover and desktop pairing remain separate. Versions and dependencies are unchanged.

## Verification entry points

The explicit [distribution router](large-file-distribution-router.md) derives metadata configuration
from one supplied chunk workspace and shares its supplied Source, preserving the original handler
and independent JSON budget. The standalone builder remains compatible and unmounted.

`file_download_manifest_http_tdd` covers strict admission with a closed Source and opt-in real
current-recipient SQL. `scripts/test-file-download-manifest-http.sh` checks compiled exact ignored
inventory before executing it. Private contract units cover typed parsing/error/projection boundaries;
`fileManifestHttp` guards cover composition and inventory only, not authority or delivery.
Current bilingual notes/course copies describe this edge without altering **106 historical status
blocks**, **38 old execution tails**, published 0.5.7 validation or the dated capability tensor.

## Recorded execution 2026-10-12

The inert API scaffold compiled against an actual 17-byte Session publication and independent
current role-free Human recipient grant. After complete fixture cleanup, behavioral Red was
**0 passed / 1 expected failed**, actual HTTP **503 instead of 200**, zero warnings. Green reran
that exact case: **1 passed**, zero warnings, complete direct manifest JSON, actual Content-Length,
fixed private headers and unchanged Source/Registry/grants/upload/journal/Published/file snapshots.
The initial fixture attempt had `E0425` missing root scope and an unused reexport warning; importing
the actual nested fixture scope fixed both. That setup failure is not behavioral Red.

| Fresh selection | Actual result and scope |
|---|---|
| Default manifest HTTP target | **12 passed / 8 opt-in ignored**, zero warnings; closed Source, canonical path/method/HEAD/query/credentials, actual empty-body/frame/error/two-second deadline, default-four clone sharing, independent state budgets, full-gate precedence, cancellation, JSON last-frame/drop and retained Bytes controls |
| Private contract units | **10 passed**, zero warnings; six typed selectors/trusted scope, canonical lexical and geometry bounds, private error mapping, exact direct JSON/Content-Length, maximum declared 16,384-chunk metadata and post-encoding byte ceiling; no 64 GiB payload execution |
| New exact SQL runner | **8 unique cases passed**, zero warnings; complete 17-byte/empty/4 MiB + 17 manifests, exact current recipient/no role bypass, pre-Source body-wait revocation, current lifecycle, selected corruption and foreign filtering, original Source-only IO, actual lock/clock expiry/arrival, SQL cancellation/drain and unsent/completed/dropped JSON response lifetime |
| Adjacent SQL runners | **10 existing manifest + 6 binary HTTP + 1 ordered-download passed**, zero warnings; with the new runner this is **25 unique fresh SQL cases**, not an additional count for the repeated baseline Green |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **973 Rust passed / 850 opt-in ignored / 90 result groups**, **311 common passed**, zero failures/warnings; the 12 new defaults and 10 private units are already included |

Native focused composition/inventory checks passed **20**. Independent source, test and bilingual
reviews found no remaining actionable defect. Test review retained abort-on-drop ownership through
held future completion, bounded cleanup/recovery probes and full binary-header absence checks.
There is no new frontend/SDK/security-audit, browser, manifest-client fetch, actual metadata network/TLS
deployment, byte-health/64 GiB payload, throughput/RSS, user save/resume or live-auth migration acceptance.
The adjacent ordered-download bridge's real loopback requests do not turn this in-process Axum/SQL
metadata matrix into new network delivery proof. Ignored inventory is not default acceptance, and
the 25 explicitly executed SQL cases are separate from default 973.

All source editors froze before serialized root-owned Cargo execution. Final disposable PostgreSQL 16
audit confirmed the exact owned private data path, no TCP, **zero fixture schemas and zero other clients**.
After stopping, only validated `data`/`socket` were removed, releasing **94308 KiB (about 92 MiB)** of
reconstructible test data. Logs remain at `/tmp/cyanrex-download-manifest-http-5ndLJ7Q5`; no project,
deployment or unrelated cache data was removed. Versions/dependencies, live API/SDK/UI, published
0.5.7 reports, **106 old status blocks**, **38 old execution tails** and the capability tensor remain
unchanged. Bilingual sources/course copies are synchronized; no commit/push.
