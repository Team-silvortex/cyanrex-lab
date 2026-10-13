# Controlled single chunk HTTP client

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API **0.3.0**, **2026-10-12**.
This explicit native client requests one exact chunk from a trusted fixed origin, then reuses the
[receiver](large-file-chunk-receiver.md). It implements the next link in
[proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), not a mounted distribution service,
whole-file downloader, resume mechanism or desktop pairing protocol.

## Trusted configuration

Included in 0.5.8, the same client also offers [ordered private download](large-file-download-client.md)
with `fetch_and_assemble`. A separate flow gate follows actual save/assembly jobs; the original
single-chunk API, transport policy and per-chunk deadlines stay unchanged. This dated single-chunk
execution record is not a new whole-flow acceptance result.

`services::file_chunk_client::FileChunkClient::new_https(origin, staging)` takes an existing trusted
`ChunkStaging` and a canonical origin, not a request URL. `new_loopback` explicitly opts into plaintext
HTTP only for a canonical **literal** IPv4 loopback or `::1`; `localhost`/DNS, numeric aliases, userinfo,
paths, queries, fragments and non-loopback HTTP are rejected. Origin input is at most 2048 bytes and
must match its parsed origin serialization exactly; it is not silently normalized or redirected.

`with_config` accepts `FileChunkClientConfig` with `FileChunkTransport::{Https, LoopbackHttp}`,
capacity **4**, header timeout **10 seconds**, receiver idle **5 seconds** and receiver total
**60 seconds** by default. Capacity is 1–16; header timeout is 1–60 seconds, including fractions.
The receiver retains its existing idle/total bounds. Optional `trusted_root_pem` is a nonempty,
bounded **64 KiB** PEM bundle for HTTPS. Only parsed certificates become extra trust roots; no
client identity is configured. Hosts must not supply key material. Parsing requires at least one
usable certificate but does not enforce certificate-only/canonical PEM or reject all mixed text.
The client retains normal certificate and hostname/IP SAN verification with default roots plus that
explicit trusted-host addition. Loopback HTTP rejects a supplied certificate bundle.

The client is private and owned: callers cannot inject a reqwest client/builder, arbitrary URL,
headers, proxy, cookie jar or dangerous certificate policy. Construction sends no request and does
not create/chmod a staging root. Configurations are not public wire payloads or serialized secrets.
Clones share the original configuration and budgets; replacing host settings does not revoke
surviving clones or requests. A configured HTTPS hostname is not DNS/address pinning, an SSRF
firewall, current server account authority or permission to target an untrusted network.

## Exact native GET

`fetch_and_stage(token, grant_id, upload_id, &manifest, index)` first validates the staging workspace,
selected chunk and a canonical non-nil 36-byte Session UUID. A mismatch fails before admission or
network access. The caller already supplies the typed expected manifest; this client does not fetch
or authenticate that manifest itself. Only the remote current-recipient command authorizes bytes.

The target is always `/platform/v1/file-chunks` with the existing seven raw canonical selectors:
`grant_id`, `upload_id`, `file_id`, `version_id`, `byte_length`, `sha256`, `chunk_index`.
No token, owner, namespace, hidden key, path or mode enters the query. The request has no body and
uses one sensitive `Cookie: cyanrex_platform_session=<token>`, marker
`x-cyanrex-platform-request: 1`, octet-stream Accept and identity Accept-Encoding. Tokens are
per-call, not stored as defaults, returned, logged or included in Debug/error strings.

Native GET omits Origin under the [existing HTTP policy](large-file-chunk-http.md). The transport
origin is not presumed equal to the separately configured browser/UI Origin; there is no invented
Authorization header, legacy cookie, Range, conditional, Referer or Content-Type.

The locked reqwest **0.12.28** client explicitly disables automatic protocol retries
(`retry(reqwest::retry::never())`), redirects, environment proxies and gzip/brotli/deflate/zstd
decoding. HTTP/1 is selected, idle connection reuse disabled and cookie storage not enabled.
`Accept-Encoding: identity` is a request, not trust in the peer: forbidden response transformations
are still rejected by the receiver. No server status or response cookie authorizes automatic retry.

## Deadlines, admission and saved outcomes

Scope/index/token checks precede fail-fast request admission. Full capacity returns
`CapacityExhausted` without opening a request. The request deadline bounds connection/TLS/send
through response headers, with a monotonic post-return check. A reqwest full-body `.timeout()` is
not used for this header-only deadline. After headers, the receiver independently enforces idle and
fixed total through complete frames, message EOF and selected SHA-256, before any file dispatch.

The conversion is `let response: http::Response<reqwest::Body> = response.into();`. It preserves
the library's framed body and errors rather than using data-only `.chunk()`, `.bytes()` or
`.bytes_stream()` convenience iteration. Checks concern the headers and HTTP message frames the
transport exposes. HTTP/1 Content-Length framing does **not** prove raw TCP connection EOF or
detection of every socket suffix; another message or unsolicited wire bytes are not automatically
an extra body frame. This is not a raw-wire parser or a transport-independent framing proof.

The whole-call request permit enters a narrow **crate-private** receiver path and the private
`LeasedCursor` with the validated buffer and receiver permit. The actual queued/running existing
save job owns all of them after waiter cancellation, dropping bytes before returning capacity.
No permit is kept only in an async wrapper after file dispatch. Public supplied-response behavior
and its cheap-header-before-admission order remain unchanged.

Before file dispatch, dropped/timed-out waiting releases local admission and sends no unverified
prefix to disk. Cancellation does not promise hard termination of DNS, OS/network tasks, remote
commands or already-dispatched file work. The existing exclusive legacy `v1` staging, no-follow,
0400 sync/original-inode readback, Conflict and retained `Unconfirmed` semantics stay unchanged.
Neither cancellation nor timeout means rollback, deletion, adoption or retry permission.

Success is the same move-only `ReceivedFileChunk`: only this call's acknowledged local stage, not
continuous remote authority, server workspace/identity attestation, a remote delivery receipt,
fresh whole-file verification or reconstructible progress. A completed metadata result does not
retain bytes. Budgets are clone-shared but independently constructed clients remain additive; they
are not a global socket/resolver/process-memory/physical-disk or multi-instance quota.

## Integration and tests

The [binary router](large-file-chunk-http.md) remains unmounted. Tests explicitly serve it on an
exclusive loopback listener; production startup, AppState, OpenAPI/SDK/UI, Session issuance,
schemas, automatic trust/discovery and deployment are unchanged. Central scheduling, final whole-file
assembly/save, resumption and peer transfers remain separate. Version stays 0.5.6/API 0.3.0.

Separate [private assembly](large-file-chunk-assembly.md) now accepts already staged legacy chunks
and checks one exclusive whole-file output. It is not part of `fetch_and_stage`, a multi-chunk network
scheduler, user-selected final saving or recovered download progress.

`file_chunk_client_tdd` separates default actual loopback HTTP/file tests from two ignored selections:
one real current-Source/PostgreSQL bridge and one OpenSSL TLS test. The latter generates temporary
CA/leaf/key material, verifies unknown CA and wrong IP SAN rejection before HTTP disclosure, and
checks correct explicitly trusted TLS followed by real 17-byte private staging. No key is committed.
`scripts/test-file-chunk-client.sh` requires an explicit disposable database and installed OpenSSL,
lists exact ignored names before running them, and is wired into the existing CI step.
`fileChunkClient` guards check composition/inventory only; actual executions are recorded separately.

Earlier **106 status blocks**, **32 detailed execution tails** and the dated capability tensor are
preserved, not rescored or relabelled as fresh evidence. Current bilingual notes and generated course
copies describe this new link without claiming LAN deployment or complete download acceptance.

## Recorded execution 2026-10-12

The inert client scaffold compiled cleanly against the actual exclusive loopback server and private
root. Red was **0 passed / 1 expected failed**, `Unavailable`, zero warnings: the client did not make
the request or save a chunk. Server/root cleanup preceded the behavior assertion; this was not a
missing-symbol, denied-socket or permissions failure. Green passed **1 / 0**, zero warnings, with the
exact real request, independent EOF/SHA-256/0400/single-link verification and no placeholder result.

| Fresh selection | Actual result and scope |
|---|---|
| Client default target | **14 passed**, **2 ignored**, zero warnings; actual 17-byte and 4 MiB + 17 HTTP/files, canonical configuration, typed inputs, bad heads/short/corrupt bodies, redirect target zero hits, distinct header/body deadlines and cancellation/admission |
| Isolated proxy environment | The same 14 passed with HTTP/HTTPS/ALL_PROXY pointing at unusable loopback port 9 and NO_PROXY empty, scoped only to that test process; no global environment was changed |
| Private chunk-worker units | **4 passed**, zero warnings: old receiver queued/running cases plus **2 new** cases for real client HTTP EOF→queued stage and real running stage with optional caller permit; cancellation retains actual gates/inode work and duplicate saves conflict |
| Explicit new external runner | **1 actual Source SQL + 1 OpenSSL TLS passed**, zero warnings; current recipient over a real socket, unchanged server snapshots, later revoke refusal with old local bytes preserved; unknown CA/wrong IP SAN yield zero independently counted decrypted HTTP bytes, trusted correct SAN yields real private staging |
| Adjacent explicit SQL runners | **3 receiver + 6 binary HTTP + 10 Source reader passed**, zero warnings, including original authorization/pins, corrupt storage, ownership, expiry, waits and cancellation |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **919 Rust passed**, **841 opt-in ignored**, **87 result groups**, **296 common passed**, zero warnings/failures |

The **20 unique fresh SQL cases** and the one explicit TLS case are separate from default acceptance;
the new 14 defaults and two units are already included in 919, not additional passes. The ignored
inventory includes SQL/kernel/external selections, not automatically executed cases. No previous
903/19 or 117-case record is relabelled. No fresh frontend build, SDK suite, security audit, actual
LAN/TLS deployment, whole-file/save/resume acceptance, disk-full/fsync/power-loss, peer transfer or
RSS/throughput benchmark was run. TLS proof uses synthetic disposable certificates, not an installed
deployment. OpenSSL reserves then releases a loopback port before process bind; process/readiness and
healthy controls check the fixture, not atomic exclusion of every external port race.

All Cargo/format/SQL runs were serialized by the root after editor freeze. Native private-file/socket
execution preserves production owner/ancestor checks. Final PostgreSQL audit reported its own exact
data directory, no TCP, **zero fixture schemas and zero other clients**. It was stopped; only validated
`data`/`socket` were removed, releasing **84432 KiB (about 82 MiB)** of reconstructible test data.
All temporary TLS keys/processes/threads were cleaned by their fixtures. Logs remain at
`/tmp/cyanrex-file-chunk-client-eGBxz16e`; no deployment/repository data was deleted. Versions, Cargo
dependencies/lockfile, normal startup and live API/SDK/UI composition are unchanged; no commit/push.
All **106 old status blocks**, **32 old execution tails** and the capability tensor remain unchanged.
The composition guard was corrected to distinguish the imported timer name and manifest `chunk(index)`
from an actual timed send or a data-only response `.chunk()`; those source-text failures were not
behavioral Red. TLS peer byte counting strengthened the negative oracle; the final explicit runner
was rerun. Bilingual notes and course copies are synchronized. The post-evidence
`scripts/quality-gate.sh --format-only` passed **296 common checks**, zero warnings/failures, plus
course/version/OpenAPI drift, tool and Rust formatting checks; the final note-only sync is rechecked.
