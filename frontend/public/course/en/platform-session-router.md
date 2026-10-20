# Shared platform login and Session router

Included in source **0.6.3**, `PlatformSessionHttpState` and `build_platform_session_router` combine
the existing [login](platform-login-http.md) and [Session lifecycle](platform-session-lifecycle.md)
adapters over one retained storage binding, under
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). The combined router remains
unmounted in the normal teaching application. The explicit [loopback host](platform-loopback-host.md)
and [native HTTPS mode](platform-loopback-tls-host.md) now include its three routes through a separate
[workspace composition with Task metadata](platform-task-index-connection.md); this builder remains Session-only.

## One original binding

`PlatformSessionHttpState::new(storage)` consumes a genuine `PlatformStorageConnector`, such as one
returned by the [expected-storage opener](platform-storage-open.md). No second Origin, Source,
workspace, namespace, pool or arbitrary pair of routers can be supplied. Construction performs no
database or filesystem IO and is not a health check.

`with_capacities(storage, login_capacity, lifecycle_capacity)` validates the original Origin and both
capacities before building either capability or HTTP budget. Each capacity is 1–16; `new` uses 4 and 4.
The original Origin, catalog/marker observations, Artifact-root descriptor and connection options are
shared privately behind one Arc. Nothing reopens, rediscovers or replaces the original baseline.

Login and lifecycle remain separate command capabilities. `PlatformSessionConnector` gains no login
method, and `PlatformLoginConnector` gains no query/logout method. Their public constructors still
consume storage, without Clone, Debug, serialization or a storage accessor. The sharing constructor
and internal Origin accessor are crate-private implementation details, not new caller authority.

Each command still opens fresh connections and runs its original transaction-owned guards. Sharing
a storage handle adds neither a connection pool nor cached Session authorization. A root or marker
replacement after construction must be rejected by both command paths.

Both capabilities use the same configured database role. Composition neither checks nor adds the
business privileges needed for login or logout; a narrow query-only role stays narrow. Missing
privileges still fail privately through the original command, and constructor success does not prove
permission readiness or process isolation.

## Routes and responses

The builder combines only private fallback-free route sets, then installs one private fallback and
one response-header layer. Existing complete standalone routers retain their old fallback behavior.

| Exact route | Method | Existing operation |
|---|---|---|
| `/platform/v1/session/login` | POST | Confirmed existing-account login and dedicated Cookie projection |
| `/platform/v1/session` | GET | Fresh current-Session validation and minimal authenticated status |
| `/platform/v1/session/logout` | POST | Exact Session logout; clear Cookie only after confirmed success |

Every other path returns private 404 without reading credentials or body. Known paths preserve their
method/Allow, query, strict JSON or empty-body, Origin/marker and Cookie checks. HEAD has private headers
and an empty body. There are no teaching, Task, Artifact, registration, CORS or execution routes.

Login still requires exact Origin and marker and refuses any existing platform Cookie. Lifecycle GET
requires the marker and dedicated Cookie, with exact Origin if present; logout requires exact Origin
as before. Neither legacy `cyanrex_session` nor forwarded headers select authority or transport policy.

Successful login alone issues the existing single sensitive host-only Cookie. Confirmed logout alone
clears its name and Path with the existing policy. HTTPS retains Secure; canonical loopback HTTP omits
only Secure. JSON contains no token, account, role or reusable resource permission. The composite does
not rewrite handler errors, retry commands or place a new timeout around dispatched mutations.

## Independent admission lanes

Login and query/logout have distinct response-lifetime semaphores. A held login response or exhausted
login rate budget does not spend a lifecycle slot; query/logout saturation does not spend a login slot.
GET and logout share the lifecycle lane, so a held GET response can temporarily block logout.

Clones and routers built from them share both lanes and the same login attempt context. Login retains
its state-wide 64 eligible requests per 60-second budget, charged before login capacity with no refunds
or success reset. Query/logout do not consume or reset that counter. Header-rejected login requests
still do not spend it. A new separately constructed state has independent, additive HTTP budgets.

Default capacity is therefore four login plus four lifecycle slots, not one global four-slot limit.
This is not server connection/header admission, fairness, distributed abuse protection or a whole-host
memory/worker quota. Cancellation releases HTTP leases without proving that dispatched password work
or remote SQL has stopped. Other surviving clones are not revoked by dropping one state handle.

## Delivery uncertainty and remaining host work

A fresh GET result is a snapshot, not authorization for later resource work. A server-side confirmed
login/logout response does not prove Cookie delivery to a browser. Cancelling delivery may leave a
consumed OTP and issued Session, or a deleted Session with the browser's old Cookie still present.
There is no automatic compensation, OTP reset, token adoption, retry or recovery.

Sharing does not solve concurrent no-Cookie login delivery or differently scoped Cookie pollution.
The existing [Cookie and unknown-delivery limits](platform-login-http.md) still apply. Actual process
preparation, trusted private startup input and host-wide ingress limits are composed only by the
explicit loopback host. TLS and Cookie-safe domain planning remain independent requirements. No normal
AppState, frontend authentication, OpenAPI or SDK route is added. Task content, Artifacts and browser
saving remain unconnected; the separate workspace builder adds only the owner-private index.

## Verification

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_http::session_router
cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_router_tdd
bash scripts/test-platform-session-router.sh
```

Default fixtures verify combined routing, rejection and independent admission. Their closed-Source
lifecycle fixture establishes private backend failure, not successful authorization. The exact SQL
runner requires a disposable `CYANREX_TEST_DATABASE_URL`, opens one genuine binding for each combined
state, and verifies real password/OTP issuance, query/logout, shared budgets, original drift, permission
failure and cancellation. Held-response tests prove server commit before cancelled delivery, not a
lost COMMIT acknowledgement, actual browser receipt or LAN/TLS acceptance.
