# Authorized verified file chunk snapshots

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API **0.3.0**, **2026-10-12**.
Following [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), a current recipient can
obtain one privately verified chunk of its exact trusted Published file. The command connects
[manifest eligibility](large-file-download-manifest.md) to actual bytes, not HTTP or client delivery.
It returns an immutable read-time snapshot; it does not promise continuous authorization or that the
filesystem remains unchanged after verification. Later revocation cannot reclaim already returned bytes.

## Configuration and request

`DurableAuthSource::file_chunk_read_workspace(downloads, staging, capacity)` creates an inert
`SessionFileChunkReadWorkspace` from trusted `SessionFileDownloadWorkspace` and already-open
`ChunkStaging`. Authority/workspace must agree. The capacity is 1–16; clones share the budget, while
independently constructed workspaces have independent budgets. Construction installs nothing and
opens no database connection. Opening staging separately still validates an existing private root.

`SessionFileChunkReadRequest` holds `download: SessionFileDownloadRequest` and `chunk_index: u32`.
The request cannot supply recipient, publisher, paths, source/root keys or permission. Teacher,
deployment, manager, publisher and owner status do not bypass an explicit own recipient grant.
An empty file has no readable chunks. An index at or above 16,384 fails before database work; other
indexes are checked against the server-selected exact authorized manifest before file work.

## Authorization file work and disclosure

`read_session_file_chunk(token, workspace, request)` performs these separate phases:

1. Cheap token, configuration and request-scope checks precede fail-fast response admission and SQL.
   An initial source-owned READ COMMITTED transaction derives the current audited active Human/member
   and account generation. It retains the original own Active recipient grant, complete history,
   fingerprint and three-table SHARE fence. Stored publisher/account and exact upload/file/version
   filter Published metadata before decoding. The full version must match the grant and request.
2. The originally selected publication retains schema/namespace identities, nonce, full manifest,
   fingerprint and hidden bindings. Its source/root keys must bind to the originally opened staging
   root. Original records and identities are rechecked. One final database clock covers BOTH Session
   expiry and the grant window before initial COMMIT acknowledgement. An invalid current window
   cannot dispatch file work.
3. With no SQL connection or locks retained, a private blocking job reads only the fixed bound `u1`
   entry for the selected manifest/index. It verifies root identity, non-following descriptor access,
   regular file type, current owner, exact 0400 mode, single link, bounded length, exact EOF and chunk
   SHA-256. Pre/post descriptor and visible entry snapshots plus a final root check reject observable
   replacement. At most **4 MiB** enters a private buffer; unverified prefixes never reach the caller.
4. A fresh source-owned READ COMMITTED transaction resumes the SAME original Session/source/Registry
   identities and refences the SAME original recipient and publication observations, including original
   table identities, history, nonce and hidden keys. No changed record or matching replacement is
   adopted. All record checks precede a final same-clock Session/grant validity sample, then COMMIT.
   Only after its acknowledgement can `SessionFileChunkRead` expose `file()`, `chunk_index()`, `chunk_sha256()` and
   borrowed `bytes()`. No filesystem work or SQL follows this final authorization phase.

Both database phases are bounded to ten seconds each and retain configured READ ONLY policy; they do
not force READ WRITE. This is not a ten-second whole-operation or file-job deadline. Grant changes
during the gap fail original-record verification, rather than silently adopting a fresh grant.

## Resource and integrity limits

The staging worker slot covers the actual queued/running blocking job, even when its async waiter is
cancelled. The independent response slot moves into that job, then the private buffer and public
move-only result, and stays occupied until the result is dropped. The result has no Clone, serializer,
public constructor or consuming byte-vector getter; Debug redacts byte content. A trusted host can
still copy borrowed bytes. These local limits are not a process RSS, global bandwidth or host-wide
budget; independently constructed staging/workspace instances remain additive.

This read verifies ONLY the selected chunk digest, not a fresh whole-file digest or historical write,
fsync, publication acknowledgement or restart proof. It does not query current upload intent, chunk
journal or publisher permission/lifecycle. Those stores do not become extra recipient authority.
The original trusted Published binding is still required; publisher revocation does not cascade to
independent recipient grants. Original file metadata is not claimed to remain current at commit ACK.

`SessionFileChunkReadError` preserves typed `Metadata(SessionFileDownloadError)` errors and adds
`InvalidInput`, `CapacityExhausted` and `Unavailable`. A failed or cancelled call exposes no bytes,
does not repair files, write metadata, release durable upload quotas, retry or adopt outcomes. Dropping a waiter
does not kill an already-started blocking job or prove SQL rollback.

## Transport remains separate

This source command itself connects no public listener or HTTP response. The separate
[binary HTTP adapter](large-file-chunk-http.md) now provides an unmounted explicit GET and retained
byte-owner budgets. OpenAPI/SDK/UI, client saving, resumption, desktop pairing and live-auth cutover
remain disconnected; a cached manifest is never a later read permit.
Dated capability-tensor scores and previous execution records are unchanged.

The explicit `session_file_chunk_read_tdd` target and `scripts/test-session-file-chunk-read.sh` use
disposable PostgreSQL and synthetic files. Private file-read unit tests exercise real blocking-worker
lifetime; `fileChunkRead` common guards only check source composition, documentation and inventory.

## Recorded execution October 12 2026

The compiled behavioral baseline first failed after real Published bytes and Session sharing were
prepared: **0 passed, 1 expected `Unavailable`, 0 warnings**. The fixture cleaned its synthetic files
and schemas before the behavior assertion. The initial compile-only pass had one unused test import,
removed before that baseline. A subsequent Green compile found the nested clock helper's visibility
too narrow; its Source-only visibility was corrected before the successful Green baseline:
**1 passed, 0 failures, 0 warnings**. Socket permission denials were infrastructure failures, not Red.

The exact `test-session-file-chunk-read.sh` inventory executed **10 distinct SQL cases**, all passing
with zero failures and warnings:

| Module | Exercised boundary |
| --- | --- |
| baseline | Real recipient bytes/version/index; held result exhausts capacity, drop restores it; unchanged metadata/files |
| behavior | Actual 4 MiB plus 17-byte file and empty file; exact local indexes; corrupt/missing OTHER chunk cannot deny the healthy selected chunk |
| authorization | Missing, expired, future, revoked, foreign-generation, teacher/publisher and self-without-grant rejection; different independent issuer positive |
| lifecycle | Current Human, audited active membership, exact Session/account generation and logout/retirement/recreation checks |
| filesystem | Missing, corrupt, short/long, linked, FIFO/special, wrong mode/root and legacy/unbound files; no byte DTO, repair or adoption |
| configuration | Shared-clone versus independent response capacity; READ ONLY and configured isolation/durability settings; closed detached pools and deleted current upload/journal rows |
| records | Selected canonical recipient history/actor and publication body/hidden-key corruption; foreign damaged rows remain filtered |
| gap | Independent writer after first actual source COMMIT; original source/Registry/recipient/publication relation replacements, grant revocation, logout/suspension, nonce and all three stored hidden keys |
| waits | Actual post-file final marker wait straddles database-clock Session/grant expiry; buffered bytes keep capacity but never escape |
| cancellation | Actual final waits at five selected marker/head/history/publication points; cancelled buffer and SQL fences drain, snapshot stays unchanged and healthy read recovers |

The source-pool `after_release` gate proves a real initial commit and lock-free metadata gap; it is not
a file-worker hook. Actual filesystem job barriers are tested independently. Final SQL waits are
confirmed using PostgreSQL blocking PIDs, not an assumed elapsed dispatch delay. Snapshot/delta checks
cover source/Registry, both grant ledgers, upload/journal/Published rows and table OIDs, plus root and
staged-file bytes/identity; failed matrix assertions still clean their synthetic fixture before rethrow.

The ten adjacent runners were also freshly executed: recipient ledger 12, Session recipient 7,
publisher ledger 12, Session publisher 7, grant management 10, Session metadata 10, Session publication
10, trusted publication 13, sharing 10 and download manifest 10. Together with the new reader these
are **111 distinct explicitly executed SQL cases**, zero failures and warnings. The separate baseline
rerun and ignored inventory are not extra executed cases.

After PostgreSQL stopped, the backend-only quality gate passed **866 default Rust cases** across
**84 result groups**, with **830 ignored cases separately accounted for**, and **281 common guards**;
zero failures and compiler warnings. Its default coverage includes the four actual file-job tests
(queued cancellation, running cancellation, completed/unclaimed result retention, and in-job digest/
same-byte entry/root replacement) and three new compile-fail contracts rejecting Clone, consuming
byte extraction and serde serialization. Those seven are included in the 866, not in the SQL count.
The first full gate stopped at one outdated streaming guard (**280/281 passed**) that forbade any
staging `Vec<u8>`; the update allows only this private leased checked buffer, its dedicated worker and
cfg-test module. All other staging paths retain the ban. The complete final gate passed after this
narrow correction; no production change was needed after the SQL matrix passed.

Bilingual sources and generated course copies are synchronized. All **106 historical status blocks**,
**26 pre-existing detailed execution tails** and the dated machine capability-tensor SHA-256 remain
unchanged. The isolated database had no test namespaces or other clients before stopping; its
reproducible data/socket were removed (**186,104 KiB**, about **182 MiB**). Logs remain under
`/tmp/cyanrex-file-chunk-read-dhM8T9R7`. No application database, product/API version, commit or push changed.
