# ADR-006: Durable account incarnations and session source

Date: **2026-09-27**. Status: **C1-E internal staging, included in source release 0.4.6**.
Builds on [audited identity lifecycle commands](collaboration-identity-lifecycle.md). This is not an
AuthService cutover, existing-account migration, public endpoint or deployment change.

Follow-up: [ADR-007 / C1-F](collaboration-session-commands.md) composes current-session verification
with binding/policy commands, without exposing deletion/retirement or switching live authentication.
The original C1-E scope and verification are retained below; dated follow-ups separately describe
the Session-boundary, password-work, stored-hash and OTP-freshness changes after 0.5.0,
the pure OTP-consumption policy, and its subsequent schema-2 atomic integration, all included in 0.5.1.
Earlier dated sections retain their then-unreleased implementation stage; the latest section defines
the current prepared format. Source inclusion does not enable live authentication or migrate existing data.

## Decision and scope

The [durable authentication source](../../engine/src/services/auth_service/durable_source/mod.rs)
accepts an injected PostgreSQL pool and an explicitly pinned `AuthorityId`. It has no environment
configuration, seeded teacher, in-memory credential/session store or database-failure fallback. One schema
belongs to one source authority; opening it with another authority fails rather than creating a new one.

Registration commits a random non-nil `LegacyAccountId` alongside username and credentials. Sessions
reference **both username and that exact account incarnation**. A deleted and recreated name receives
another incarnation, so it cannot inherit an old session or registry identity. User/session coordinates
and session digests cannot be updated in place through the installed schema.

| Internal operation | Confirmed result and boundary |
|---|---|
| `install_empty_schema` | Explicit installation on absent or compatible empty auth tables only; no account import/backfill |
| `register` | New account incarnation and TOTP setup returned after confirmed commit; no Principal, role, Membership or Grant |
| `login` | Password and TOTP verification, current-incarnation recheck and committed session; only its SHA-256 token digest is stored |
| `validate_session` | Fresh read-only snapshot of the exact unexpired account/session pair; no cleanup, role lookup or allocation |
| `logout` | Confirmed removal of this exact session, idempotent when already absent; not account retirement or device/kernel cleanup |

These are trusted adapter primitives. In particular, registration is not public enrollment or teacher
bootstrap. `DurableAccountRef` and `DurableSession` are forgeable data/snapshots, **not an authorization
capability** for a later identity or policy command. No routes, AppState fields or existing AuthService
behavior are switched. The existing HTTP/SDK surface and teacher/student workflow remain unchanged.

## Explicit installation, never inferred migration

At the original C1-E introduction, [the installer](../../engine/src/services/auth_service/durable_source/schema.rs) serialized explicit
installation using a schema-scoped advisory lock. It locks both auth tables before checking emptiness,
verifies legacy column types, NOT NULL constraints, primary keys and cascading foreign-key targets,
then applies the new [0010 template](../../engine/migrations/0010_durable_auth_source.sql) and records
source schema version 1 with its pinned authority. DDL, constraints and activation commit together.
Released templates 0001–0009 are unchanged; startup and reads do not invoke this installer.

Nonempty old tables are rejected without assigning IDs, rewriting credentials or invalidating sessions.
An empty table with a missing session primary key is also incompatible: successful column projection
does not prove uniqueness. A failing regression exposed this gap; catalog checks now reject it along
with missing foreign keys, nullable credentials and wrong timestamp types. The scoped
[maintenance instruction](../../.github/instructions/durable-auth-source.instructions.md) preserves this rule.

Repeat installation verifies existing structure, immutable-coordinate triggers, version and authority;
it does not repair missing state. New account coordinates have no default, so legacy inserts omitting
them fail at the SQL boundary. **This does not fence the old in-memory AuthService or every old writer**:
the old runtime can fall back, mutate credentials or delete rows. Never point both implementations at
the same activated schema. A real cutover needs a stopped/fenced old writer and a reviewed migration.
Schema checks and triggers protect cooperating code, not against a privileged database owner.

## Transactions and authentication

Source mutations take the metadata row `FOR UPDATE`; coherent reads take `FOR SHARE` before account
and session rows, including absent keys. This deliberately serializes writes per source across pools.
A session read waits for a pending logout, then observes its committed deletion rather than a cache.
The operation deadline is ten seconds, lock wait two seconds, SQL statement five seconds.

The original slice reused the existing Argon2 helpers off the async worker; the follow-ups below add
dedicated admission and a fixed prepared profile. Expensive verification holds no database transaction.
Login reads credentials, verifies password/TOTP,
then reacquires source/row locks and compares the full credential snapshot and exact incarnation before
inserting a session. It checks them again after insertion, since triggers can modify rows in the same
transaction. Inserts/deletes confirm affected rows and final readback before commit acknowledgement.
No account ID, setup secret or raw session token is returned before confirmed commit; secret-bearing
registration/login wrappers intentionally implement neither Debug nor Serialize.

Sessions expire twelve hours after issuance using fresh database time after lock waits. Reads check
fresh database time after their row locks, not transaction-start time. Expired sessions are rejected but
not silently deleted on reads. Cancellation, timeout and a lost commit response still **do not prove
rollback**. An uncertain registration is not retried with another identity; this slice has no durable
command receipt, secret redisclosure or automatic compensation/reconciliation API.

Login admission is bounded but **process-local**: clones share at most 1,024 username entries, five
admitted attempts per username in a fixed five-minute window. Admitted failures, including storage
errors, consume attempts; a confirmed login clears its entry. New independent source instances do not
share limits. Password input is capped at 4,096 bytes and OTP at 64 bytes; registration requires at least
eight password bytes. Distributed rate limits, public registration policy and HTTP/CSRF integration
remain gates before exposing this adapter to untrusted clients. The follow-ups below add prepared-only
process-local job admission and an explicit stored-hash cost profile.

## Next integration gate

### Unreleased Session-boundary hardening · 2026-10-03

`login`, `validate_session` and `logout` now pin the current namespace and the actual `users`,
`sessions` and source-metadata relations before operating. They require one effective private,
canonical schema, no temporary namespace, permanent ordinary tables, validated nondeferrable primary
keys, no row-level filtering or inheritance, and the existing source schema/authority contract.
These guards use only the three source tables: a registry is not required or implicitly installed.

Login retains its first namespace/table identities across password verification and checks them in
the second transaction; equal credentials in replacement tables are not the same source. After a
write, guards verify the current path without resetting it, then recheck relations, schema and
authority before reading back the result. Session freshness is checked after the guard's waits.
Logout confirms absence in that original source. A regression reproduced a suppressed DELETE whose
trigger redirected the follow-up read to an empty shadow table: the old code returned success while
the real Session remained usable. The new guard rejects and rolls back that transaction.

The raw `search_path` string must also stay identical within one operation, including both pooled
connections used by login. Different quoting that resolves to the same schema is still rejected;
this is not a global path pin across independent source instances or separate requests.

This deliberately tightens these three primitives: `public`, system or multiple effective schemas,
temporary namespaces and filtered/replaced relations are not accepted. Installation, registration,
password/deletion commands and live AuthService are not redesigned by this follow-up; it does not
make every primitive enforce identical connection rules, repair a schema, migrate data or authorize
a public endpoint. The shared private-work namespace rules and registry checks remain separate.
Database-owner tampering and uncertain commit outcomes are not turned into guarantees.

The follow-ups below bound prepared worker admission and hash costs, and add schema-2 OTP consumption.
An untrusted Session issuer still needs account-enumeration timing policy,
ingress/session limits and recovery composition. The username counter and dedicated cookie names do not
establish those properties. C2-N remains an unmounted content router, not a login flow.

Follow-up tests live in [the Session-boundary target](../../engine/tests/durable_session_boundary_tdd.rs)
and its supporting modules. The exact runner is
[`test-durable-session-boundary.sh`](../../scripts/test-durable-session-boundary.sh);
current execution results are recorded in [project status](project-status.md), not added to the
historical C1-E totals below.

### Unreleased password work admission · 2026-10-03

The prepared source's registration, login, password change and empty-authority bootstrap share one
process-local password-work gate, including independently constructed source instances and pools.
At most **4 jobs may be dispatched** to Tokio's blocking executor, with **20 admitted jobs in total**
including dispatched work and requests waiting for a worker slot. The fixed limits are a conservative
admission policy, not a benchmarked capacity claim. Excess admission returns `RateLimited`; there is
no unbounded internal waiting queue or new environment/configuration override.

Work waits within the operation's existing ten-second deadline, outside database transactions.
Password material is copied for the blocking job only after both slots are acquired. Cancelling a
request before dispatch releases its admission without starting computation. After dispatch, both
permits belong to the blocking closure, including while Tokio has not yet scheduled it; cancellation
or timeout of its caller does not free capacity early. Completion or unwind releases the slots, and
a worker failure returns `StorageUnavailable`, not an invalid-password result. Already-dispatched
computation may finish after its caller disappears; this does not resume the dropped database command.

The gate itself limits **job counts**, not a single job's memory, cost or duration. The separate
stored-hash follow-up below pins accepted parameters; neither is a wall-clock or process RSS bound. There
is no credential migration, OTP consumption policy, distributed quota or fairness guarantee among
sources. The old live AuthService helpers and synchronous default-teacher setup do not use this gate;
it is not an upper bound on all password work in the process. HTTP issuance and live cutover remain absent.

Deterministic lifecycle tests are kept with the private worker implementation, with an additional
source-wiring check to reject bypasses at the five prepared hash/verification call sites. Actual test
counts and adjacent database reruns are recorded separately in [project status](project-status.md).

### Unreleased stored password hash profile · 2026-10-03

Prepared account reads and password verification now reject unsupported PHC records before worker
admission or secret copying. The text is capped at 1,024 bytes before parsing. Accepted records use
`argon2id`, explicit `v=19`, exactly one each of `m=19456`, `t=2`, `p=1` (in any order), and a 32-byte
digest. Missing, duplicate, extra or noncanonical parameters are rejected, even if the underlying
parser accepts them. Salt must fully decode into a fixed buffer and contain 8–48 bytes; it need not
be a UUID or match the separate salt column. This is a cost/format policy, not a new salt-binding rule.

Registration, bootstrap and rotation explicitly write that same profile, matching the former locked
library defaults rather than following future default changes. Unsupported or malformed records return
`InvalidRecord`; a wrong password against a supported record remains `InvalidCredentials`. No enormous
cost is executed to test rejection. Preflight does not hand unchecked numbers to Argon2 parameter
conversion: duplicate lookup semantics and arithmetic overflow cannot bypass the local checks.

This deliberately tightens the prepared source only. It does not rewrite or adopt other credential
profiles, change schema versions, consume OTPs or revoke existing Sessions. Session validation and
logout retain their independent source checks. Existing live AuthService and synchronous teacher
initialization remain unchanged. The fixed 19,456 KiB Argon2 memory parameter is not a bound on process
RSS, aggregate legacy work or execution time; running jobs still cannot be interrupted by a deadline.
Public issuance, timing/OTP recovery policy, trusted ingress and reviewed migration remain separate gates.

The [profile SQL target](../../engine/tests/durable_password_profile_tdd.rs), pure profile tests and
worker wiring checks cover this boundary; current runs are recorded in [project status](project-status.md).

### Earlier OTP freshness stage · 2026-10-03

Historical stage: the later schema-2 integration below adds consumption to these freshness checks.

Prepared login and password rotation now check the submitted TOTP after password verification,
again behind the source writer fence, and after the last SQL read/time check before requesting
COMMIT. A code that leaves the accepted window during a lock, trigger or readback wait returns
`InvalidCredentials`. Login does not publish its pending token; rotation rolls back its credential
and Session changes when that pre-commit check fails. Existing namespace/credential pins, Session
expiry checks, lock order and acknowledgement rules still apply.

The algorithm and input behavior remain SHA-1, six ASCII digits after trimming, 30-second steps,
and one accepted step on either side of application UTC. Session expiry still uses fresh database
time: the two clocks are not silently merged or made immune to clock rollback. Negative Unix times
are rejected; the epoch does not cast a negative candidate counter to an unsigned value. No system
clock is changed by tests; a private per-source clock exists only in unit-test builds so real SQL
waits can be paired with deterministic OTP expiry.

This is **freshness, not single-use**. The same code may still succeed more than once in its accepted
window, including across independent sources and login/rotation. [RFC 6238 section 5.2](https://www.rfc-editor.org/rfc/rfc6238.html#section-5.2)
separately requires rejecting reuse after successful validation. Meeting that requirement needs a
durable, account-incarnation-bound consumption transaction and an explicit schema/recovery contract;
no memory cache or Session count is used as a substitute here. Public issuance remains blocked on
that policy, timing/ingress limits and the other composition gates.

The final check occurs before sending COMMIT, not at acknowledgement. Commit-time deferred work,
network delay or a lost reply can still cross the window or leave an uncertain outcome. This change
adds no schema migration, consumption record, OTP format change or live AuthService switch.
The private OTP unit/SQL tests and exact runner are recorded in [the testing guide](testing-guide.md);
dated results belong to [project status](project-status.md), not the original C1-E totals.

### OTP consumption policy and its earlier pure stage · 2026-10-04

The private [`otp_consumption` module](../../engine/src/services/auth_service/durable_source/otp_consumption.rs)
defines a proposed counter transition, not a database consumer by itself. Its first implementation
was deliberately unwired and left schema 1 unchanged; the later integration below now calls it from
login and rotation. The pure policy alone does not make a supplied watermark authoritative or establish
cross-process replay prevention.

`Watermark` is the highest consumed time-step counter, with all earlier counters also excluded.
The explicit value `-1` means none consumed; lower values are invalid state, not a recovery default.
Any larger signed 64-bit value is representable, including one ahead of the clock, which blocks
earlier codes rather than silently lowering stored state. Missing database state must not later be
mapped to `-1` without a reviewed initialization contract.

Given explicit time, bounded secret/code text and a watermark, `prepare` checks **all** matching
counters in the existing SHA-1/six-digit/30-second/±1 window. If any match is at or below the
watermark it rejects the code, even when the same six digits also match a newer counter. Otherwise
it proposes the highest match. This is monotonic counter consumption, not permanent uniqueness of
six-digit values. A fast authenticator may consume a future step and make the user wait for a later
counter. Wall-clock rollback does not lower state, but not every backwards time adjustment is rejected.

The pending proposal privately binds the decoded secret and trimmed code using a domain-separated,
length-framed digest. It exposes only the previous/next watermarks and a recheck; it has no public
constructor, Debug/serialization/Clone, or raw secret/code storage. Normalization-equivalent input
remains equivalent, but a different key cannot replace the original even if its code happens to match.
`recheck` reruns selection using the **original** watermark and requires the same next counter and
binding. A moved window cannot silently retarget a pending proposal to a new colliding counter.
Repeated pure calls consume nothing, and this digest is neither an account identity nor a credential,
database record, transaction receipt or guarantee at commit acknowledgement.

Persistence requires a separate integration: pin the exact account incarnation and complete credentials,
lock the source, compare-and-update the stored previous watermark to next, and verify the pending
watermark and business result after writes in the same transaction. Login Session issuance and
password rotation/revocation must commit with consumption; password changes must not reset it.
Versioning, old-writer fencing, existing-data adoption and uncertain-outcome recovery need an explicit
contract before activation; the schema-2 follow-up below supplies the fresh-install contract. Tests that reuse OTPs must be adapted without bypassing their
intended fault/lock barriers or turning an early replay rejection into a false rollback test.

The pure tests include fixed public test-key collision vectors and explicit time; no machine clock or
database is involved. Current execution evidence is in [project status](project-status.md).

### Unreleased atomic OTP consumption · 2026-10-04

Prepared login and password rotation now require **auth source schema 2**. Each exact account row
stores `otp_last_counter`, a non-null BIGINT initially `-1`, with a validated lower-bound constraint.
Fresh explicit installation adds `0015_durable_otp_consumption.sql` after the unchanged base templates;
registration and bootstrap verify the initial watermark before confirming enrollment. Existing schema
1 is rejected even when empty: no automatic adoption, repair, backfill or live-data migration is supplied.
Do not run the additive template manually against existing data, relabel metadata, or reset a watermark
to bypass rejection. Product/API version numbers are independent of this storage version.

After password work, both commands retain the original source pins, take the source writer fence,
recheck the exact incarnation and full credentials, and prepare against the **current stored** watermark.
Login compares and updates the old watermark before inserting its Session. Rotation updates credentials
and watermark together, then revokes all Sessions. Both verify the expected watermark and business
result after writes, and recheck the bound, exact chosen counter immediately before requesting COMMIT,
without another SQL wait. They return success/token only after confirmed commit. Password changes never
clear consumption; ordinary Session validation/logout do not consume another OTP.

The account row and transaction, not a process cache, serialize independent sources. Reusing a committed
step fails with `InvalidCredentials`, including login versus rotation. A successful password change
therefore needs a later OTP for re-login. The pure policy's collision rules still apply: any matching
counter at/below the watermark rejects; otherwise consume the highest match. Six-digit strings are not
forever unique. A future-step match closes older steps too; a fast authenticator can require waiting.
Watermarks belong to account incarnations, not usernames across deletion/recreation.

A known pre-commit failure rolls back consumption with its Session/password effects, allowing retry
only while the code remains eligible. Cancellation, timeout or a lost COMMIT reply is **not proof of
rollback**: a code may be consumed without delivering its Session token. Do not blindly retry, restore
old Sessions or reset counters; reconcile through trusted maintenance and use a later OTP. No credential
receipt, TOTP secret rotation/recovery or existing-data migration is implemented. Application UTC still
defines the OTP window; database time defines Session expiry. This is not freshness at acknowledgement
or rejection of every clock rollback.

Version 2 fences cooperating old prepared writers, not legacy AuthService or a privileged SQL owner.
Keep the legacy writer isolated; do not activate this source on its live tables. This follow-up does not
mount a public issuer or bypass timing-enumeration, ingress, migration and recovery gates. Nor does it
claim every private resource command performs full schema verification after every write.

SQL fixtures now use explicit synthetic Sessions where only rotation is under test, or later OTP steps
for genuine repeated login. Fault counters prove targeted triggers were reached; early replay rejection
must not masquerade as rollback coverage. Exact consumption, schema and freshness runners and dated
results are recorded in [testing](testing-guide.md) and [project status](project-status.md).

### Remaining composition and migration

C1-E established the durable incarnation needed by C1-D. Included in 0.4.6, [C1-F](collaboration-session-commands.md)
already composes current-session verification, binding, policy/audit and commit with one lock order.
Included in 0.4.7, [C1-G](collaboration-account-deletion.md) adds restricted deletion/retirement of
another active bound account. The same release includes internal staging for [C1-H password rotation](collaboration-password-change.md),
[C1-I empty-authority bootstrap](collaboration-bootstrap.md), [C1-J local provisioning](collaboration-provisioning.md)
and [C1-K read-only reconciliation](collaboration-reconciliation.md). General lifecycle/recovery and live
composition remain pending. Never commit auth changes and then best-effort write the registry.
Standalone C1-D retirement still does not revoke Sessions, and logout does not retire a Principal.

Real-data adoption separately requires source inventory, restore-tested backup, explicit migration
mapping, writer fencing, cutover approval and rollback/reconciliation boundaries. No deployed database,
live server, account or role was changed by this slice.

## Verification

[Source regressions](../../engine/tests/durable_auth_source_tdd.rs) and
[fault injection](../../engine/tests/durable_auth_source/faults.rs) provide 14 explicit PostgreSQL cases,
plus a default closed-pool test and a bounded/shared-admission unit test. Missing APIs, omitted CI
selection and incompatible-empty-schema adoption were observed failing before implementation/fixes.
Coverage includes concurrent installation/registration, authority mismatch, restart/recreation,
password/TOTP, hashed sessions, expiration/logout, immutable coordinates, wrong incarnations,
missing/unknown storage, suppressed/altered writes, deferred commit failure, pre-commit cancellation,
logout/read ordering and credential/generation changes during session insertion.

Local verification used PostgreSQL 16 with a private Unix socket and synthetic accounts: all 14 new
cases, 62 preceding collaboration cases and 22 legacy authentication cases passed (**98 database
tests**). The full default Rust suite passed **333 tests, with 156 ignored**; the 98 database cases were
run separately, while the other 58 ignored cases were not run in this slice. The new CI step's 14 exact
selection commands also passed locally. `quality-gate.sh --format-only` passed **76 common tests**,
formatting, file lengths, version/document sync, API/SDK contracts and tooling checks.
Browser, frontend-build, live-kernel/LAN, deployment/restore acceptance and live dependency audits were
not run for this slice. Tooling smoke checks are not actual kernel or artifact acceptance.

[CI](../../.github/workflows/ci.yml) lists and runs all 14 ignored cases by exact name; the
[CI regression](../../scripts/tests/postgresCi.test.mjs) rejects omissions or empty selectors. Use only a
disposable database via `CYANREX_TEST_DATABASE_URL`, never the deployed `DATABASE_URL`.

```bash
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test durable_auth_source_tdd -- --include-ignored --test-threads=4
```

The implementation slice was verified against 0.4.5 without a release or deployment. Its source is now
included in 0.4.6; see [release validation](../../reports/releases/0.4.6/README.md). Historical reports
and frozen API/SDK evidence remain unchanged. This source release does not cut over live authentication.
