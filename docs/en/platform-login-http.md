# Explicit platform login HTTP adapter

Included in source **0.6.3**, the independent `SessionLoginHttpState` and
`build_session_login_router` project [retained login](platform-login-connection.md) into an explicit
HTTP adapter under [host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3).
The standalone router is unmounted: it starts no listener, installs no storage, changes no teaching
login and does not connect browser saving. The explicit [loopback host](platform-loopback-host.md)
now runs its routes through shared composition; LAN/TLS and browser integration remain separate.

The separate [shared Session router](platform-session-router.md) now combines this adapter with
query/logout over one original storage binding, without changing this standalone router's scope.

## Trusted composition

`SessionLoginHttpState::new(connector)` consumes one `PlatformLoginConnector`, retaining it behind
an Arc for the state's lifetime. `with_capacity(connector, capacity)` accepts 1–16 response-lifetime
slots, default 4. Origin comes only from the original connector; no request, environment variable,
teaching Cookie, pooled Source fallback or forwarded address can select the backend or Cookie policy.

State clones and routers built from those clones share the connector, rate counter and admission
slots. Separately constructed states have independent HTTP budgets. The existing
[Session lifecycle](platform-session-lifecycle.md) state still handles only query/logout and returns
404 for login; it does not acquire issuance rights merely by sharing the path prefix.

## Request contract and order

Only exact `POST /platform/v1/session/login` is accepted. Queries, aliases and trailing slashes are
not accepted; other methods return 405 with `Allow: POST`. HEAD retains private headers and no body.

The handler checks these gates in order:

1. Exactly one `x-cyanrex-platform-request: 1` and one exact configured Origin. Referer is not a
   fallback; Authorization is rejected. Cookie headers share an 8 KiB budget. Any occurrence of
   `cyanrex_platform_session`, including empty, malformed, duplicate or bare-name forms, returns
   `409 session_present` without inspecting its validity, reading the body or touching storage.
   An unrelated `cyanrex_session` can coexist but is never selected, changed or cleared.
2. One fixed global counter per state admits at most 64 eligible requests per 60-second monotonic
   window, shared across all names and clients. Capacity admission follows it and never queues.
   Busy, malformed, cancelled and successful requests do not refund an already consumed rate count;
   successful login does not reset it. Rejected header checks consume neither budget.
3. Collect at most 32 KiB of actual streamed JSON within two seconds. Reject Content-Encoding,
   unsupported or duplicate Content-Type, trailers, body errors and nonterminating empty frames.
   Content-Length does not substitute for actual bytes or EOF.
4. Parse one strict object containing only `schema_version: 1`, `username`, `password`, `otp`.
   Reject arrays, duplicate/unknown keys, trailing JSON and invalid types. Username uses the existing
   `LegacyUsername` contract; decoded password is at most 4096 bytes and OTP at most 64 bytes.
   The HTTP adapter does not trim or normalize submitted credentials.
5. Invoke the retained connector exactly once. Its own transaction, OTP and stock policy remains
   authoritative. No HTTP handler timeout, automatic retry or follow-up validation is added around
   dispatched login; confirmed success proceeds directly to response projection.

The state-wide rate limit is a bounded resource policy, not fair per-client service, distributed
abuse prevention or an HTTP server connection/header limit. A client can exhaust the shared budget;
changing names, X-Forwarded-For or Forwarded does not create another bucket.

## Confirmed Cookie delivery

Confirmed success returns only `200 {"schema_version":1,"authenticated":true}`. It emits exactly
one sensitive Set-Cookie header, with no token, username, role, expiry or database detail in JSON:

```text
cyanrex_platform_session=<new-token>; Path=/platform/v1; HttpOnly; SameSite=Strict; Secure
```

Canonical loopback HTTP, including the existing localhost policy, omits only Secure. The choice
depends on the frozen Origin, never Host or proxy headers. No Domain, Max-Age or Expires is added:
this is a browser session Cookie, with the existing server-enforced twelve-hour Session expiry.
Clearing remains the existing lifecycle command's confirmed, same-name/same-Path policy.

This rejects replacement of a Cookie already present on the request, not every possible concurrent
delivery. Two concurrent no-Cookie login requests can still issue Sessions; response order is not
idempotency or cross-tab coordination. Use explicit logout before switching a canonical existing
Session. Malformed or differently scoped Cookie pollution may require deliberate browser cleanup;
the adapter does not guess Path/Domain values to clear.

Cookie attributes do not appear in the request Cookie header, and cookies are not isolated by port.
A different localhost port is not a separate Cookie security domain. See
[RFC 6265 sections 4.2.2 and 8.5](https://www.rfc-editor.org/rfc/rfc6265#section-8.5).

## Failure and unknown delivery

Before connector dispatch, failures use the existing private error shape with `not_attempted`.
Every connector error uses `unconfirmed`: invalid credentials map to 401, rate/stock refusal to
429, and remaining source/storage failures to generic 503. No failure sets or clears a Cookie,
echoes credentials, advertises Retry-After or grants CORS. All responses retain no-store, no-cache
and nosniff. An invalid backend token is also refused without exposing it.

The permit covers admitted body collection, backend waiting and the unsent response until body
completion/drop; it does not track an already dispatched password worker after request cancellation.
The connector retains its ten-second work budget, separate final-close wait and non-preemptible
synchronous filesystem observations. The two-second body limit is not a whole-request deadline.

A successful server response does not prove the browser received or applied the Cookie. Cancellation
after confirmed commit can leave an issued but undelivered Session and consumed OTP. Never infer
rollback, automatically repeat login, adopt a stored token, reset OTP or delete a Session to compensate.

## Verification and remaining host work

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::session_login
cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_http_api_tdd
bash scripts/test-platform-login-http.sh
```

Default private transport fixtures establish header/body/order/rate/admission/projection behavior,
not real authorization. The exact SQL runner requires only an explicit disposable
`CYANREX_TEST_DATABASE_URL` and uses genuine restricted roles, password/OTP login and retained storage.
Its held-response delivery test proves confirmed commit before cancellation, not loss of COMMIT
acknowledgement, actual browser receipt or deployed TLS.

Host startup, actual TLS/ingress limits, Cookie-safe domain planning and the independent platform
browser auth context remain unfinished. The old `/auth/login` and role-based Sidebar cannot be
switched to this URL alone. Task/Artifact/index composition, UI/SDK saving and restart recovery
remain separate work; no live routes, OpenAPI or SDK contracts are changed by this adapter.
