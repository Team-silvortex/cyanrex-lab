# Explicit composition of file distribution endpoints

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.7**, API **0.3.0**, **2026-10-13**.
This [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) follow-up combines the existing
[manifest GET](large-file-manifest-http.md) and [binary chunk GET](large-file-chunk-http.md) from one
trusted configuration. It removes manual selection between independently configured routers, without
mounting the normal app, issuing Sessions or separating the privileged Engine into an isolated service.

## One Source and one workspace

`FileDistributionHttpState::new(source, workspace, origin)` accepts one `DurableAuthSource` and one
`SessionFileChunkReadWorkspace`. A crate-private cached seam checks the existing authority/scope
relationship and clones that workspace's exact nested download configuration: publication and
recipient configuration are not supplied a second time. Both endpoint states use the same supplied
Source and canonical platform Origin. The host cannot inject two raw routers through this API.

Construction is inert. It does not connect to SQL, open new paths, install schemas, verify OIDs/store
health, authenticate the caller or establish configuration provenance. Requests still perform the
original current-Session/recipient/Published checks and original source pinning; cached consistency
is neither byte authority nor proof that a deployment is initialized or unprivileged.

`with_capacities(source, workspace, origin, manifest_capacity, chunk_capacity)` configures independent
HTTP budgets, each 1–16 and each 4 by default. Validation order is canonical Origin, both capacity
bounds, then cached workspace matching. The separate `FileDistributionHttpConfigError` has static
`InvalidOrigin`, `InvalidCapacity`, `InvalidWorkspace` variants, leaving old error enums unchanged.
Clones and separately built routers from them share each HTTP budget; separately constructed states
have additive HTTP budgets, while cloning the supplied workspace preserves its original byte budget.

## Two exact endpoints and one private fallback

`build_file_distribution_router(state)` combines fallback-free internal route tables with the old
handlers, rather than merging complete public routers. It registers only `/platform/v1/file-manifests`
and `/platform/v1/file-chunks`; old handlers enforce exact GET, six/seven raw selectors, private
credentials, optional exact GET Origin and empty-body checks. One fallback returns private 404
`not_found` / `not_attempted`, without body polling, authentication or SQL; one shared response-header
layer covers successes and errors. No teaching, login, capability-discovery or control endpoint is added.

Standalone builders keep their original fallback/header behavior. Queries cannot choose scopes,
owners, namespaces, grant issuers or storage roots. Protocol version 1 and public OpenAPI remain unchanged.

## Independent response lifetimes

Metadata HTTP admission lasts through its ordinary JSON body's final frame/end/error/drop; retaining
JSON `Bytes` does not retain the slot. Binary data retains both HTTP and Source byte budgets through
the last owner-retaining `Bytes` clone or nonempty slice, even after body EOF/drop. Its original ten-second
first-frame window, actual worker ownership and cancellation behavior remain unchanged. No new global
quota, timer, buffer, background task, retry, cleanup or transfer receipt is created by the composition.

Slow binary consumers do not exhaust the independent metadata HTTP budget. Metadata remains a current
eligibility observation, not permission for later bytes; later Source admission rechecks revocation
but cannot reclaim already-approved snapshots. A full HTTP or byte budget can reject first with a
capacity error. Pairing endpoints is not continuous authorization.

## Verification and remaining connections

`file_distribution_http_tdd` covers default closed-Source routing, trusted configuration, strict
selectors/credentials/body checks, independent/shared admission and cancellation/error-body release.
It does not fabricate binary ownership or claim SQL authorization. Existing explicit delivery Source
cases now send all original requests through one composed router; a separate Source lifetime case
checks actual binary clones/slices, metadata independence and later revocation. Run their exact inventory
with `bash scripts/test-file-delivery.sh` and an explicit disposable database URL.

`fileDistributionHttp` guards cover composition/inventory only. Current bilingual notes and generated
course copies describe this edge without changing **106 historical status blocks**, **54 existing
execution tails**, **8 protected files**, the dated capability tensor or published 0.5.7 evidence.
UI/SDK mounting, secure Session issuance, an unprivileged data-service launcher, deployed-LAN acceptance,
resume/recovery and physical disk policy remain open. The following record separates each executed layer.

Unreleased after the 0.5.8 tag, [real Source over HTTPS](large-file-source-https.md) adds one
combined native authorization/file/router/TLS/saving test chain, including pre-dispatch revocation
and actual response-body cancellation. It changes no production listener, policy or dated score.

## Recorded execution — 2026-10-13

All checks below ran natively in `/home/chiharukiryu/cdev/cyanrex-lab`; logs remain under
`/tmp/cyanrex-distribution-router-oiOxZGHU`. No commit, tag, push or version change was performed.

- The compiled baseline
  `baseline::distribution_http_routes_manifest_and_chunk_gets_to_one_explicit_closed_source_with_private_errors`
  produced the intended behavioral Red: both valid requests and fixture cleanup completed, then the
  manifest GET's 404 failed against the required closed-Source 503 (**0 passed, 1 failed**). With
  composition implemented, the unchanged baseline passed (**1 passed**); the full new default target
  passed **9 tests, 0 ignored**. The earlier getter type-alias compilation error was repaired before
  Red and is not behavioral evidence. Defaults use a deliberately closed pool, not SQL authorization.
- The delivery runner passed **3 unique actual Source cases** through the composed entry. The first
  retains large/empty delivery with private, observed-owned and controlled-owned selected outputs;
  independent bytes/hash, exact GET inventory, saved ownership/mode and Source/storage snapshots
  remain checked. The second checks real revocation between metadata and chunk, plus revoked empty
  metadata. These two use the original client over loopback TCP into one composed router, not two
  independent router configurations.
- The third Source case uses actual SQL/files and in-process Axum: two routers built from one state
  clone share binary HTTP admission, and the original workspace retains its Source byte budget.
  Actual approved 17-byte frames, retained clones and a nonempty slice keep both budgets full after
  body EOF/drop. Metadata still succeeds independently; an unpolled JSON response holds only its
  own slot, while retained JSON bytes after the terminal frame do not. The exact recipient revocation
  is the only server snapshot delta; old approved slices remain readable, and after the final binary
  slice drops both endpoints return current private 404 rather than a capacity error.
- Adjacent unchanged route/Source behavior was rerun: **8 manifest HTTP SQL**, **6 binary HTTP SQL**
  and **2 granted manifest-client SQL** cases. Together with the 3 delivery cases, this is **19 unique
  SQL cases**, all passed with zero warnings. SQL ran only in this turn's owned disposable PostgreSQL,
  listening on its private Unix socket with TCP disabled; no user/live database was used.
- The existing controlled HTTPS runner passed **3 unique TLS cases** separately: complete selected
  large/empty/short outputs, unknown CA/wrong IP SAN/redirect rejection and explicit cancellation.
  This reruns the client against disposable OpenSSL peers; it does not combine TLS with Source SQL
  or establish deployed-LAN, normal-app or privilege-isolation acceptance.
- Focused **60 static guards** passed; they check composition/inventory, not authorization behavior.
  The native sanitized-environment `--backend-only` gate passed **1,097 Rust tests across 98 successful
  result groups**, **858 explicitly ignored cases** and **351 common static checks**, including all
  **15 compile-fail doctests**, with zero final warnings/failures. The ignored cases were not silently
  executed by the full gate; the 19 SQL and 3 TLS cases above are separate explicit runs. Frontend/SDK
  builds and a fresh dependency/security audit were not run for this backend-only slice.
- The frozen baseline guard confirmed **83 related source files unchanged** outside this composition's
  allowed seams, and **106 historical status blocks**, **54 existing execution tails**, **8 protected
  files** unchanged. The dated capability tensor, public OpenAPI, dependency/version manifests and
  published 0.5.7 release evidence remain intact. Shared old test support's unused independent harness
  is allowed only in the delivery test target; no production warning suppression was introduced.
- After all SQL runners finished, the exact owned server was verified and stopped. Only this turn's
  `data` and `socket` directories were removed, freeing **59,268 KiB (about 58 MiB)**; logs were kept.
  This is test-resource cleanup, not a new distribution cleanup or disk-policy feature.
