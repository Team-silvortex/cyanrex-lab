# Persistent file publisher grant metadata

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased internal preparation**, source **0.5.6**, **2026-10-11**.
This working-tree follow-up is outside the published 0.5.6 tag; the public API baseline remains 0.3.0.
This follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) persists
publisher-grant metadata and an exact issue/revoke history. A separate current-Session reader returns
only the subject's own record. Neither a stored Active grant nor a successful metadata read authorizes
publication, sharing or byte access. Recipient grants and transfer services remain separate.

## Explicit ledger and history

`FilePublisherGrantStore::new(pool, scope, installation_id, namespace, max_grants)` freezes trusted
configuration without connecting or installing storage. The namespace must already be selected by
its pool. `install_empty_namespace` is an explicit fresh-only operation using a separate schema 1
marker, head table and change table; it does not migrate or adopt existing or partially installed data.
Workspace, installation UUID, original namespace/table OIDs, exact ordinary table shapes and the
configured limit remain checked throughout each operation. RLS, temporary/shadow relations,
inheritance and changed search paths are rejected.

`issue(grant_id, issuer, subject, validity)` creates revision 1, Active state and the first change
record together. `issuer` and `subject` are exact `FileAccountRef` values, including account generation;
both must belong to the configured authority. They are **trusted caller attribution**, not credentials
or verified Registry-management authority. An issuer's teacher, workspace-owner or deployment role
does not manufacture a publisher permission or authenticate this trusted API.

`revoke(grant_id, actor, expected_revision)` admits only the original Active revision 1 and creates
Revoked revision 2 plus a second change. It retains the first issuer, subject and validity interval,
and records the supplied exact actor/account. There is no reactivation, validity edit, subject change,
command replay, implicit retry or cleanup. Every duplicate ID or revision/state mismatch conflicts.
History uses canonical `grant_payload` bytes and checked integer database `recorded_at` Unix seconds;
timestamps do not prove an authenticated issuer, monotonic clock or trusted historical permission.

The installed `max_grants` is 1–256 and applies to all retained heads, including expired/revoked
records and older account generations. At most two changes belong to each grant. Writes serialize
in one table order, use READ COMMITTED after waits, validate the bounded whole ledger, flush deferred
work, and compare the original schema and exact expected head/history before requesting COMMIT.
Configured READ ONLY policy is not disabled. Only acknowledged commit returns the stored result;
loss of acknowledgement may leave the change committed and grants no right to repeat it.

## Current Session own metadata

`DurableAuthSource::file_publisher_grant_workspace(namespace, scope, installation_id, max_grants)`
constructs a separate `SessionFilePublisherGrantWorkspace`. It installs nothing and changes no live
authentication. `get_session_file_publisher_grant(token, workspace, grant_id)` derives the subject
Principal/account from an audited current active Human membership, not from a supplied client ID.
The authentication and grant namespaces must be separate.

The reader borrows one source-owned READ COMMITTED transaction, retains original source/Registry
and target pins, validates the selected head and complete history, and rechecks current policy and
fresh Session expiry before acknowledged COMMIT. The selected read fences grant-table writes,
including history appends, in the common schema/head/change order until that transaction ends.
Marker, selected head and history rows also retain FOR SHARE locks. This coarse fence can block
other grant writers; the existing ten-second command, two-second lock and five-second statement
limits still apply. Independent store getters retain their separate READ ONLY, REPEATABLE READ path.

Missing and another subject/account's records return authorized `None`; selected own corruption
fails closed. The read does not audit unrelated grants. A recreated account cannot inherit old records;
an active role-free member can read its own metadata, but a teacher or deployment manager cannot read
another subject's record. Expired and Revoked records remain visible as own metadata. Returned values
are observations, not capabilities: callers must not cache Active state as permission for a later action.

## Gates before publication authority

The Registry still has no persisted `ManageFileGrants` permission. A later independent
[Session management adapter](large-file-grant-management.md) now uses explicit trusted-host manager
configuration, never deployment management or teaching roles. Its new evidence does not alter this
ledger/read slice's recorded execution below. Publication must separately use a current audited
publisher decision in its protected command.
The later [Session publication adapter](large-file-session-publication.md) now supplies that separate
consumer, retaining original grant/source/Registry pins across file work and final fenced source commit.
It does not upgrade the trusted ledger's historical attribution or rewrite its recorded evidence.
Recipient management and download admission must additionally bind the exact trusted Published version
and publisher account, with their own revocation and in-flight byte-work boundaries.

The later independent [recipient ledger](large-file-recipient-grants.md) supplies trusted persistence
and own-recipient reads, not that management or download consumer. Publisher revocation does not
cascade into its retained grants; this publisher ledger and recorded execution remain unchanged.

This ledger reads no files, upload stores, publications or recipients. It adds no mounted HTTP route,
Session issuer, OpenAPI/SDK/browser API, startup installer, migration, file recovery or network transfer.
The existing [owner-private file metadata](large-file-session-metadata.md) and
[pure grant contracts](large-file-access-contract.md) keep their independent responsibilities.

## Verification

`file_publisher_grant_tdd` covers trusted storage; `session_file_publisher_grant_tdd` covers the
separate current-Session metadata reader. Their default tests check inert configuration and cheap
admission against closed pools. `scripts/test-file-publisher-grants.sh` and
`scripts/test-session-file-publisher-grants.sh` inventory and explicitly run real disposable PostgreSQL
cases with `--ignored --exact`. Common `filePublisherGrant` checks inspect source composition,
inventory and bilingual documentation; they are not SQL authorization, transfer or deployment tests.

## Recorded execution 2026 10 11

Compiled ledger Red: **0 passes / 1 expected failure** at the explicit fresh installer, with
**5 temporary scaffold/fixture warnings**. After ledger baseline Green, the independently compiled
Session Red produced **0 passes / 1 expected failure** at the own metadata read, also with **5
temporary warnings**. Initial ledger/Session baseline Greens had 3/1 remaining warnings respectively;
the final implementations remove those unused scaffold imports/method warnings.

The explicit PostgreSQL 16 ledger runner passes **12 exact cases**; the Session runner passes **7**.
Each runner checks the selected compiled inventory before executing a case. Full ordinary-table rows
and OIDs are compared around reads and failures, and successful changes retain exact prior history.
Ledger controls cover CAS/issuer immutability, retained cross-generation quotas, independent RR
writer blocking chains, corrupt/orphan/excess inventory, suppressed writes, deferred exact-delta
tampering, valid-clone original-relation substitution, path/marker drift and temporary-shadow rejection.
Cancellation is observed during deferred `SET CONSTRAINTS ALL IMMEDIATE` work before COMMIT;
it is not a lost-acknowledgement experiment. The sync probe observes deferred flush, not power-loss durability.

Session controls cover own expired/revoked/future records, role-free membership and teacher isolation,
old account generations, membership/identity/session lifecycle, scope/install/shape refusal, a closed
independent installer pool, configured RR/READ ONLY preservation, real marker/head/history row waits
through database-clock expiry and cancellation, and a real append backend blocked by the read's
table SHARE fence. That append experiment starts at a history-row wait, not the final-auth query.
This target adds no source/Registry clone experiment, full outer-timeout test or both concurrent logout
orderings; adjacent private-file regressions retain their separate original-relation coverage.

The first combined runs exposed fixture assumptions: the second writer waits through the first,
foreign workspace is ScopeConflict, and a held row cannot safely upgrade its table lock behind the
store fence. Corrected controls track both actual writer chains, retain the exact error, and pre-admit
the marker writer lock; production admission is unchanged by those fixture repairs.

The adjacent `test-session-file-metadata.sh` first passed nine cases, then exposed an older fixture's
sequential await of two queued callers: if the unpolled caller held the authority lock, its peer
timed out. Polling both with `tokio::join!` preserves the exact winner/quota result and all snapshot
checks. The repaired exact case passes independently, followed by a clean **10-case** full runner,
including original source/Registry relation replacement. The new 19-case runs and final adjacent
runner have zero Rust warnings. All SQL execution uses a newly created private Unix-socket fixture;
no existing application database is consulted or changed.

`quality-gate.sh --backend-only --no-npm-install`, with test/ambient database variables removed,
passes **838 default Rust tests** (including 3 compile-fail doctests), **253 common checks** and all
length/version/course/OpenAPI/API/SDK-generation and operator-tool checks, with **761 opt-in cases
ignored** and zero Rust warnings. The new targets contribute **6 default cases**. Frontend runtime,
SDK runtime, fresh npm audits, the complete opt-in inventory, cross-platform execution, real LAN
transfer and deployment acceptance are not rerun or claimed. The 106 prior project-status evidence
blocks, 14 pre-existing detailed execution tails and dated capability tensor remain unchanged.
Version remains **0.5.6** with API baseline **0.3.0**; no commit, push, live installer or migration is run.
The private synthetic database is stopped and its generated data removed; execution logs are retained.
