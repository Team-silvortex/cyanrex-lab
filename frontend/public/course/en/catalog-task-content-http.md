# Explicit catalogue task content HTTP adapter

Status: **C2-Y internal preparation included in source release 0.5.5**. Decision date: **2026-10-11**.
Source release: **0.5.5** (**2026-10-11**). This independent router translates
bounded HTTP requests into the existing [Session catalogue-content commands](session-task-content.md).
It selects exact definitions from trusted configuration rather than accepting client definition bodies.
It is not mounted in the running application and does not issue Sessions, connect the browser editor,
change live OpenAPI/SDK contracts, migrate records or execute assessment rules.

Dated `Unreleased` follow-ups and verification records retain their original source versions and
execution scope; inclusion in 0.5.5 does not rerun or extend that evidence.

## Trusted composition

On Unix, `platform_http::CatalogTaskContentHttpState::new(source, workspace, origin)` takes an explicit
`DurableAuthSource`, typed `SessionCatalogTaskContentWorkspace` and canonical allowed origin.
`build_catalog_task_content_router(state)` returns an independent Axum router. Construction does not
read environment configuration, install storage, open a listener or modify `AppState`/`build_router`.

The workspace retains its frozen definition catalogue, including explicitly composed packages and
definitions with both assessment bindings or with neither. Requests choose an exact admitted reference,
not a catalogue, provider, namespace or mode. An unbound definition still creates a catalogue Task with
`Some(TaskDefinition)`; it is not manual work or an automatic pass. Complete-definition equality and
all existing text/Session checks remain authoritative.

`new` allows four in-flight requests. `with_capacity(source, workspace, origin, max_in_flight)` accepts
an integer from **1 to 16**; invalid capacities return `InvalidCapacity`. Clones and separately built
routers using those clones share one budget. Independently constructed catalogue, manual or text
states have independent budgets, so their admitted capacities add rather than form a global quota.

The origin must be canonical HTTPS or HTTP on localhost/a literal loopback address. Configuration
does not establish TLS, proxy trust or a secure eventual listener. The host must separately arrange
isolated source/storage installation, protected ingress and delivery of a valid durable Session.

## Request and response contract

Only this explicitly constructed router exposes the `/platform/v1/catalog-tasks` prefix. Task path
IDs are canonical UUIDs. The four methods and suffixes match the manual adapter, but creation requires
an exact `definition_ref` in addition to the existing manifest:

| Method and path | Exact request fields | Success |
|---|---|---|
| `POST /platform/v1/catalog-tasks` | `schema_version: 1`, `task_id`, `definition_ref`, `manifest` | 201 with the committed catalogue Task and manifest |
| `GET /platform/v1/catalog-tasks/{task_id}` | No body or query | 200 with the committed Task, manifest and ordered text contents |
| `PUT /platform/v1/catalog-tasks/{task_id}/content` | `schema_version: 1`, `expected_revision`, `manifest` | 200 with the committed replacement |
| `POST /platform/v1/catalog-tasks/{task_id}/status` | `schema_version: 1`, `expected_revision`, `status` | 200 with the committed state change |

`definition_ref` is an object with exactly `package`, `name` and `version`. Its `package` is an object
with exactly `name` and `version`, for example:

```json
{
  "package": {"name": "example.editorial", "version": 3},
  "name": "example.editorial.prepare",
  "version": 1
}
```

The example is admissible only when the host configured that exact entry. Names and positive JSON-safe
contract versions use the [definition contract](task-domain-boundary.md); there is no latest fallback,
catalogue discovery or publisher authentication. Root, reference and package records must be objects;
unknown or duplicate fields, positional arrays, malformed names and invalid versions are rejected.
Missing `definition_ref` cannot default to manual work. Clients cannot send a full definition, owner,
namespace, assessment policy or a selector that changes manual/catalogue mode.

Creation copies the complete configured definition. Replacement and status requests cannot replace
that definition; the existing commands preserve its assessment-binding choice, exact identity and
manifest invariants. They verify the complete stored metadata, old/new owned text, expected Task
revision and current Session. A wrong-mode record is rejected rather than promoted or reinterpreted.
No provider, typed Evidence, assessment, Review, execution or Task acceptance is dispatched.

Writes return `{schema_version: 1, task, manifest}`; reads also return ordered `contents`, each
`{artifact, text}`. These are the shared projections of confirmed Session results, not a detached
file read. Manifests allow zero to 32 exact text pins, each at most 256 KiB. Supplying a manifest does
not publish new bytes; [C2-X text publication](text-artifact-http.md) remains an independent command.

## Shared HTTP admission

The catalogue adapter reuses the [manual adapter's security and transport rules](task-content-http.md):
one `cyanrex_platform_session` Cookie, one `x-cyanrex-platform-request: 1` marker, an exact configured
Origin for mutations, no Authorization header, no legacy-cookie authority, no CORS and no Referer fallback.
The marker is not a secret CSRF token, and the reusable durable token gains no new audience binding.
This adapter neither issues nor clears cookies.

The complete JSON request is limited to **64 KiB** with a **two-second body-read deadline**. Queries,
GET bodies, duplicate media types, unsupported encodings, trailers and unsupported methods are
rejected under the same shared checks. Endpoint/header checks precede fail-fast admission and body
collection; a full budget returns `503 capacity_exhausted` with `not_attempted`, without polling the
body or calling a Session command. There is no admission queue or `Retry-After`.

An admitted slot remains held through collection, the existing Session command, serialization and
the response body's terminal frame/end, error or drop, including error responses. Handler return alone
does not release it. Cancellation releases admission, not a rollback guarantee or a stop signal for
already-dispatched blocking I/O. The existing ten-second Session command deadline remains unchanged;
there is no extra competing mutation timeout, response-delivery deadline or process-wide memory quota.

## Errors and uncertain outcomes

All responses use the shared private `no-store`/`nosniff` policy. Errors expose only the versioned
`error.code`/`error.outcome` envelope, not catalogue contents, ownership or storage diagnostics.

| Boundary | HTTP result | Outcome |
|---|---|---|
| Malformed request or exact-reference JSON before Session dispatch | 400 `invalid_request` | `not_attempted` |
| Exact definition absent from trusted configuration | 400 `invalid_request` | `unconfirmed` |
| Complete-definition drift, wrong stored mode or invalid referenced bytes | 503 `service_unavailable` | `unconfirmed` |
| Capacity exhausted before dispatch | 503 `capacity_exhausted` | `not_attempted` |

An unknown definition is mapped after calling the Session command, even though its internal lookup
can reject before database work. It is therefore not HTTP rollback evidence. All dispatched service
errors remain conservatively `unconfirmed`; missing and other-owned Tasks share the same private
not-found response. No error authorizes retry, cleanup, adoption or a fabricated confirmation.

## Manual compatibility and verification boundary

`TaskContentHttpState` and `/platform/v1/tasks` remain manual-only. Their request JSON still rejects
definition fields, and their typed workspace does not admit catalogue records. The new router has its
own fallback and is not automatically merged with either the manual or text router. A trusted host
must deliberately compose any listener; clients cannot switch modes or discover its catalogue.

The dedicated in-process HTTP target and exact disposable PostgreSQL runner exercise this adapter,
while source guards check typed composition and absent application/API/SDK/browser wiring. Executed
counts and omissions belong in [project status](project-status.md), separately from source inventory.
These checks do not establish a live listener, proxy/TLS deployment or browser save workflow.

C2-P–W draft publication, observation and journal workflows remain separate and manual-only. C2-Y
does not call them, enforce their journal or add atomic publication/Task creation. Session issuance,
public deployment composition, browser mapping, conflict/uncertain-outcome handling, migration,
sharing, Review authority and execution remain independent boundaries.
