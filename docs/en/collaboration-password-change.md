# ADR-009: Atomic self-service password change and session revocation

Status: **C1-H internal staging, included in 0.4.7**, originally based on 0.4.6 commit
`45e0abcca25c3e795ad78882a293d060b12c208e`. Builds on the explicit
[authentication source](collaboration-auth-source.md), [session commands](collaboration-session-commands.md)
and [restricted deletion](collaboration-account-deletion.md). The original C1-H slice did not change
live AuthService, public API/SDK, schema templates, existing accounts or deployment.

## Supported operation

[`change_session_password`](../../engine/src/services/auth_service/durable_source/credentials.rs)
accepts a Session token, current password, new password and TOTP. The exact account is derived from
the current durable Session; the caller cannot select a username, target incarnation or actor.
The source must already be explicitly installed. This is self-service credential rotation, not
a manager reset, enrollment, recovery or migration primitive.

A successful return confirms both the new password and deletion of **all** Sessions for that exact
account, including the calling Session and expired Sessions. No replacement token is issued; the
caller must log in again (with a later OTP under the schema-2 follow-up). Username, account incarnation and TOTP secret are preserved, as are other
accounts and their Sessions. Rotating to the same password still changes the salt and logs out devices.

This source-level operation does not require or create a registry binding or grant. Unbound accounts,
ordinary members and the sole manager may rotate their own credentials. A registry-retired account
whose source account still exists may also rotate, but its Principal stays retired and collaboration
commands still deny it. Authentication is not restored authorization. No Membership, deployment grant,
ownership, policy revision or identity/policy audit history is rewritten or appended by this operation.
There is no credential-change audit ledger in this slice.

## Transaction and authorization boundaries

Included in 0.5.1: both password verification and replacement hashing now use the
[shared prepared password-work gate](collaboration-auth-source.md). Its four dispatched/twenty total
limits span other source instances and prepared entry points, outside SQL locks and within the same
ten-second operation budget. A separate prepared PHC policy validates account snapshots before
admission and pins replacement hashing to the existing Argon2id profile; unsupported records fail
with `InvalidRecord`, not a credential rewrite. The schema-2 OTP follow-up consumes a counter together
with credential replacement and revocation, never resetting the watermark. It rechecks the bound exact
counter after the final database-time query, before requesting commit; it does not promise freshness
at acknowledgement. Existing schema 1 is rejected unchanged, not migrated. Original release evidence
below is historical; current verification is in [project status](project-status.md).

1. Under a short source `FOR SHARE` transaction, resolve the fresh unexpired Session and its exact
   credential snapshot, then commit that read transaction.
2. Share the existing bounded login admission budget; verify the current password/TOTP and derive
   a fresh Argon2 hash outside database locks and connections. New passwords are 8–4096 bytes;
   current password and OTP inputs retain the source's 4096/64-byte ceilings.
3. Start a new transaction with source `FOR UPDATE` **before** child rows. Recheck the exact Session,
   incarnation, complete old credentials and current OTP watermark after the lock wait, preserving
   the original namespace/relation pins across both transactions. Never upgrade a held source
   shared lock to a writer lock.
4. Count every matching Session; update exactly one account using the verified incarnation and old
   credentials and watermark as predicates; replace the password and advance consumption together.
   Recheck the authorizing Session after the UPDATE, before intentional
   revocation, then delete exactly the counted Sessions.
5. Read back the new credentials, preserved incarnation/TOTP, expected watermark and absence of Sessions. Since the
   calling Session is now intentionally deleted, compare its verified expiry with fresh database time
   after the final write/trigger waits. The follow-up included in 0.5.1 then rechecks OTP without another SQL
   wait before requesting COMMIT. Confirm COMMIT before returning success or clearing admission.

The source writer fence serializes login, logout, C1-F commands and C1-G deletion. Registry locks are
not needed for a source-only change; no reverse registry-to-source lock order is introduced. A login
committed first is revoked; one still verifying old credentials must recheck them before issuing a
token. Validation behind the change observes no old Session. Logout first invalidates the change;
change first leaves logout idempotent. Concurrent changes cannot both consume the old credentials.
Binding admitted first may complete before rotation; binding behind it cannot use the revoked token.
Administrative deletion can follow rotation, but rotation cannot act after deletion or on a replacement.

The admission budget is shared by clones and login within this source instance: five admitted attempts
per username per five-minute window, bounded to 1024 names. It is not a durable cross-process throttle.
Existing ten-second operation, two-second lock and five-second statement limits remain in effect.

## Failure and recovery limits

Suppressed writes, changed credentials/incarnation, altered or reinserted Sessions, post-wait expiry,
deferred commit failure and pre-commit cancellation must not produce a successful result. No memory
fallback, DDL, bootstrap or automatic retry is attempted. Secrets remain outside result DTOs and logs.

Cancellation, timeout and lost commit acknowledgement **do not prove rollback**. There is no command
ID, durable change receipt or success replay: the authorizing token is deliberately unusable after
success. Reauthenticate and reconcile current source state through trusted maintenance when the
outcome is uncertain; do not restore old Sessions, overwrite a later password, or infer which operation
committed from a historical identity receipt. A late COMMIT can cross expiry or lose its response;
the final freshness check is not continuous authorization through an arbitrarily delayed commit.

Privileged SQL/DDL and the legacy live AuthService do not obey this source fence. Never activate the
staging source over live AuthService tables. The live password endpoint retains its existing behavior;
this ADR does not claim it now revokes every device.

## Verification and remaining gates

The [suite](../../engine/tests/durable_password_change_tdd.rs),
[fault tests](../../engine/tests/durable_password_change/faults.rs),
[concurrency tests](../../engine/tests/durable_password_change/concurrency.rs) and
[registry boundaries](../../engine/tests/durable_password_change/registry.rs) add one default
closed-pool case and 17 explicit PostgreSQL cases. Missing-API compilation and a missing CI selector
were observed before their implementations. All 18 cases passed locally against an isolated PostgreSQL
16 instance on 2026-10-02. CI verifies each ignored case exists, then runs it by exact name; the
[selector regression](../../scripts/tests/postgresCi.test.mjs) rejects omissions.

```bash
CYANREX_TEST_DATABASE_URL='<disposable PostgreSQL URL>' CARGO_BUILD_JOBS=2 \
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_password_change_tdd \
  -- --include-ignored --test-threads=2
```

The same local run also passed 336 default Rust tests (205 opt-in tests ignored), all 147 explicitly
selected PostgreSQL authentication/C1 cases, and the format/common quality gate including 79 script
regressions, contract/version/course synchronization and tooling checks. The nine authentication/C1
CI step bodies were executed locally, including C1-G's 16 database cases; this is not a claim that a
remote CI run was triggered. Frozen API/SDK baselines remain unchanged. Disposable database, runtime
and Cargo artifacts were isolated; debug info was disabled and incremental compilation stayed off
for this disk-limited verification. No frontend production build, real kernel or live deployment ran.

Unified bootstrap, password reset/TOTP recovery, credential audit/reconciliation, public routes with
CSRF and distributed admission, reviewed migration/restore and live cutover remain separate gates.
This is not release-candidate, deployment or kernel acceptance. Historical ADR verification records
remain unchanged; later regression results belong to this slice.
