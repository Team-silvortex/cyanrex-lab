# Current-Session file grant management

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased internal preparation**, source **0.5.6**, **2026-10-11**.
This working-tree follow-up is outside the published 0.5.6 tag; API baseline remains 0.3.0.
It adds explicit host-configured `ManageFileGrants` over the
[publisher ledger](large-file-publisher-grants.md), following
[proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2).
It does not make an Active record sufficient for publication, sharing or byte access.

## Trusted configuration, current actor

`DurableAuthSource::file_grant_management_workspace(publisher_workspace, manager)` creates a
`SessionFileGrantManagementWorkspace` without SQL, installation or policy mutation. Its private
fields freeze the existing namespace, scope, installation and retained-grant budget, plus one exact
`FileAccountRef` manager: Principal and account generation. Scope and manager authority must match
the source. It has no request codec, default manager, role lookup or first-user promotion.

The trusted host must construct this configuration independently of untrusted requests. Allowing
a requester to select `manager` and then constructing a handle would destroy the boundary.
This is **host-configured ManageFileGrants**, not a new persisted Registry permission. The existing
Registry still has only its existing membership/deployment policy; no schema or role is upgraded.

`issue_session_file_publisher_grant(token, management, grant_id, subject, validity)` derives issuer
from the current audited active Human/member Session and requires exact equality with the configured
manager. The request cannot supply issuer, actor or management policy. A role-free configured member
can manage; an unconfigured teacher, workspace owner or deployment manager cannot.

Issue checks the target's exact current source account, audited active Human identity and active
membership in the same workspace before and after writing. Equal usernames cannot inherit a prior
account generation. Validity must satisfy the pure contract, but issuance does not require its interval
to contain now: future or already-expired grants remain metadata, not a current permission decision.

`revoke_session_file_publisher_grant(token, management, grant_id, expected_revision)` derives the
revocation actor in the same way. It deliberately does not require the old subject to remain active,
a member or present in the source: retired/deleted subjects must still be revocable. The ledger keeps
its exact Active revision 1 → Revoked revision 2 CAS, original issuer/subject/validity and immutable
history. Repeated or stale transitions conflict; expired/revoked grants retain their quota slots.

## One source-owned transaction

Both commands borrow the ledger's move-only pending mutation helpers, not its standalone APIs.
The source alone owns transaction settings, the ten-second command deadline and COMMIT.

1. Set READ COMMITTED before source queries, retain configured READ ONLY, set synchronous_commit on.
2. Capture original source/Registry/target identities; authenticate current actor and exact manager.
3. For issue, verify the target through the original source pins, then return to the original target.
4. Take the ledger's ordered three-table writer fence, validate the bounded complete ledger and write
   exactly one expected head/history transition while retaining the original schema and ledger.
5. Flush `SET CONSTRAINTS ALL IMMEDIATE`, recheck the original schema and complete expected ledger;
   issue also rechecks target identity/membership through the original source pins and ledger again.
6. Restore synchronous_commit on, finish original source/Registry/current actor checks, then perform
   the last fresh database-time Session check. Only acknowledged source COMMIT releases the record.

The two-second lock and five-second statement limits remain. Borrowed helpers do not acquire pools,
start/commit transactions, change settings or flush deferred work. Subject validation is a generic
Principal/account private-work guard, not a file permission hidden in shared platform policy.
Original relation pins reject valid clones rather than adopting them after a trigger changes paths
or objects. Trigger/deferred failures roll back definite pre-commit changes; cancellation, deadlines
or lost acknowledgement do not generally prove rollback or authorize retry, repair or cleanup.

## Revocation and remaining boundaries

Logout stops that Session; a new login of the same configured account can manage again. Suspension,
retirement or membership removal stops commands, but restoring the same account/member can restore
the fixed configuration's effect. Merely replacing the configuration **does not revoke surviving
clones or in-flight commands**. To remove a configured manager, the trusted host must stop distributing
old handles, drain their work and destroy all old handles or restart. Online durable, audited manager
revocation requires a separate future permission ledger and fence, not a copied configuration snapshot.

Revoking a publisher grant does not revoke manager configuration, independent recipient grants,
delete published files or stop ongoing byte work. Managing grants does not grant publishing, download,
private Artifact access or deployment permission. Historical trusted-store records cannot be upgraded
into database-proven Session issuance merely because this new entry point exists; stored attribution
does not independently prove which API was called or what historical host configuration authorized it.

No actual publisher permission consumption, recipient ledger, byte IO, HTTP/SDK/UI, live mounting,
Session issuance, startup installation, migration, restart recovery or automatic retry is added.
The original own Session metadata reader and trusted standalone ledger remain separate APIs.

Later [Session publication](large-file-session-publication.md) independently consumes a current
publisher grant, rechecks original source/Registry/grant identities across lock-free file work and
shares the insert with final authorization. Managing grants still does not itself grant publication.
This later slice has separate execution evidence; the management record below is unchanged.

## Verification

Default `session_file_grant_management_tdd` cases check inert configuration and cheap admission.
The explicit `scripts/test-session-file-grant-management.sh` checks compiled inventory and runs
disposable PostgreSQL cases with `--ignored --exact`. Common `fileGrantManagement` guards check
source composition and bilingual documentation, not runtime authorization or deployment acceptance.
Execution evidence for this slice is recorded separately from the original ledger's dated results.

## Recorded execution 2026 10 11

Compiled Session Red: **0 passes / 1 expected failure**, with **zero Rust warnings**, at actual
issuance through the inert borrowed mutation seam. The fixture cleans up before asserting failure.
After implementation, the explicit PostgreSQL 16 runner passes **10 exact cases**, with zero warnings.
Each case is checked against the compiled inventory before execution. The three default configuration
and admission cases also pass in the full backend gate.

Controls prove role-free configured management, unconfigured teacher/deployment denial, exact manager
Principal/account generation, clone-policy persistence, automatic issuer/revocation attribution,
original issuer retention, CAS and current target/source/audited membership checks. A recreated manager
needs new explicit host configuration; retired/deleted subjects remain revocable. Commands succeed
with the independent grant installer pool closed, using only the source-owned transaction.

Immediate and deferred fault controls reach actual writes and compare full source/Registry/grant rows
and OIDs after rejection. They cover current actor/target/session changes, suppressed writes, foreign
ledger tampering, SQL/path failures and legal original source/Registry/target relation replacement.
Healthy explicit commands after removing each synthetic fault prove the fixture is recoverable;
this is not an implementation retry policy or a blanket audit of arbitrary trigger effects.

Real lock controls cover issue/revoke expiry after grant-marker waiting, fresh marker-incarnation
drift and cancellation during deferred flush after tentative writes. Independent post-cancellation
observation establishes rollback only in those controlled pre-commit experiments. An admitted command
blocks a real concurrent `source.logout` until commit; sequential logout-before-command also denies
later management. A separate session-row-holder deletion is explicit SQL injection, not the other
concurrent logout ordering. No arbitrary lost-COMMIT-acknowledgement experiment is claimed.

Concurrent queued callers are both polled after releasing a real barrier. Recursive backend blocking
chains prove two writers waited; configured RR/off-sync connections still get exactly one quota/CAS
winner through fresh command reads. READ ONLY is preserved, defaults restore afterward and revoked
slots remain occupied. The deferred sync probe observes `on` at flush, not power-loss durability.
The marker mutation holder pre-admits its table write lock to avoid a fixture lock-upgrade deadlock;
the fixed-authority foreign workspace is explicitly `ScopeConflict`, not `ScopeNotFound`.

Adjacent clean runners pass **12 ledger + 7 own-Session read + 10 private-file metadata cases**,
all with zero warnings: **39 exact database cases** including this slice. All use one newly created
private Unix-socket fixture with no TCP listener; no existing application database is consulted.

`quality-gate.sh --backend-only --no-npm-install`, with test/ambient database variables removed,
passes **841 default Rust tests** (including 3 compile-fail doctests), **257 common checks** and all
length/version/course/OpenAPI/API/SDK-generation and operator-tool checks. **771 opt-in cases remain
ignored**; zero Rust warnings. Frontend/SDK runtime, fresh dependency audits, the full opt-in inventory,
cross-platform execution, real LAN transfer and deployment acceptance are not rerun or claimed.
The 106 prior status evidence blocks, 16 prior detailed execution tails and dated capability tensor
remain unchanged. Version stays **0.5.6**, API baseline **0.3.0**; no commit, push, live installation
or migration is run. The disposable database is stopped and its generated data/socket removed
(about 92 MiB, recreatable by the tests); execution logs remain under
`/tmp/cyanrex-file-management-xHPacXWT/`.
