# Login through retained platform storage

Included in source **0.6.3**, `PlatformLoginConnector` issues an existing account's Session through an
explicit retained storage binding. It follows [host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)
and reuses the prepared Source login algorithm. It does not start a host, expose HTTP login, set a
Cookie, enroll an account or connect browser saving.

The separate [login HTTP adapter](platform-login-http.md) now projects this capability into bounded
requests and a confirmed dedicated Cookie. It remains independently constructed and unmounted.

## Explicit login capability

`PlatformLoginConnector::new(storage)` consumes a genuine `PlatformStorageConnector` without IO.
It freezes authority, Origin and Session stock limits from that original configuration; `origin()`
returns only public metadata. `login(username, password, otp)` is a complete command, returning the
existing secret `DurableLogin` only after confirmed COMMIT. Neither connector nor login secret is
Debug/Serialize; the connector has no Clone, pool, secret getter or baseline updater.

The [query/logout connector](platform-session-connection.md) remains separate and gains no login
method. Constructing a query-only HTTP state does not grant credential-reading or issuance rights.
The caller must prepare the [Linux process](platform-process-startup.md), original catalog/markers,
private Artifact root and a separately provisioned runtime database role before constructing either
capability. No CLI configuration format, installer, migration or live AuthService is changed.

## Password gap and commit order

The old pooled Source and retained login share private login stages rather than independent copies
of credential, OTP and stock checks. Those stages are not public partial commands or reusable grants.

1. Open a fresh original-storage connection. In its business transaction, verify retained catalog and
   markers, capture the Source namespace/relation identity and read the original account credentials.
   Recheck retained storage before confirming the read transaction and closing that connection.
2. Run password verification on the existing bounded worker without holding a database connection
   or Source lock. A genuinely missing account still performs one synthetic verification, but cannot
   become a later account with the same name. Storage or invalid-record errors are not dummy fallbacks.
3. Open another connection through the same original binding. Use READ COMMITTED, recheck the first
   Source identity and current account incarnation/credentials, and acquire the Source writer fence.
4. Enforce the original configured Session stock limits before OTP consumption. Consume the OTP,
   insert exactly one hashed-token Session and flush deferred constraints. Recheck original retained
   storage before the final Source, credentials, watermark, exact Session and stock proofs.
5. Recheck fresh OTP validity immediately before attempting COMMIT, with no subsequent SQL guard.
   Only confirmed COMMIT publishes `DurableLogin`; a later graceful-close failure does not undo it.

The token has the existing twelve-hour lifetime. It is not logged, persisted in plaintext or returned
by a public route here. The trusted caller owns secure delivery; a lost response or cancelled waiter
can leave an issued but undelivered Session. Do not adopt a matching row, reset OTP consumption,
automatically retry, delete Sessions or rerun bootstrap to resolve an unknown result.

## Limits and permissions

Retained login always uses `config.limits()`; there is no unlimited overload. Counts include expired
Session rows and are checked before and after insertion under the Source writer fence. Separate
connectors therefore serialize OTP/stock decisions in the database. Existing legacy unlimited login
remains separate; do not mix unreviewed writers into the same activated source.

Each connector retains the existing bounded attempt policy: five admitted attempts per username in
five minutes, at most 1,024 tracked names, and clearing that username after confirmed success.
The window starts with that name's first admitted attempt.
Sharing one connector shares that state; separately constructed connectors have independent maps.
The process-wide password gate still owns at most four dispatched jobs and twenty admitted jobs;
actual jobs retain their permits after waiter cancellation. This is not host ingress rate limiting,
distributed abuse prevention or constant-time authentication.

Login requires explicitly granted credential SELECT, OTP-counter UPDATE, Session INSERT/readback
and the existing Source-row locking rights, in addition to original catalog/marker observations.
It does not grant them itself or require schema/database ownership, schema creation, account
registration or Session deletion. The narrower query/logout role remains credential-restricted;
do not silently upgrade that role to make login pass.

Command waiting has one ten-second budget, with independent final graceful-close waiting.
Synchronous filesystem observations cannot be preempted by that timer. Root identity is checked
when admitting each fresh connection, not continuously locked during SQL. Catalog/marker and Source
checks retain their transaction-owned order; none establishes permanent storage health or kernel isolation.

## Verification and next boundary

Default API checks and the exact disposable PostgreSQL runner are separate:

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_api_tdd
bash scripts/test-platform-login-connection.sh
```

The SQL runner requires an explicit disposable `CYANREX_TEST_DATABASE_URL` with database/role
administration and runtime-role password authentication. It checks each exact test selector before
running it. Fixtures use synthetic credentials, actual restricted roles and private roots; they do
not test or modify application accounts.

Public issuance still needs an actual host and safe same-origin HTTPS transport. The unmounted
adapter supplies bounded pre-authentication gates and Cookie projection, not browser receipt. Host
startup and Task/Artifact/index commands must
consume the same reviewed binding before UI/SDK save/list/reopen can be connected. Current teaching
authentication, the unmounted lifecycle router and local-only Task editor remain unchanged.
