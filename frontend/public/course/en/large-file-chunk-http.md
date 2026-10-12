# Private single chunk binary HTTP adapter

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API **0.3.0**, **2026-10-12**.
The explicitly constructed adapter connects one strict GET to the existing
[authorized verified chunk reader](large-file-chunk-read.md). A success exposes one immutable verified
chunk, not a download receipt. The router is **unmounted**: no normal application listener, public
OpenAPI/SDK/UI, Session issuer, client save/resumption, desktop pairing or live-auth cutover is added.

## Trusted composition and exact request

`FileChunkReadHttpState::new(source, workspace, origin)` takes a trusted `DurableAuthSource` and
`SessionFileChunkReadWorkspace`. `with_capacity` accepts 1–16 HTTP slots; the default is four.
Origin configuration follows the existing canonical HTTPS or loopback HTTP policy (localhost or loopback IP). Construction
is inert. State clones share HTTP admission; independent states have additive HTTP budgets. Source
workspace clones separately share the existing verified-byte budget.

`build_file_chunk_read_router(state)` exposes only **GET `/platform/v1/file-chunks`**. Other methods,
including HEAD, return private 405 with `Allow: GET`; HEAD has no body. Unknown paths return private 404.
The raw query is at most 512 bytes and must contain each of these keys exactly once, in any order:

| Key | Accepted value |
| --- | --- |
| `grant_id`, `upload_id`, `file_id`, `version_id` | Lowercase canonical non-nil UUID |
| `byte_length` | Canonical unsigned decimal, at most 64 GiB |
| `sha256` | Lowercase 64-hex whole-file digest; empty length requires the standard empty digest |
| `chunk_index` | Canonical unsigned decimal, 0–16,383 |

No decoding or normalization accepts percent escapes, `+`, whitespace, empty or duplicate fields,
unknown keys, extra equals signs, signs or leading-zero numbers. Local manifest indexes and empty
files are checked by the existing authorized command. Clients cannot select workspace/authority,
recipient, publisher, owner, account generation, namespace, storage root or path. The configured
workspace supplies scope; an exact file reference is a selector, never permission.

## Admission and current authorization

Route/query and unsupported Range/conditional headers are rejected before body polling or SQL.
`Range`, `If-Range`, `If-None-Match`, `If-Match`, `If-Modified-Since` and `If-Unmodified-Since` are not
supported; this is an exact chunk protocol, not arbitrary ranges or 206/304 caching.

The dedicated canonical `cyanrex_platform_session` cookie and `X-Cyanrex-Platform-Request: 1` marker
are mandatory. Supplied Origin must match the exact configured origin; GET may omit it under the
existing native-client policy. Authorization/Bearer headers and query credentials are rejected.
The existing bounded cookie-header policy is unchanged. Cheap checks precede fail-fast HTTP admission.

Admitted GET collection requires an empty body within two seconds. Content-Encoding, nonempty/error/
trailer bodies and unsupported media types are rejected. Pending bodies and endlessly ready empty
frames cannot bypass that collection deadline. Optional Content-Type follows the existing exact JSON
media-type policy; no JSON request body is decoded.

One existing `read_session_file_chunk` call then performs its two original-pinned source transactions
around private file work. It derives the current audited Human/member/account and requires an own
Active exact-version recipient grant. No teacher, publisher or deployment role bypasses this grant.
The final same-clock Session/grant check and source commit acknowledgement precede response projection.
The adapter adds no subsequent SQL, filesystem reads, grant adoption, repair or writes.

| Failure | Private HTTP projection |
| --- | --- |
| Missing, foreign, expired, revoked or wrong exact-version recipient permission | 404 `not_found` |
| Invalid Session or credentials reported by the source | 401 `invalid_session` |
| Inactive or denied current identity/membership | 403 `access_denied` |
| Full source byte budget | 503 `capacity_exhausted` |
| Authorized empty file or local index outside its manifest | 400 `invalid_request` |
| Selected corruption, schema/root/file failure, timeout or configuration failure | 503 `service_unavailable` |

Dispatched failures have outcome `unconfirmed`; cheap/body/HTTP admission failures are `not_attempted`.
Errors contain no raw SQL, hidden keys, paths, account identities or byte prefix. There is no Retry-After,
Set-Cookie or automatic retry. Cancellation is not proof of SQL rollback or termination of dispatched IO.

## Binary response and byte ownership

The separate [controlled client](large-file-chunk-client.md) now requests the explicit route from
a host-frozen origin; native GET omits UI Origin and still uses the original cookie/marker policy.
This does not mount the route or make the whole-file, resumption or deployment workflow live.

The separate [receiver](large-file-chunk-receiver.md) validates supplied responses before local
exclusive staging. It does not mount this router, initiate requests or add whole-file saving/resumption.

Success is 200, `application/octet-stream`, exact Content-Length and a fixed
`Content-Disposition: attachment; filename="chunk.bin"`. Every response has `Cache-Control: no-store`,
`Pragma: no-cache` and `X-Content-Type-Options: nosniff`. No CORS, ETag, Last-Modified or Content-Range is added.

Six fixed metadata headers expose only the selected public version coordinates:
`x-cyanrex-file-transfer-version: 1`, `x-cyanrex-file-id`, `x-cyanrex-file-version-id`,
`x-cyanrex-chunk-index`, `x-cyanrex-chunk-sha256` and `x-cyanrex-file-sha256`.
The chunk digest comes from the original server-selected manifest and was verified against actual
bytes. The whole-file digest is declared metadata, **not a fresh whole-file rehash**. User filenames
and private binding keys never become response headers.

The move-only source result and HTTP permit enter a private zero-copy `Bytes::from_owner` owner.
Both slots remain occupied while an unpolled response, completed/unclaimed response, data frame or any
Bytes clone/slice retains that owner. End-of-body alone does not release a slot while handed-off data
survives. Dropping the last owner restores both budgets. This is stricter than the existing JSON
admission wrapper, whose slot ends at terminal frame/end/error/drop; admitted JSON errors retain that
existing behavior. Neither interface enforces a global RSS, bandwidth or proxy-buffer quota. A trusted
consumer can still deep-copy bytes outside the managed owner.

A **ten-second first-frame deadline** starts immediately after the source returns. A late first poll
emits a static body error, no data frame, and drops both leases. It is not an autonomous timer: an
expired unpolled body holds its leases until poll/drop. After handing off the data frame, its Bytes
owner remains governed by lifetime, not a delivery timeout.
The response is already constructed as 200 with Content-Length; a late body error does not replace
it with JSON or 503, and headers may already have been sent. It is not a completed chunk response.

Authorization is the final source snapshot, not a fresh check at first poll or per frame. Later
revocation cannot reclaim approved bytes. The first-frame window limits response deferral after
the command; it is not a bound on revocation propagation, socket delivery or remote receipt. The
existing source phase deadlines and actual job-lifetime worker budget remain separate.

## Verification boundaries

`file_chunk_read_http_tdd` covers in-process requests and real response frames.
`scripts/test-file-chunk-http.sh` explicitly executes its disposable PostgreSQL/synthetic-file inventory.
Private parser/body units cover lexical and owner/deadline behavior; `fileChunkHttp` common guards
check composition, documentation and inventory only. None substitutes for mounted TLS/LAN transport,
client persistence, final whole-file verification, resumption or a deployed compatibility review.
Historical execution records and the dated capability tensor remain unchanged.

## Recorded execution October 12 2026

The compiled behavioral baseline used real Published bytes and current Session sharing. Its inert
503 response failed the expected 200 assertion: **0 passed, 1 expected failure, 0 warnings**. The
fixture cleaned files and schemas before that assertion. After implementation, the same baseline
passed **1/1**, zero failures and warnings. Missing symbols or environment denials were not counted as Red.

The default target passed **9 cases**, with its six SQL cases explicitly ignored and zero warnings.
The first sandbox run stopped all nine at private root preparation (`ChunkStaging::open` returned
Unavailable); the unchanged target passed under native user permissions. This was kept as separate
environment evidence, not a reason to relax descriptor/owner checks or change production behavior.
The **nine private units** also passed: five strict raw-query tests and four byte-owner/deadline tests.
Those units use synthetic private owners only; they cannot manufacture the public verified Source proof.

The complete exact `test-file-chunk-http.sh` inventory executed **six distinct SQL cases**, all
passing with zero failures and warnings:

| Module | Exercised boundary |
| --- | --- |
| baseline | Real authorized 17-byte response, all fixed binary/privacy headers and complete unchanged metadata/file snapshots |
| behavior | Actual 4 MiB plus 17-byte file, selected chunk/tail and local invalid index; a missing unrequested tail cannot deny the healthy selected chunk; publisher revocation does not cascade |
| authorization | Publisher/teacher without recipient permission, missing upload/grant, individually wrong file/version/length/hash, expired/future/revoked grants, suspension/logout and healthy restoration; original grant revocation and publication nonce drift across the gap |
| storage | Selected wrong digest, short/long bytes, malformed Published manifest and wrong relation marker; private 503, no prefix/repair, unchanged injected state and healthy restoration |
| response_lifetime | Unpolled/dropped response, real data frame, clone/nonempty slice and completed-unclaimed response retain BOTH budgets; last drop restores them; real 10.1-second late first poll yields error/no data |
| waits | Genuine final publication-marker waits cross Session and grant expiry; same captured workspace still holds its byte permit; cancellation drains locks and permits with unchanged snapshots and healthy recovery |

Snapshots cover Auth/Registry, both grants, upload/journal/Published rows and relation OIDs, plus
root and staged bytes/identity. All new matrix failures clean their synthetic fixture before rethrow.
The first transaction's connection-return gate, the later observed PostgreSQL blocker and the Source
control flow together distinguish the final wait after file work. The generic pool callback alone
does not prove COMMIT instead of rollback and is not a file-worker callback.

Before execution, review corrected a test that checked the original fixture workspace instead of
the independently constructed captured workspace's byte budget. The same captured handle is now
used for that assertion; no production change was needed. Source and HTTP successful first-frame
limits remain independent of actual socket/LAN delivery and remote persistence.

The eleven adjacent runners were freshly executed as well: recipient ledger 12, Session recipient 7,
publisher ledger 12, Session publisher 7, grant management 10, Session metadata 10, Session publication
10, trusted publication 13, sharing 10, download manifest 10 and the original chunk reader 10.
Together with the new HTTP six, these are **117 distinct explicitly executed SQL cases**, zero failures
and warnings. Baseline reruns and ignored listings are not additional executed cases.

The disposable database had no remaining test namespaces or other clients and no TCP listener before
stopping. Only its reproducible data/socket were removed: **219,248 KiB**, about **214 MiB**.
Logs remain under `/tmp/cyanrex-file-chunk-http-vGGaSKEu`; no application database was changed.

After PostgreSQL stopped, the backend-only quality gate passed **884 default Rust cases** across
**85 result groups**, with **836 ignored cases separately accounted for**, and **286 common guards**;
zero failures and compiler warnings. The new nine default HTTP cases and nine private units are
included in the 884, not added again. Four original actual file-job lifetime tests and three
move-only/serialization compile-fail contracts also remain passing within that total.

Bilingual sources and generated course copies are synchronized. All **106 historical status blocks**,
**28 pre-existing detailed execution tails** and the dated capability-tensor SHA-256 remain unchanged.
No product/API version, application mounting, database migration, commit or push changed.
