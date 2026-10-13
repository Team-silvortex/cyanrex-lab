# Exact manifest client and granted download

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-12**.
This follows [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), outside the
published 0.5.7 tag. It connects the independent [manifest GET](large-file-manifest-http.md) to
the existing [ordered private downloader](large-file-download-client.md). Nothing mounts a normal
application route, issues a Session, changes live authentication or adds a user-facing save action.

## Two explicit entries, one original composition

`FileChunkClient::fetch_manifest(token, grant_id, upload_id, expected_file)` returns a complete
checked `FileTransferManifest`. `fetch_granted_and_assemble` first observes that manifest, then
requests each chunk in order, awaits its actual private staging acknowledgement and assembles in
the same original root. Both share the original trusted composition; standalone metadata uses only
HTTP and the cached scope, with no receiver or file work. The composed entry reuses receiver/staging.
The original `fetch_and_assemble(..., supplied_manifest)` remains compatible and does not fetch
metadata. Its empty-file case still makes zero HTTP requests; the new granted entry always makes
one successful metadata GET before even an empty local assembly. Neither entry provides resume.

Before admission or network IO, the expected workspace must match the cached original staging
scope, and the Session token must be a canonical non-nil lowercase hyphenated UUID. Typed grant
and upload IDs already satisfy their shared contract. The target is the configured origin plus
fixed `/platform/v1/file-manifests`, with exactly six selectors: `grant_id`, `upload_id`, `file_id`,
`version_id`, `byte_length`, `sha256`. Requests cannot choose another URL, scope, owner or path.

Each native GET sends one sensitive `cyanrex_platform_session` Cookie,
`x-cyanrex-platform-request: 1`, `Accept: application/json` and `Accept-Encoding: identity`.
It sends no Origin, Authorization, request body, Range or conditional selector. The configured
transport origin is not browser Origin authority. The existing HTTPS certificate/SAN and optional
public CA checks, explicit literal-loopback HTTP opt-in, no proxy/redirect/retry/decompression,
HTTP/1 and disabled idle pooling remain unchanged. TLS does not establish current grant validity.

## Bounded response, actual message EOF and exact version

Only HTTP 200 is accepted. Non-200 and redirects become static `InvalidResponse`, without trusting
their body or forwarding diagnostic text. At most **64 header entries / 8192 name-and-value bytes**
are accepted; repeated fields count independently. Exactly one of each is required:

| Header | Exact accepted value |
|---|---|
| Content-Type | `application/json` |
| Content-Length | Positive canonical ASCII decimal, at most **1152 KiB** |
| Cache-Control | `no-store` |
| Pragma | `no-cache` |
| X-Content-Type-Options | `nosniff` |

Encoding, range, transfer-encoding, trailer, cookies, redirects, ETag, Last-Modified, Retry-After,
Content-Disposition, all six binary file-transfer metadata headers and every `access-control-*`
header are refused. Other bounded headers supply neither permission nor a filesystem destination.

The actual reqwest response is converted to a framed Body, not pre-collected with a convenience
JSON/bytes API. The collector polls through real HTTP message **EOF**, ignoring size hints and
early end declarations. Short or excess data, trailers, non-data frames and body errors are refused,
including errors after the declared byte count. The wire-byte Vec uses fallible `try_reserve_exact`;
this is not fallible recovery for every Serde/model, frame, Box or transport allocation. Data copies
yield and check clocks in **64 KiB** segments. Ready empty frames yield without extending idle time.
Only nonempty accepted byte progress refreshes idle; total time remains fixed from header completion.
Header waiting and body waiting use the existing separate configured deadlines and post-clock checks.
HTTP message EOF is library framing, not proof of raw TCP suffix or connection EOF.

Raw bytes go directly through the shared strict `FileTransferManifest::parse_json`; an intermediary
JSON Value must not erase duplicate fields. Schema, unknown/nested duplicate fields, UTF-8, display
name, chunk geometry and ordered digest bounds retain that model's checks. The complete
`FileVersionRef::from_manifest` must equal `expected_file`: authority/workspace, file/version IDs,
length and whole digest, not just a few echoed headers. Trailing non-whitespace fails parsing.
Synchronous bounded parsing is checked before and after, not forcibly preempted.

The **16,384-chunk / 64 GiB** limits are metadata declaration limits, not measured payload throughput,
RSS or a global memory quota. Retaining typed metadata after completion does not retain admission.
Matching metadata is not authenticated remote-principal provenance, a delivery receipt, durable byte
permission or a continuing grant. Each subsequent Source chunk request still authorizes independently.

## Admission, cancellation and preserved evidence

An independent clone-shared manifest gate uses the existing capacity: default **4**, trusted **1–16**.
It covers headers, body EOF, strict parsing and exact version comparison. It is separate from the
old chunk-request and whole-flow gates. Independently constructed clients have additive budgets.
`FileManifestClientError` exposes only `InvalidInput`, `CapacityExhausted`, `TimedOut`, `Unavailable`
and `InvalidResponse`, with static text, no tokens, paths, remote bodies or transport diagnostics.

The composed entry acquires the original whole-flow gate **before the metadata GET**, then reuses
the existing download core without acquiring it again. Capacity one therefore does not self-deadlock.
`FileGrantedDownloadError` separates invalid input/flow exhaustion from `Manifest(...)` and
`Download(FileDownloadError)`; no variant is added to the old public download error enum.
Metadata-phase cancellation releases both slots and creates no file job. During actual queued or
running staging/assembly, the existing job-held shared flow lease survives waiter cancellation.
No next network request or detached continuation starts. Timeouts are per request, not one hard
deadline for the entire file or hard cancellation of OS/database/CPU work.

Metadata failure starts no chunks. A later chunk/assembly error stops and preserves confirmed or
unconfirmed evidence. Repeated calls encounter existing exclusive targets as Conflict; they do not
adopt, skip, delete, clean up, retry or recover progress. The local whole-file acknowledgement retains
the assembler's actual EOF/digest/readback meaning, not final remote authorization or user saving.

## Verification and remaining boundaries

`file_manifest_client_tdd` contains real loopback defaults and explicitly ignored current-Source
bridges. `scripts/test-file-manifest-client.sh` requires an explicit disposable database, checks
compiled exact ignored inventory and executes only that inventory. Private framed-Body units cover
cases HTTP framing may reject before the collector. `fileManifestClient` guards check composition
and inventory, not real authorization. Native execution results are recorded separately below.

An original assembly acknowledgement can separately enter [explicit private saving](large-file-save.md)
to a trusted host-selected new file. That operation makes no GET and does not grant remote access.
Normal AppState/listener mounting, platform Session issuance, public OpenAPI/SDK/UI, discovery,
user-selected final save, progress/resume, desktop peer transfer, migrations and LAN deployment
acceptance remain separate. Versions/dependencies, published 0.5.7 validation, **106 historical status
blocks**, **40 existing execution tails** and the dated capability tensor remain unchanged.

The new [authorized delivery call](large-file-delivery-client.md) joins these stages in one explicit operation.
One original whole-flow lease remains owned by actual chunk/assembly/save work after cancellation;
first failure stops with no retry/adoption, and UI saving, resume and normal mounting remain separate.

The separate [owned user-directory target](large-file-user-save.md) explicitly accepts 0700/0750/0755
without relaxing private staging. Original directory UID/GID/mode checks and checked saving remain;
file picking, writable shared folders, resume and live mounting are separate.

## Recorded execution 2026-10-12

Both inert new API entries first compiled against actual loopback HTTP/private roots, then failed
behaviorally after fixture cleanup: **0 passed / 2 expected failed**, `Unavailable` and
`Manifest(Unavailable)`, zero warnings. Green reran those exact two cases: **2 passed**, zero warnings.
The Source hook's first compilation lacked its own `fixture::scope` reexport (`E0432`); adding that
separate Source-scope bridge fixed setup. That compilation failure is not behavioral Red.

The first default run was **15 passed / 1 failed / 2 ignored**, zero warnings: an oversized declared
Content-Length paired with a short immediately ending test body could make Hyper refuse before
headers flushed, yielding `Unavailable`. Holding only that fixture's actual first body frame behind
a registered cleanup barrier stabilized header observation. No production limit or general rejection
assertion was relaxed. The first static guard run also found a stale post-clock variable expectation;
it was updated to the actual captured `body_started`, not a change in timeout behavior.

| Fresh selection | Actual result and scope |
|---|---|
| Default manifest client target | **16 passed / 2 opt-in ignored**, zero warnings; actual six-selector native GET/full metadata, header/byte/strict JSON/version refusals, maximum declared metadata and actual bounded JSON, redirect target zero hits, real header/idle/fixed-total deadlines, clone/default-four/independent budgets, separate old chunk gate, cancellation, capacity-one composition, empty GET, first-failure/prefix preservation and repeat Conflict |
| New private contract/frame/job units | **16 passed**, zero warnings; 4 header, 9 collector, 3 lifecycle cases, including false hints/end, real None, post-length error/trailer/excess, ready-empty yield, Pending EOF, original head-derived clock, maximum digest array, metadata cancellation and actual queued chunk/empty assembly ownership |
| New explicit Source network runner | **2 unique SQL cases passed**, zero warnings; real Session publication and independent recipient grants for 4 MiB + 17 / empty files, actual metadata → ordered chunk ACKs → same-root `f1`, independent EOF/SHA/pattern/inode/0400/single-link checks and unchanged server snapshots; actual revoke after metadata before chunk delegation gives 200/404 with no file, revoked empty metadata gives 404 with no assembly |
| Cancellation regression selection | **29 passed / 7 opt-in ignored**, zero warnings; includes the new 3 lifecycle cases and unchanged running-save/assembly units for the shared core, not fresh public new-entry running-job hooks |
| Adjacent explicit runners | **8 existing manifest HTTP + 6 binary HTTP + 1 supplied-manifest download + 1 single-chunk SQL + 1 existing OpenSSL TLS passed**, zero warnings; with the new 2 SQL cases this is **18 unique fresh SQL + 1 existing TLS**, not new metadata-TLS deployment proof |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **1005 Rust passed / 852 opt-in ignored / 91 result groups**, **316 common passed**, zero failures/warnings; the new 16 defaults and 16 private units are already included |

Native focused composition/inventory guards passed **25**. Independent production, Source-test and
bilingual reviews found no remaining actionable defect; clarification distinguishes standalone
HTTP-only metadata from composed receiver/file work and limits fallible allocation to wire collection.
No new frontend/SDK/security-audit, browser/UI saving, resume, metadata TLS deployment, 64 GiB payload,
throughput/RSS, continuous authorization or live-auth migration acceptance is implied. The separately
ignored SQL cases are not default acceptance; repeated baseline and filtered units do not add unique
cases to full-gate totals.

Production froze before the private-unit selection; all code editors froze before serialized root
integration/SQL/full-gate execution. Final disposable PostgreSQL 16 audit confirmed the exact owned
data path, no TCP, **zero fixture schemas and zero other clients**. After stopping, only validated
`data`/`socket` were removed, releasing **59084 KiB (about 58 MiB)** of reconstructible test data.
Logs remain at `/tmp/cyanrex-manifest-client-j2GSrodQ`; no project/deployment or unrelated cache data
was removed. Versions/dependencies, live API/SDK/UI, published 0.5.7 reports, **106 old status blocks**,
**40 old execution tails** and the capability tensor remain unchanged. Bilingual sources/course
copies are synchronized; no commit/push.
