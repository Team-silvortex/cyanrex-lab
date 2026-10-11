# Explicit task content HTTP adapter

Source release **0.5.5** (**2026-10-11**) includes the shared-admission and separate catalogue-adapter
follow-ups. Dated `Unreleased` labels retain their original implementation stage, source versions and
verification scope. Neither source inclusion nor earlier tests mount the router, issue Sessions,
connect browser saving, switch live authentication, migrate storage or validate a running deployment.

Status: **C2-N preparation included in 0.5.0**. Decision date: **2026-10-03**.
This separately constructed router translates bounded HTTP requests into the [C2-M Session content
commands](session-task-content.md). **It is not mounted in the running application**, does not issue
login credentials, and does not connect the browser editor. Existing routes, authentication, OpenAPI
and generated SDK remain unchanged.

The **2026-10-08 Unreleased follow-up**, based on source version **0.5.4**, adds shared admission
through response-body completion. It is not part of the published 0.5.4 tag or the original 0.5.0 evidence.

The separate **2026-10-11 Unreleased C2-Y follow-up**, still on source 0.5.4, adds a typed
[catalogue-content HTTP adapter](catalog-task-content-http.md) at `/platform/v1/catalog-tasks`.
This C2-N router remains manual-only: its state still requires `SessionTaskContentWorkspace`, its
creation JSON rejects definition fields, and it does not admit catalogue records. The new adapter
has its own explicit state and fallback, not a client-selected mode or automatic router merge.
Neither adapter is mounted in the application or connected to browser saving, Session issuance or
C2-P–W publication/journal workflows.

## Trusted composition

On Unix, `platform_http::TaskContentHttpState::new` takes an explicit `DurableAuthSource`,
`SessionTaskContentWorkspace` and one canonical allowed origin. `build_task_content_router` returns
an independent Axum router. Construction reads no environment, installs no schemas, opens no listener,
and does not change `AppState` or the normal `build_router`.

`new` allows four in-flight requests. Trusted composition may instead call
`with_capacity(source, workspace, origin, max_in_flight)` with an integer from **1 to 16**;
other capacities return `InvalidCapacity`. State clones and routers built separately from those
clones share one budget; constructing a new state creates an independent budget.

The configured origin must be HTTPS, or HTTP on localhost/a literal loopback address. It must be a
canonical origin without user information, a path, query, fragment or wildcard. This admission rule
does not configure TLS, trust a proxy, validate a certificate or constrain an eventual listener.
Before mounting, the host must explicitly select the isolated source/storage installation, secure
ingress, cookie issuer and login lifecycle. Activated source tables must not share a legacy writer.

## Request admission

Every request uses the dedicated `cyanrex_platform_session` Cookie and a single
`x-cyanrex-platform-request: 1` header. Only one occurrence of that Cookie name is accepted across all
Cookie fields, with a canonical non-nil UUID token. Cookie headers are bounded to 8 KiB. The old
`cyanrex_session` Cookie and Bearer/URL credentials do not supply platform authority. Any Authorization
header is rejected, even alongside a valid platform Cookie.

Every mutation requires one exact configured `Origin`; a read may omit it, but a supplied Origin must
still match. Missing, null, duplicate or foreign mutation origins are rejected. There is no Referer
fallback, environment bypass or CORS support. The fixed request marker is **not a secret CSRF token**.
These checks precede body consumption and do not replace current-Session authorization.

The separate Cookie name prevents automatic legacy-cookie reuse; it does **not** add an audience to
the underlying reusable durable token or prevent replay of a stolen token. This slice neither issues
nor clears cookies. A future issuer must protect delivery and use appropriate Secure, HttpOnly and
SameSite attributes, without storing credentials in URLs, local exports or logs.

## Shared request and response capacity

Cheap endpoint, Origin, Cookie and request-marker checks run before trying to acquire a slot.
There is no admission queue. A full budget returns **503**, `capacity_exhausted` and
`not_attempted`, with `no-store` and `nosniff` but no `Retry-After`. The rejected request's body is
not polled and no C2-M command or SQL work is invoked. Pre-admission rejections consume no slot.

An admitted request holds its slot across body collection, the existing C2-M command and JSON
serialization. Returning from the handler does not release it: the returned response body retains
the slot until its terminal frame/end, body error or drop. Admitted error responses follow the same
rule. Body completion means transfer to the next HTTP consumer, not confirmed delivery to a client.
The two-second/64 KiB body limit and C2-M's ten-second command deadline remain unchanged; there is
no competing command timeout or response-delivery deadline.

Cancelling a request or dropping its response releases the slot, not a rollback guarantee or a
stop signal for already-dispatched blocking I/O. The budget covers only the same trusted state
instance and its clones. It is not a global process, memory, storage-worker, network/proxy-buffer
quota, tenant isolation or a historical receipt, and it does not authorize retries.

## Request and response contract

The prefix below exists only on the explicitly constructed router. Task path IDs are canonical UUIDs.
Queries, GET bodies, encoded content and unsupported methods are rejected. Mutations accept JSON only;
the complete HTTP body, including its envelope, is limited to **64 KiB** and has a **two-second read
deadline**. The real incoming stream is bounded independently of `Content-Length`, including streams
of empty frames. The accepted media types are exactly `application/json` and
`application/json; charset=utf-8`; a supplied GET media type follows the same rule. Duplicate media
types, content encodings and trailers are rejected. Known-route method errors advertise only that
route's allowed method; HEAD errors have no response body.

| Method and path | Exact request fields | Success |
|---|---|---|
| `POST /platform/v1/tasks` | `schema_version: 1`, `task_id`, `manifest` | 201 with committed Task and manifest |
| `GET /platform/v1/tasks/{task_id}` | No body or query | 200 with committed Task, manifest and ordered text contents |
| `PUT /platform/v1/tasks/{task_id}/content` | `schema_version: 1`, `expected_revision`, `manifest` | 200 with committed replacement |
| `POST /platform/v1/tasks/{task_id}/status` | `schema_version: 1`, `expected_revision`, `status` | 200 with committed state change |

Root records must be objects. Unknown and duplicate fields, positional arrays, invalid versions,
non-integer/unsafe revisions and alternate enum-object forms are rejected. A manifest uses the
[server manifest contract](task-content-manifest.md), not browser draft JSON. Status is a string from
the existing lifecycle; it cannot express acceptance or execution. Owner, scope, namespace, filesystem
root and authentication source are never selected by a request.

A successful write returns `{schema_version: 1, task, manifest}`. Reads add `contents`, in manifest
order, with each item `{artifact, text}`. The text comes from exact owned revisions validated by C2-M,
not from a separate unguarded file read. There can be zero to 32 items, each at most 256 KiB; JSON
escaping and metadata add response overhead. This per-request limit is not a process-wide memory or
concurrency quota. Missing and another owner's Tasks share the same not-found response.

The adapter does not expose Artifact publication/revision, whole-draft import, a task list, catalogue
admission, sharing, Review, execution or file deletion. Clients cannot save new text by putting it
inside a manifest; publication remains a separate missing public boundary.

The 0.5.3 [C2-O importer](task-draft-publication.md) prepares publication values and binds supplied
content to this manifest shape without I/O. It is not called by this router and does not expand its
64 KiB request limit or accept raw draft text over HTTP.

## Errors and uncertain outcomes

The separate 0.5.3 [C2-P workflow](session-task-draft-publication.md) composes internal publication
steps and Task creation, but this router does not invoke it, accept attempt state or expose its progress.
Its private in-memory state does not change the HTTP outcomes below or provide a durable operation receipt.

Errors have a versioned JSON envelope with `error.code` and `error.outcome`, not raw storage errors,
owner details or credentials. All router responses, including unknown paths, rejected methods,
malformed input and unavailable storage, are private `no-store` with `nosniff`.

`not_attempted` means this HTTP request was rejected before invoking C2-M. Once the adapter dispatches
a C2-M command, any error is conservatively `unconfirmed`, including not-found or conflict errors;
the envelope does not promise that a mutation rolled back. A successful missing-task read is also
reported as not-found after dispatch, without granting permission for a retry.

The underlying ten-second command deadline remains authoritative after body parsing; the HTTP adapter
does not add a competing timeout around a dispatched mutation. There is no Retry-After, automatic
retry, idempotency receipt, outcome reconciliation or success before confirmed commit. Disconnects,
timeouts and missing responses are not rollback evidence. Independently published content is not
deleted because an edit failed.

## Verification and next boundary

Default in-process HTTP tests cover admission, JSON and transport limits, private response headers
and unavailable sources; static guards check the absence of normal-app/API/SDK wiring. Disposable PostgreSQL cases send HTTP
requests through this router into real C2-M transactions, exercising success and authorization/storage
failures. Dated counts and omissions are recorded in [project status](project-status.md).

These checks do not establish a live listener, browser-to-server flow, proxy/TLS behavior or deployed
acceptance. This router does not publish content; the separate 0.5.5 [C2-X text Artifact
router](text-artifact-http.md) supplies explicit publication and exact reads without mounting either
router. Secure Session issuance, explicit installation/public composition and authorized use of the
pure draft mapping still precede browser save/read/conflict handling. The live OpenAPI and SDK must
only advertise the interface when its intended deployment composition is deliberately added.
