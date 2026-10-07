---
description: Schema adoption and authentication boundaries for the durable auth source
applyTo: 'engine/src/services/auth_service/durable_source/**/*.rs'
---

## Learnings

An empty table and a successful `SELECT ... LIMIT 0` do not establish schema compatibility: before
explicit empty-source installation, verify column types/nullability and primary/foreign key targets,
including cascading deletion, rather than silently adopting or repairing incompatible tables.
Preserve the missing-session-primary-key and malformed-empty-schema regressions in
[the source integration tests](../../engine/tests/durable_auth_source_tdd.rs).
The account incarnation must come from committed registration, never username-based backfill;
a returned session snapshot is not authority to execute a later identity or policy command.

Prepared source schema 2 is fresh-install only: apply new template 0015 without rewriting released
0010, and reject schema 1 even when empty. Verify the OTP watermark's int8/non-null/default/CHECK
expression contract, not just a constraint name. Registration verifies schema before and after its
write; registration and bootstrap must read back initial watermark -1. Never initialize a missing or
corrupt consumption record by falling back to -1. Version gates reject older prepared writers, not
legacy AuthService writers that ignore source metadata; keep those writers isolated.

Source-only `login`, `validate_session`, `logout` and password rotation pin one private namespace and
the three actual auth relations without requiring a collaboration registry. Retain the original
namespace/table OIDs and path across login and rotation password-worker gaps; do not recapture a
replacement as the accepted source.
After writes, verify the current path before unqualified readback, never reset it to hide drift.
Recheck relation/schema/authority and use fresh Session time after guard waits. An empty redirected
table cannot confirm logout. Preserve `durable_session_boundary_tdd` and its exact CI runner; these
guards neither repair storage nor make the trusted primitives a public Session issuer.

Explicit expired-Session maintenance uses the source writer fence and original namespace/relation pins,
then one fresh database cutoff. Keep the fixed 128-candidate limit and deterministic expiry/digest order;
bound text in SQL projection without filtering out corruption. Validate every selected full row and
exact-account existence before any deletion; fetch no credentials or OTP state. Compare all returned
fields including created_at, verify original pins before absence reads, and check selected digests without
an expiry predicate so future reinsertion also fails. Empty batches still verify and confirm COMMIT.
Preserve exact SQL fault/cancellation regressions and runner selection. Do not add implicit read/login
cleanup, caller-selected tokens/limits, orphan repair or blind retry after an uncertain outcome. A batch
count is not a global emptiness/storage bound or a full audit of malicious trigger effects elsewhere.
The cleanup projection must guard expires_at and created_at in SQL before chrono decoding, on both
SELECT and DELETE RETURNING. SQLx PostgreSQL 0.8.6 binary timestamp decoding can panic on infinity or
finite values beyond chrono's upper range; try_get().map_err(...) cannot catch that panic. Retain CASE
checks for isfinite and the bound chrono MAX_UTC parameter, decode Option timestamps, and reject NULL
as InvalidRecord. Do not filter corrupt candidates away or clamp their timestamps into valid values.
Preserve infinite and out-of-range finite timestamp regressions; this is a maintenance-path guard,
not a driver-wide fix or dependency upgrade.

Apply the same SQL-before-decode range policy to the shared Session expires_at reader, including
validation, login readback and guards that call it. Invalid values must remain InvalidRecord, not be
filtered into absence. Reconciliation keeps Session timestamp range/expiry checks entirely in SQL and
fetches only Option<bool>; NULL is InvalidSource, never a raw expiry disclosure or an expired default.
Retain finite 4713 BC/chrono-maximum boundary cases and the original snapshot expiry cutoff.
Those source Session checks are distinct from the registry timestamp guards in
[collaboration-store.instructions.md](collaboration-store.instructions.md): nullable retired_at needs a
separate validity bit and mandatory audit times reject NULL projections before publication. Preserve
reconciliation's InvalidIdentity / IdentityHistoryMismatch / PolicyHistoryMismatch mappings and the
maximum-time bind in every shared projection path, including LIMIT 0. Neither guard establishes safe
decoding of unrelated timestamp fields or authorizes repairing stored data.

Every prepared password hash/verification uses the process-local shared `password_work` gate, including
registration, login, rotation and bootstrap. Keep admission before secret copies and SQL locks out
of worker waits. Both total and worker permits belong to the blocking job after dispatch, including
Tokio queue time; caller cancellation/timeout must not release them early. Preserve deterministic
`password_work_tests` and five-call-site wiring checks. Do not silently widen this gate to legacy
AuthService, treat worker failure as wrong credentials, or describe job counts as a PHC cost bound.

Prepared PHC records require `password_profile` validation on account reads and before worker admission.
Keep the explicit Argon2id v19/m19456/t2/p1/32-byte writer and verifier profile in sync; no legacy fallback
or automatic profile adoption. Bound text before parsing, reject duplicate/missing/extra parameters,
and decode salt into a fixed buffer. Do not pass unchecked PHC values to Argon2 Params: parser lookups
and conversion disagree on duplicates, and extreme parallelism can overflow before library validation.
Invalid records are not wrong passwords; cost policy must not implicitly revoke existing Sessions.

Prepared login must retain the optional account snapshot through original-pin verification and the
first confirmed COMMIT, releasing the database before password work. Only a genuine absent account
selects the fixed public synthetic PHC in the supported profile; schema/authority/storage errors and
malformed real credentials must still fail, never become dummy work. Both paths use one shared-gate
verification, followed by an independent requirement for the original real account. A synthetic match
or concurrent same-name registration cannot authorize a second transaction, OTP consumption or Session
issuance. Keep input/username admission first and preserve cancellation-held permits. This removes the
missing-account KDF shortcut, not every timing difference or the remaining public-issuer gates.
If the supported profile changes, update the synthetic PHC together with derive/verify and preserve
unit checks for its profile, matching public test password and rejected mismatches; never hash a dummy
record afresh per request or use its verification result as account authority.

Prepared login and password rotation recheck TOTP after password work, behind the writer fence, and
after the last SQL read/time check immediately before requesting COMMIT. Do not move an awaited query
after the final OTP check or treat an earlier success as permanent proof. OTP uses application UTC,
Session expiry uses fresh database time; the per-source test clock exists only under `cfg(test)`.
Keep deterministic SQL wait/rollback regressions. Freshness alone is not single-use consumption, protection
against every wall-clock rollback, or a guarantee at commit acknowledgement; consumption requires the
same source-owned transaction described below.

The `otp_consumption` module remains pure, but prepared login and rotation now use it with durable
account watermarks. Collect all matching counters; any match at/below the supplied watermark rejects,
otherwise choose the highest.
Recheck the same credential binding and chosen counter against the original watermark. Preserve real
collision tests and authentication-wiring guards. Under the source writer fence, compare-and-update
the exact account/credentials and previous watermark to the proposed next value in the same transaction
as Session issuance or password change/revocation. Recheck source identity, exact watermark and all
business effects after writes; the final proof recheck follows the last SQL wait and precedes COMMIT.
Never reset the watermark during password changes or consume OTP in a separate best-effort write.
Monotonic counters do not make six-digit strings permanently unique. Unknown commit outcomes do not
prove rollback or authorize automatic retry/reset. Preserve fault barriers and trigger-hit evidence
when adapting tests that formerly reused one OTP; early replay rejection is not a rollback regression.

Session-authorized commands hold one transaction from source verification through registry mutation,
audit and confirmed commit: acquire source metadata, registry metadata, authority, then child rows;
recheck the exact session with fresh database time after waits and before committing.
Preserve the logout-order and expiration regressions in
[the composition fault tests](../../engine/tests/durable_collaboration/faults.rs), not a separate
`validate_session` call followed by an unrelated command transaction.
When revoking management, a remaining registry grant counts only if its audited Human identity still
matches a current durable source account incarnation; deleted or same-name recreated accounts cannot
serve as the last-manager safety net, as covered by
[the composition integration tests](../../engine/tests/durable_collaboration_tdd.rs).

Restricted account deletion takes source `FOR UPDATE` before registry locks (never upgrade from
`FOR SHARE`), admits only another active bound incarnation, and rechecks the surviving manager and
Session after deletion. An identity retirement receipt is not durable deletion evidence: reject
already-retired targets and receipt replay before source writes. Preserve both login/logout orderings,
replacement-account protection and rollback tests in `durable_account_deletion_tdd`.

Self-service password rotation derives its target from the current Session and requires the current
password plus TOTP, not a manager grant. Release snapshot locks before Argon2; reacquire source
`FOR UPDATE` and recheck the exact Session, incarnation and complete credentials before writing.
Credential replacement, OTP consumption and all-session revocation must commit together. After
deliberately deleting the calling Session, check its verified expiry against fresh database time after the final writes;
do not skip freshness or expect the deleted row to remain. Preserve identity/TOTP/grants, count and
read back revocation, and retain the fault/concurrency/registry tests in `durable_password_change_tdd`.
A failed acknowledgement is not proof of rollback, and no credential-change replay is available.

Private Task/Artifact commands share the `private_work` guard: current audited Human membership
permits only the caller's own resources, without a teaching/deployment grant. Keep target/source
namespace pins and return neither records nor content bytes before the final Session check and commit.
Borrowed Artifact reads must lock the head before related queries under the source's READ COMMITTED
transaction; the standalone reader's unlocked lookup depends on its own REPEATABLE READ snapshot.
File publication is not rolled back with PostgreSQL. Cancellation can leave a blocking writer running;
never delete, adopt or automatically retry an unpublished file based only on a failed acknowledgment.

Task-input composition pins every namespace before writes and verifies the departing and revisited
name/OID instead of recapturing a replacement. Lock the Task snapshot before resolving its saved
Artifact references, with stable Artifact-head ordering. Recheck all exact owned content after
Task/outbox writes and reread the pending Task: row locks cannot prevent a trigger's same-transaction
side effects. Retained input bytes still require final source/membership/Session checks and commit.
Preserve `session_task_inputs_tdd` namespace-substitution and post-write Session-deletion regressions.

Catalogue Task admission snapshots only the trusted catalogue's immutable definitions, never retains
or executes a provider during authorization. Compare the complete saved definition with its exact
configured entry; the same reference cannot hide changed policy, evidence schema or display metadata.
Keep manual Task entry points strict. Zero-input catalogue tasks still pin and validate Artifact
namespace/schema metadata before and after writes; an empty loop is not a storage check. Preserve
`session_catalog_task_tdd` provider-drop, same-pin drift and zero-input metadata-wait regressions.

Draft input replacement must verify the full old/new union in one global Artifact lock order,
including removed references, before and after the Task/outbox write. Do not deduplicate matching
coordinates with different digests. Keep exact revision and no-op checks, a distinct replacement
event, the Task schema-version gate and final pending Task/Session checks. Publication is separate:
replacement failure never authorizes file deletion or blind retry. Preserve `session_task_revision_tdd`
same-coordinate digest, removed-content corruption, schema-version and post-write tampering regressions.

Private human Review commands derive the reviewer from the Session and pin policy in trusted server
configuration. They verify both targets and evidence as exact owned content, in one globally sorted
Artifact-head order, not two independently sorted lists. Borrowed Review history reads lock the current
head before checking the requested revision under READ COMMITTED. Recheck every reference after
Review/outbox writes and compare the pending Review before the final guard/commit. A stored `approved`
opinion is not Task acceptance, cross-user review authority or proof of historical Session provenance.
Preserve `session_review_tdd` evidence-only corruption, exact-history wait and Session-deletion tests.

Fresh-authority bootstrap is trusted provisioning, never public registration or first-login election.
Before DDL, take the existing source/identity/access installer advisory fences in that order and reject
any nonempty or ambiguous namespace. Use source -> registry metadata -> authority -> child lock order,
commit account/identity/initial grant/audit activation together with initial OTP watermark -1, and
publish no Session or secret before confirmed commit. A repeated bootstrap or deleted first account
must not trigger regrant/recovery.
Keep the bootstrap fault injection counters: an early unrelated DDL error is not the intended row fault.

Maintenance reconciliation is deliberately separate from locking authorization: use one bounded
REPEATABLE READ, READ ONLY transaction, verify table/column shapes before payload reads, reject RLS
and inheritance, and inspect full audit chains with strict row/metadata budgets. Fetch no credentials
or token digests. The reader's users-column grants now include otp_last_counter solely for an SQL
range-validity boolean; neither fetch nor report its value, and never treat a snapshot as a consumption
receipt or permission to reset state. Retired source accounts and unbound registrations can be valid,
but account IDs cannot be reused under another name. Do not infer rollback, delivery, quiescence or retry authority
from a read snapshot. Preserve read-only-role, corruption, MVCC and cancellation regressions.
