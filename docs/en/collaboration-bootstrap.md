# ADR-010: Atomic initialization of an empty authority

Status: **C1-I internal staging, included in 0.4.7**. Builds on
[C1-E](collaboration-auth-source.md), [C1-F](collaboration-session-commands.md),
[C1-G](collaboration-account-deletion.md) and [C1-H](collaboration-password-change.md).
One explicit trusted-operator call now creates the initial source account, legacy Workspace, Human
identity, owner/teacher membership, deployment grant and audit baselines in the same transaction.
This is not public enrollment, a live deployment change or migration of existing accounts.
The original 0.4.7 implementation used source schema 1. The OTP-consumption follow-up included in 0.5.1 now
initializes prepared source schema 2; the transaction description below reflects that contract.
The original verification record is retained separately.

## Scope and trust

[`bootstrap_empty_authority`](../../engine/src/services/auth_service/durable_source/bootstrap.rs)
accepts an operator-pinned Workspace/authority, typed username and new password. The authority must
match the injected source. The caller must be trusted local provisioning code with a pool pinned to
the intended database/schema, not a browser user, a discovered peer or whoever registers first.
There is no HTTP/CLI entry, startup hook, environment default or automatic invocation in this slice.

The selected connection must resolve exactly one non-system namespace, without temporary objects.
Bootstrap rejects any existing relations, functions or types there, including unrelated data,
legacy auth tables, an empty preinstalled source, partial registry installation and a prior successful
bootstrap. It does not repair, adopt, clear or retry over such state. This is stricter than the separate
staging installers: use a newly selected empty namespace, not their partially prepared output.
Uncooperative privileged SQL/DDL remains outside the adapter's trust boundary.

The fixed initial policy is active `cyanrex.workspace.owner` plus `cyanrex.teaching.teacher`, and a
separate explicit deployment grant for this authority. That grant comes from the operator's bootstrap
action, not an inference from the teacher role. It preserves the personal-use teacher/deployment
model while retaining the legacy teaching Workspace as the first collaboration mapping. Neither the
grant nor the owner role is global authority over other instances or other people's private content.
Subsequent registrations remain unbound and unprivileged until current audited commands bind/grant them.

## Transaction and publication

Included in 0.5.1: password derivation uses the
[shared prepared password-work gate](collaboration-auth-source.md) before acquiring database locks.
Admission waits consume the existing operation deadline; cancellation after dispatch does not free
the worker slot early. Derivation explicitly pins the existing Argon2id profile and account readback
validates it. This neither issues a Session nor changes installation or secret delivery.

The separate OTP-consumption follow-up adds fresh-install template 0015 without changing released
templates 0001/0010. New accounts start with `users.otp_last_counter = -1`, meaning no counter has
been consumed. Both ordinary registration and bootstrap verify that exact initial value; registration
also verifies source schema before and after insertion. Later prepared login and password rotation
atomically advance this watermark with their Session/password effects, rather than consuming a code
during enrollment. Password rotation must not reset it.

Prepared source schema 1 is rejected, even when empty, rather than upgraded or relabeled. Bootstrap
continues to reject every preinstalled namespace. Existing data needs a separately reviewed migration
and recovery contract. The version check fences older prepared writers that honor source metadata;
it cannot fence the legacy live AuthService, which does not use that metadata. Do not let that writer
share an activated prepared source.

Password length remains 8–4096 bytes. Argon2 and TOTP secret preparation occur outside database locks.
Inside the existing bounded transaction, the initializer:

1. Acquires the same schema-scoped advisory fences as the auth-source, identity and access installers,
   in that order, before DDL or checking absence. Concurrent bootstrap requests cannot both see an
   empty namespace and install independent first managers.
2. Checks namespace emptiness, creates auth tables from unchanged templates 0001/0010 plus fresh-only
   template 0015, pins source authority/version 2, takes the source writer lock and verifies source
   constraints, including the watermark's type, nullability, default and range-check expression.
3. Creates registry/access tables from unchanged templates 0006/0007; takes identity/access metadata
   writer locks before authority and child rows. Creates the pinned authority and active legacy Workspace.
4. Uses the shared pending registration writer to create a fresh random account incarnation with
   watermark `-1`. Creates its Human binding and canonical owner/teacher membership with deployment
   grant at revision 1.
5. Creates audit tables using unchanged templates 0008/0009, appends one identity baseline and one
   policy baseline, and activates identity/access schema 2. The external operator is not an authenticated
   Principal: baselines have no invented historical actor, command ID or replay receipt.
6. Rechecks the complete Workspace/identity/policy graph against its audit heads and current management
   authority, verifies exactly one initial graph, then rechecks source authority, credentials and account
   incarnation and initial watermark `-1`. Exactly one account and zero Sessions must exist. Only
   confirmed COMMIT publishes a result.

Registry helpers borrow the outer SQL transaction and return pending state; they never commit or
acquire source locks in reverse order. The registration refactor preserves ordinary registration's
no-grant semantics. The fresh-schema bootstrap is distinct from the existing general audit upgraders:
it does not weaken their preconditions or enable new-authority creation on an activated registry.

The result includes the committed account, Workspace, identity, policy and one-time TOTP enrollment
material, with no `Debug`/serialization implementation for the secret-bearing result. No Session is
issued. The user must complete normal password/TOTP login before using management commands; an
initialized database is not a preauthenticated browser. Initial audit activation also fences the old
unattributed registry writers, and the sole-manager protection remains in force.

## Failure, concurrency and recovery limits

Suppressed/tampered writes, audit failures, deferred commit failure and tested pre-commit cancellation
leave no partial account, grant, audit activation or DDL behind. The operation keeps the source's
ten-second total, two-second lock and five-second statement limits. A source/identity installer behind
bootstrap waits on its existing advisory fence; an access installer can fail closed on not-yet-visible
identity metadata. If a separate installer wins first, bootstrap rejects the resulting nonempty state.

Repeated calls, changed usernames/scopes and calls after deletion of the initial source account are
rejected rather than reseeding authority or returning secrets. This is not an emergency-manager path.
An error, cancellation or lost commit acknowledgement **does not prove rollback**. There is no durable
bootstrap command receipt or success replay. A baseline records state, not proof of who initialized it
or delivery of its TOTP secret. Reconcile the pinned database/schema through trusted maintenance;
do not drop tables, remove audit history, change IDs or silently create another manager to recover.
Tests deliberately retry only after confirming rollback of their own synthetic transaction.

## Verification and next gates

The following records the original C1-I verification, not a new schema-2 run. Current follow-up
evidence and its scope are recorded in [Project Status](project-status.md).

The [suite](../../engine/tests/durable_bootstrap_tdd.rs),
[fault tests](../../engine/tests/durable_bootstrap/faults.rs) and
[concurrency tests](../../engine/tests/durable_bootstrap/concurrency.rs) add one default closed-pool
case and 14 explicit PostgreSQL cases. Missing API compilation and an absent CI selector were observed
before implementation. All 15 cases passed locally against isolated PostgreSQL 16 on 2026-10-02.
CI checks each ignored test exists, then executes it by exact name; the
[selector regression](../../scripts/tests/postgresCi.test.mjs) rejects omissions.

Because production bootstrap rejects preinstalled tables, fault injection uses schema-filtered DDL
event triggers in a **disposable test database with event-trigger privileges**. A sequence counter proves
the intended row trigger actually fired even when its transaction rolls back; an unrelated earlier DDL
error cannot pass those fault tests. Test hooks are not part of application code or production migrations.
PostgreSQL's event-hook catalog is database-global even when callbacks filter schemas: whole test cases
hold a dedicated database advisory lock while installing/using/removing hooks. This fixes an observed
parallel teardown/cache race without serializing the competing bootstrap/install operations inside a case.

```bash
CYANREX_TEST_DATABASE_URL='<disposable PostgreSQL URL>' CARGO_BUILD_JOBS=2 \
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_bootstrap_tdd \
  -- --include-ignored --test-threads=2
```

The same development run passed 337 default Rust tests (219 opt-in cases ignored), all 161 explicitly
selected PostgreSQL authentication/C1 cases, and the format/common quality gate including 80 script
regressions, frozen-contract/version/course checks and tooling tests. The ten CI step bodies ran locally
against the disposable database; no remote CI run was started. Build/runtime/database artifacts were
isolated, with debug info disabled and incremental compilation off to limit disk use. There was no
frontend production build, live migration, deployment or privileged kernel acceptance.

[C1-J](collaboration-provisioning.md) subsequently adds the controlled local interface and private
secret delivery. Recovery, durable lifecycle reconciliation, reviewed migration/restore,
public-route/CSRF integration and live cutover remain separate gates.
The original C1-I slice did not change frozen API/SDK contracts, current runtime configuration, old
migrations or the crate release version; its results do not establish release-candidate, remote CI,
deployment or kernel acceptance. The source-schema-2 contract included in 0.5.1 does not authorize an
existing-data migration or live authentication cutover.
