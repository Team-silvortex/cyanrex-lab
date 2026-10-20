# Private task content index HTTP adapter

Source **0.6.3** includes explicit loopback/TLS hosting, limited Session issuance and private
Task create/list/reopen/manifest replacement. Browser/SDK saving, new-byte publication, LAN access
and migration remain absent; earlier implementation-stage labels retain their original scope.

The later [retained platform connection](platform-task-index-connection.md), Unreleased after 0.6.2,
adds an original-storage backend and explicit host mounting. The Source-based adapter below retains
its contract; normal application mounting, content reopening and browser saving remain absent.

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased internal preparation**. Decision date: **2026-10-11**. Source version: **0.5.5**.
This independently constructed router returns bounded private Task metadata through the existing
[current Session index](task-content-index.md). It is not mounted in the live application and adds no
Session issuer, browser integration, OpenAPI/SDK route, storage installation or migration.

This is a separate same-day follow-up to the service index. The existing manual and catalogue content
routers remain unchanged: GET on their collection paths still returns 405, and their requests still
reject query strings. Listing uses only `/platform/v1/task-index`; routers are not automatically merged.

## Trusted configuration

On Unix, `platform_http::TaskContentIndexHttpState::new(source, workspace, origin)` accepts a
`DurableAuthSource`, a `SessionTaskContentIndexWorkspace` and a canonical allowed Origin.
`build_task_content_index_router(state)` returns the standalone Axum router. Construction does not
read environment configuration, install a namespace, start a listener or change `AppState`.

The supplied workspace fixes the namespace, scope and manual-only or explicitly neutral catalogue
admission policy. Requests cannot select an owner, authority, workspace, namespace, definition or
catalogue. Neutral configuration admits manual records and complete definitions from the frozen
catalogue, including bound and explicitly assessment-unbound definitions. Neither admission nor
listing invokes a provider, assessment, Review or execution.

`new` defaults to **four** in-flight requests. Trusted composition can use
`with_capacity(source, workspace, origin, max_in_flight)` with **1–16** slots. State clones and routers
built from those clones share slots; separately constructed states have independent, additive budgets.
This is not a process-wide memory, worker, network or proxy-buffer quota.

Origin configuration accepts canonical HTTPS, or HTTP on localhost/literal loopback only. It does
not configure TLS, authenticate a proxy, issue cookies or make a future listener safe to deploy.
The host still needs an explicitly isolated authentication source and protected Session delivery.

## Endpoint and raw query

The only endpoint is exact `GET /platform/v1/task-index`, with an empty body. Other methods on this
path return **405** with `Allow: GET`; HEAD errors have no response body and `Content-Length: 0`.
Other paths, including a trailing slash or another router's prefix, receive private **404** responses.

Only the following optional raw query fields are accepted, in either order:

| Field | Accepted value |
|---|---|
| `after` | Canonical lowercase, hyphenated, non-nil Task UUID |
| `limit` | Canonical decimal integer from **1 to 32**, without leading zeros |

With no query, the page defaults to **20** entries; an `after`-only query keeps that default. A query, when present,
must contain **1–128 bytes**. A bare `?`, empty values, unknown or repeated fields, missing or extra
`=`, trailing `&`, percent encoding, `+`, whitespace, signs, fractions and exponent notation are
rejected. Parsing uses raw URI bytes without URL/form decoding or normalization. A UUID in the query
is scoped by trusted workspace configuration, not by caller-supplied `TaskRef` JSON.

For example, `?limit=1` and `?after=10000000-0000-0000-0000-000000000001&limit=32` are valid shapes.
Validation does not establish that the cursor exists or belongs to the caller. Selection still uses
the current Session owner and ascending UUID order, strictly after the cursor.

## Private response projection

A successful response is **200** with exactly `{schema_version: 1, entries, next_after}`. Each entry
has these seven fields; service `TaskContentIndexPage` and `TaskContentIndexEntry` remain non-wire
values without serialization or request deserialization.

| Field | Meaning |
|---|---|
| `reference` | Complete scoped `TaskRef` |
| `revision` | Current Task revision |
| `title` | Current task-content title, not the definition title |
| `status` | Current lifecycle status, not assessment or acceptance |
| `definition_ref` | Exact definition reference, or explicit `null` for manual work |
| `updated_at` | Current Task update time |
| `payload_items` | Manifest item count, not content or byte availability |

`next_after` is a complete `TaskRef` for the last returned entry only when a validated lookahead
exists; otherwise it is explicit `null`. To request the next page, use that reference's `task_id`
as `after` on the same trusted router. An empty page is a successful list, not a not-found response:

```json
{"schema_version":1,"entries":[],"next_after":null}
```

The response contains no owner, text, manifest, Artifact pins, complete definition or total count.
Pages are private current metadata, not a fixed cross-request snapshot, complete inventory, content
availability check or saved-write receipt. UUID ordering does not promise chronological order.

## Request admission and authorization

Requests reuse the dedicated `cyanrex_platform_session` Cookie and single
`x-cyanrex-platform-request: 1` marker. The Cookie must contain one canonical non-nil UUID across all
Cookie headers; total Cookie-header bytes are bounded to 8 KiB. GET may omit Origin, but any supplied Origin must
be a single exact match. Any Authorization header is rejected, even with a valid platform Cookie;
the legacy Cookie does not grant platform access.
There is no CORS or Referer fallback; the fixed marker is not a secret CSRF token. This adapter does
not add token audience binding, issue cookies or prevent replay of a stolen Session.

Endpoint/query and credential checks precede fail-fast capacity admission. A full state returns
**503** `capacity_exhausted` / `not_attempted` without polling the body or dispatching Session/SQL work.
Admitted GET requests accept **zero actual body bytes**, reject content encoding and use the shared
**two-second whole-body deadline**, including streams of empty frames. If Content-Type is supplied,
only exact `application/json` or `application/json; charset=utf-8` is accepted, without duplicates.

The handler calls only `list_session_content_tasks`. Its existing source-owned transaction derives
the current account and membership, validates bounded heads, manifests and latest outbox records,
including lookahead and empty-page required columns, then rechecks namespace pins, account,
membership and exact Session with fresh database time before confirming commit. Corruption or
full-definition drift fails the whole page rather than filtering records out. No Artifact namespace,
blob or file is read, so listing may succeed even when reopening content must fail.

The slot remains held through the existing command, JSON projection and response-body EOF, error
or drop, including admitted error responses. Cancellation releases admission but does not cancel
already-dispatched blocking work or establish earlier rollback. Original Session/SQL deadlines stay
unchanged; there is no new command timeout, retry queue, `Retry-After` or recovery protocol.

## Errors and verification scope

All responses use `no-store`, `no-cache` and `nosniff`. Errors contain versioned `error.code` and
`error.outcome`, not raw storage diagnostics, tokens or ownership details. Pre-dispatch rejection
uses `not_attempted`; every service error conservatively uses `unconfirmed`. Invalid/expired Sessions
map to 401, denied private-work access to 403, and corrupted metadata or unavailable storage to generic
503 `service_unavailable`. These outcomes are not receipts for prior writes or permission to retry.

The dedicated `task_content_index_http_tdd` target covers in-process requests. Its **eight SQL cases**
are a separate source inventory: mixed pages; owner isolation and invalid queries; lifecycle and
recreated accounts; full head/manifest/outbox and lookahead faults; missing columns on empty pages;
Artifact independence; real row-wait expiry; cancellation, pool restoration and response capacity.
The exact runner is [`scripts/test-task-content-index-http.sh`](../../scripts/test-task-content-index-http.sh);
[`taskContentIndexHttp` guards](../../scripts/tests/taskContentIndexHttp.test.mjs) check source wiring,
strict projection and selection inventory, not database behavior.
Source inventory is not an execution count; dated results belong in [project status](project-status.md).
See the [testing guide](testing-guide.md) for maintained selections.

The HTTP module retains the existing Unix preparation boundary. These tests do not establish
cross-platform compilation, a real listener, browser/TLS/proxy behavior or installed-system acceptance.
No new SQL path, writable command, installer, provider, Session issuer, SDK/OpenAPI/UI connection,
migration or recovery is introduced.

Implementation: [trusted state and router](../../engine/src/platform_http/task_index/mod.rs),
[query and response contract](../../engine/src/platform_http/task_index/contract.rs),
[Session dispatch](../../engine/src/platform_http/task_index/handlers.rs), and
[dedicated test target](../../engine/tests/task_content_index_http_tdd.rs).
