# Durable Session stock limits

Included in source **0.6.0** on **2026-10-15**; see [release validation](../../reports/releases/0.6.0/README.md).
The dated implementation scope below remains historical, not a new deployment acceptance.

The 2026-10-13 Unreleased follow-up after 0.5.9 adds explicit stock-limited login preparation under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). This is a trusted
Rust entry point, not a public login route or a host. The [Session lifecycle router](platform-session-lifecycle.md)
remains unmounted and still only reads current authentication or confirms exact logout.

## Explicit configuration

`SessionStockLimits::new(per_account, total)` accepts a per-account limit of 1–64 and a total limit
of 1–4096, with the account limit no greater than the total. Invalid configuration returns
`DurableAuthError::InvalidInput`. Both values use `u16` and have read-only getters. The policy has
no default, request deserializer, environment lookup or installation side effect.

Trusted composition passes the policy to
`DurableAuthSource::login_with_session_limits(username, password, otp, limits)`.
It retains the original source, password-work admission, attempt window, exact account incarnation,
credential checks, atomic OTP consumption, original namespace/relation pins and confirmed commit.
No teaching role or reusable resource permission is granted.

The existing prepared `login` entry point remains unchanged in behavior and does not enforce this
stock policy. The live teaching AuthService is also unchanged. A future host must explicitly use
the bounded entry point with one consistently reviewed policy and isolate other writers. These
limits are not a persisted schema quota, a guarantee against direct SQL writers or a process-wide
budget across unrelated sources or policies.

## What occupies a slot

The total covers **every stored Session row in the exact original source namespace**, including
expired, orphaned or malformed records. The account stock uses its exact `account_id`, not a
username, role or supplied snapshot. Counting does not read token digests, passwords, OTP secrets
or timestamp payloads and does not filter expired or invalid rows out of the stock.

Each SQL aggregate is limited to the configured threshold plus one row. After the bounded total
check, the account predicate searches that same source under its writer fence. This bounds count
projection and normal cooperating-writer work; it is not a database-size or query-time guarantee
against unrelated writers, historical storage, table bloat or an untrusted database operator.

Stock refusal returns the existing `RateLimited` error before OTP consumption or Session insertion.
That error also covers the existing password-work and attempt gates, so it is not a public quota
diagnostic. Login never deletes an old device or automatically prunes expired Sessions. Capacity
can be released through explicit confirmed logout or the existing trusted bounded maintenance
operation; corrupt maintenance candidates are not silently repaired or discarded.

## Transaction safeguards

Only the new bounded path explicitly sets its second, writer transaction to `READ COMMITTED`
before any read. A caller's database default of repeatable read must not retain an old stock
snapshot after waiting for the source fence. The operation does not force a read-only database
role or connection into read-write mode.

The available stock is checked after current credentials and OTP preparation, before the original
compare-and-update consumption. After consumption and Session insertion, the bounded path runs
`SET CONSTRAINTS ALL IMMEDIATE`, then rechecks the original source pins, credentials, watermark and
exact new Session. This brings the queued deferred work forward instead of relying only on
commit-time effects; it is not protection against privileged trigger reconfiguration or other writers.

The final total and account counts must each equal the original count plus one and remain within
the limits. Unexpected net cardinality changes or over-limit counts return `StorageUnavailable`, not
a token. The final SQL check is followed by the original pure OTP recheck and confirmed COMMIT,
with no intervening SQL. Counts prove this cardinality transition, not complete integrity of every
other Session row or the absence of all possible privileged database side effects.

A timeout, cancellation or lost commit acknowledgement still does not establish rollback. There
is no automatic retry, OTP reset, adopted token, storage migration or hidden cleanup fallback.

## Verification and remaining work

Default policy tests cover explicit values, both supported boundaries, invalid limits and account
limits above the total. Disposable SQL regressions use real prepared registration, original login
transactions and a private test-only OTP clock. They cover full stock without OTP consumption,
explicit capacity release, stored invalid/expired rows, post-write faults and independent-source
competition for the final slot. Failure snapshots contain only private SQL digests and counts.

Run the separate SQL inventory with an explicit disposable `CYANREX_TEST_DATABASE_URL`:

```bash
./scripts/test-durable-session-stock.sh
```

The runner verifies each exact test selector. CI selects it and the earlier Session lifecycle SQL
suite using the existing disposable PostgreSQL service. Neither suite may target application data;
these tests do not establish a mounted platform, public issuer or LAN deployment.

Ingress/rate policy, timing disclosure, secure secret delivery, unknown-commit handling, actual
low-privilege host composition and transport acceptance remain separate gates before public login.
Schema 2, old prepared login, current teaching authentication, UI, SDK and OpenAPI stay unchanged.
