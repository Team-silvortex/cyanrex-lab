# Explicit text Artifact HTTP adapter

Status: **C2-X internal preparation included in source release 0.5.5**. Decision date: **2026-10-08**.
Source release: **0.5.5** (**2026-10-11**). The router remains unmounted.

Dated `Unreleased` follow-ups and verification records retain their original source versions and
execution scope; inclusion in 0.5.5 does not rerun or extend that evidence.

This separately constructed router publishes one private text Artifact and reads one exact revision
through existing current-Session Artifact commands. It supplies the text-publication transport that
the separate [C2-N Task-content router](task-content-http.md) does not expose. Neither router is
mounted in the running application, and neither connects browser drafts, issues Sessions or changes
live authentication, OpenAPI, the generated SDK or publication journals.

## Trusted composition and admission

On Unix, `TextArtifactHttpState::new(source, workspace, origin)` receives an explicit
`DurableAuthSource`, `SessionArtifactWorkspace` and canonical allowed origin;
`build_text_artifact_router(state)` returns an independent Axum router. The workspace supplies scope,
namespace and file root. A request cannot select them, its owner or its authentication source.
Construction reads no environment, installs no schema, opens no listener and changes no `AppState`.

Origin, dedicated `cyanrex_platform_session` Cookie, canonical token and
`x-cyanrex-platform-request: 1` admission reuse the [C2-N rules](task-content-http.md#request-admission).
There is no Bearer/legacy-cookie fallback, CORS or Session issuer. These checks do not configure
TLS, deliver secure cookies or replace each source command's current authorization.

The default capacity is **four**; trusted `with_capacity` accepts **1–16** and otherwise returns
`InvalidCapacity`. Clones of one text-Artifact state share slots, including separately built routers.
New states and Task-content states have **independent, additive budgets**. Cheap endpoint/header
checks precede fail-fast acquisition. Full capacity returns private 503
`capacity_exhausted` / `not_attempted` without body polling, SQL or `Retry-After`.

Slots cover body collection, the existing source command, JSON serialization and the returned success
or error body until terminal frame/end, error or drop. Handler return alone does not release a slot.
Cancelling releases admission, not a rollback guarantee or a stop signal for dispatched blocking I/O.
This is not a cross-router/process memory, worker, network or proxy-buffer quota.

## Publish one text Artifact

`POST /platform/v1/text-artifacts` accepts exactly one JSON object with:

| Field | Requirement |
|---|---|
| `schema_version` | Integer `1` |
| `artifact_id`, `revision_id` | Existing canonical non-nil identifier formats |
| `title` | Nonblank, control-free text, at most 256 UTF-8 bytes; checked without inventing a title |
| `text` | At most 256 KiB decoded UTF-8 bytes; empty is allowed; control characters other than TAB/LF/CR are rejected |

Unknown/duplicate fields, positional arrays and invalid field forms are rejected. The adapter fixes
kind to `cyanrex.task-text` and media type to `text/plain`; clients cannot select a kind, owner, scope,
path or length/digest assertion. Request IDs are explicit server-contract identifiers, not authority
granted to local editor IDs.

The complete JSON wire body has an independent **2 MiB** limit and **two-second** read deadline.
The actual stream is bounded even with an incorrect `Content-Length` or repeated empty frames.
JSON escaping counts toward the wire limit; the decoded-text limit remains 256 KiB. C2-N Task
requests retain their **64 KiB** limit. Queries, encoded bodies, trailers and unsupported media/methods
are rejected under the shared transport rules.

Successful publication returns **201** with `{schema_version: 1, artifact: ArtifactRevision}`.
Duplicate publication returns **409**, even for identical IDs/text; it does not adopt an existing
revision or turn the request into an idempotent receipt. Publication and a subsequent Task command
are separate commits; a failed Task command does not authorize deleting the Artifact.

## Read one exact revision

`GET /platform/v1/text-artifacts/{artifact_id}/revisions/{revision_id}/{sha256}` accepts no query or
body. Raw path IDs and the digest must have their canonical forms. A successful read returns **200**
with `{schema_version: 1, artifact: ArtifactRevision, text}` after current authorization and exact
reference/digest checks. A historical revision remains readable even if a newer head exists; valid
non-first sequence/parent metadata is allowed. The route does not silently substitute the head.

The returned revision must have the fixed text kind/media type and valid bounded UTF-8 text.
Another kind/media type, binary/control content or oversized text produces a private **503** with
`unconfirmed`, not a successful text projection. The generic reader can first materialize up to
**1 MiB** before the stricter text check; 256 KiB is not a peak-memory or worker limit.
Missing and foreign-owned revisions share **404**. Wrong-digest or corrupt-storage failures remain
**503**, not missing content or permission to publish again.

## Outcomes and the Task boundary

Responses use the existing private `no-store` / `nosniff` JSON envelope. Local admission/parse failures
are `not_attempted`; after invoking a source create/read command, every error is conservatively
`unconfirmed`. Existing command deadlines remain authoritative, with no competing HTTP command timeout.
An error, cancellation or lost response is not rollback, historical confirmation or retry authority.

A trusted caller can take a successful response's exact Artifact reference, form a C2-K manifest and
make a **separate** request to C2-N. The representative preparation chain is an independent in-process
fixture HTTP sequence, not a Text-router call into the Task router, automatic router merge, atomic
batch or mounted public workflow. Each request reauthorizes; deployment routing/fallback composition,
Session issuance and browser save/conflict handling still require explicit design.

There is no revision/delete API, adoption, automatic retry, restored attempt, journal dispatch,
installer, live-auth cutover or new deployment permission.

## Verification boundary

On 2026-10-08, **12 default contract/transport cases, 10 exact disposable PostgreSQL cases and three
new common guards passed** for this slice. This is separate evidence, not inherited C2-N execution.
The initial valid 70 KiB POST failed against the refusal stub, then passed after implementation.

The dedicated target is `text_artifact_http_tdd`, with an exact disposable-database runner
`scripts/test-platform-text-artifact-http.sh`. Contract/transport units and source guards are separate
from SQL execution; dated actual results belong in [project status](project-status.md). Relevant
boundaries include text/wire limits, old revisions, private errors, current authorization, retained
response bodies and publication-reference-to-Task composition. In-process HTTP and synthetic storage
do not establish a real listener, browser, proxy/TLS, deployment or restart/recovery acceptance.
