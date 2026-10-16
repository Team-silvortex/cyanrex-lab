# Platform Session lifecycle

A **2026-10-16 Unreleased** [retained-storage backend](platform-session-connection.md) adds an
explicit alternative to the original pooled Source state. Its Origin comes from the bound config;
transaction guards and confirmed Cookie behavior remain required. The router is still unmounted.

Included in source **0.6.0** on **2026-10-15**; see [release validation](../../reports/releases/0.6.0/README.md).
The dated implementation scope below remains historical, not a new deployment acceptance.

The 2026-10-13 Unreleased follow-up after 0.5.9 adds an explicit, **unmounted** Session lifecycle
adapter. It can read a current durable Session and confirm logout. It does not issue Sessions,
start a platform host or change the teaching application's authentication. The next stages follow
the [independent low-privilege platform host proposal](https://github.com/Team-silvortex/cyanrex-lab/issues/3).

This boundary is for a future same-origin platform host. Constructing its state neither connects to
storage nor initializes it; trusted composition must supply an already prepared `DurableAuthSource`
and one canonical HTTPS origin. Canonical loopback HTTP is allowed explicitly for local development,
not for plaintext LAN deployment.

## Exact operations

| Request | Confirmed response | Cookie effect |
|---|---|---|
| `GET /platform/v1/session` | `200 {"schema_version":1,"authenticated":true}` | None |
| `POST /platform/v1/session/logout` | `200 {"schema_version":1,"ok":true}` | Clear only the dedicated platform Cookie |

GET calls the original Source-owned `validate_session` operation. Missing, expired, revoked or
same-name recreated accounts cannot authenticate an old Session. The response exposes no account,
role, expiry, token or resource. An authenticated snapshot is not reusable Task, Artifact or file
permission: each later resource operation still needs its original transaction-owned authority.

POST calls the original Source-owned `logout` directly, without a separate authentication precheck.
The Source can also confirm absence of a canonical unknown token. Only confirmed success permits
Cookie clearing; this is not permission to clear every device's Session or the old teaching Cookie.

Exact known paths reject any query. Extra components and trailing slashes are not aliases. Wrong
methods return `405` with `Allow: GET` or `Allow: POST`; HEAD keeps private headers and an empty body.

## Browser and transport boundary

Requests require exactly `x-cyanrex-platform-request: 1` and one canonical, non-nil UUID token in
`cyanrex_platform_session`. Duplicate platform Cookie values, malformed tokens, bearer credentials
and legacy-only `cyanrex_session` are rejected. Cookie header bytes retain the existing 8 KiB budget.
The legacy Cookie may coexist but is never selected or cleared.

POST requires the exact configured Origin. GET may omit Origin, but an included Origin must match
exactly. Referer is not an Origin fallback. Both requests require a truly empty body under the
existing two-second collection deadline; encoded bodies, incompatible media types, trailers and
body faults fail before any Source operation. There is no CORS permission or redirect fallback.

Confirmed HTTPS logout returns exactly one host-only clearing header:

```text
cyanrex_platform_session=; Path=/platform/v1; HttpOnly; SameSite=Strict; Max-Age=0; Secure
```

Explicit loopback HTTP omits only `Secure`. No Domain attribute is set. A future issuer must use
the same name and Path policy; this adapter supplies no issuer. A successful server response does
not prove that the browser received or applied the clearing header.

All responses use `Cache-Control: no-store`, `Pragma: no-cache` and
`X-Content-Type-Options: nosniff`, without CORS, Location or Retry-After headers.

## Failure and capacity semantics

Errors retain the private shape `{"schema_version":1,"error":{"code":...,"outcome":...}}`.
Pre-dispatch rejection reports `not_attempted`. A Source-confirmed invalid Session returns `401`
with `invalid_session`; Source failures return generic `503 service_unavailable`. Both dispatched
errors conservatively report `unconfirmed` and never clear a Cookie or expose storage diagnostics.
An unknown outcome does not establish rollback or authorize automatic logout retry.

Credential checks precede fail-fast capacity admission, which precedes body collection and Source
work. State clones share a default four slots, configurable only within 1–16. A slot lasts through
the admitted request and its unsent response body until completion or drop. Separate new states
have separate budgets. This is not a global Session stock limit, SQL-worker budget or proof of
delivery; cancelling a request does not prove its database transaction rolled back.

## Verification

`engine/tests/platform_session_http_tdd.rs` contains eight default transport regressions and seven
explicitly ignored disposable PostgreSQL cases. The SQL cases cover current and invalid Sessions,
exact logout with another device preserved, HTTPS and loopback clearing, suppressed or redirected
DELETE, actual row-lock expiry, DELETE lock timeout, response capacity and cancellation with actual
database-wait evidence. Fresh random namespaces are removed before final behavior assertions.

Run the default target without a database configuration:

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_http_tdd
```

For the separate SQL suite, supply an explicit disposable `CYANREX_TEST_DATABASE_URL`, then run
`./scripts/test-platform-session-http.sh`. Never point fault-injection tests at application data.
The runner verifies each exact test name before execution and does not fall back to `DATABASE_URL`.
Tests and source inventories do not establish a mounted service or deployed LAN security.

## Remaining host and issuer work

Normal application mounting, login/enrollment, secret delivery, UI/SDK/OpenAPI changes, database
installation or migration and implicit expired-Session cleanup are deliberately absent. Existing
AuthService, content routers, Source schema and original transaction guards remain unchanged.

The independent host still needs a genuinely unprivileged process, explicit database roles and
namespaces, owned private Artifact storage and secure same-origin LAN transport. Public Session
issuance additionally needs ingress/rate policy, active Session stock bounds, timing policy and a
safe unknown-commit/secret-delivery protocol. Only after these gates should private Task saving,
listing, reopening and revision-conflict handling connect to that host.

A subsequent [explicit Session stock policy](durable-session-stock.md) bounds stored rows during
trusted limited login, including expired records. It does not turn this router into an issuer,
force policy on old prepared writers or complete the remaining host and issuance gates.

An Unreleased follow-up after 0.6.0 verifies a [credential-restricted database role](platform-session-permissions.md)
for this query/logout boundary. It removes the Source-only table-wide read dependency, not the
remaining process/transport/issuer gates or registry-backed commands' separate privilege needs.
