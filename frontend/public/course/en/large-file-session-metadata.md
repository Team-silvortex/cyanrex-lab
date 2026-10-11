# Current Session private file metadata

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased Unix preparation**, source **0.5.5**, **2026-10-11**.
This follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) authorizes
creation of an owner's upload reservation and exact reads of that owner's upload or Published
metadata. It does not authorize actual publication, byte access or distribution to recipients.

## Explicit configuration and commands

`DurableAuthSource::file_upload_workspace(namespace, scope, installation_id, limits)` returns a
`SessionFileUploadWorkspace`. `file_publication_workspace(namespace, scope, installation_id,
max_publications)` returns a separate `SessionFilePublicationWorkspace`. These freeze trusted
configuration, validate it without SQL or filesystem IO, and install nothing. Targets must already
have their explicitly installed schema and match the exact workspace, installation and limits.
The source authentication/identity namespace must differ from the selected metadata namespace.

| Command | Returned metadata | Authority excluded |
|---|---|---|
| `reserve_session_file_upload(token, workspace, upload_id, manifest)` | One immutable logical upload reservation | No supplied owner/account, bytes, physical disk allocation or publishing permission |
| `get_session_file_upload(token, workspace, upload_id)` | Exact own reservation or `None` | No complete-ledger health claim, verified chunk progress or resumed attempt |
| `get_session_file_publication(token, workspace, upload_id)` | Exact own `FileUploadPublicationRecord` or `None` | No publisher call, historical receipt, current byte health, recovery or download grant |

The server derives Principal and account generation from the current Session and its audited
identity binding. It requires an active workspace, active Human Principal and active membership,
not a teaching role or deployment grant. An active member with no role refs may manage its own
private intent. Agent/service identities, suspended or retired bindings and a recreated account
cannot inherit an older incarnation's records. A foreign owner's record is absent, not disclosed.

## One source owned transaction

All SQL runs on the authentication source's pool in one borrowed transaction, including metadata
access. A store handle does not acquire another pool or commit the source transaction. The command
sets READ COMMITTED before the guard's first snapshot, regardless of a source pool's default
REPEATABLE READ setting. Configured READ ONLY policy is preserved and fails closed: even metadata
reads need locking authentication/registry checks and are not PostgreSQL read-only transactions.

The guard pins the original source namespace and source/registry relation identities, resolves the
current audited owner and selects a separately pinned target. Upload reservation keeps the fixed
writer fence, checks the entire bounded ledger, enforces byte/count quotas across all account
generations and checks permanent upload/version conflicts before quota. It inserts exactly one
row, flushes deferred constraints and verifies the exact expected record, full ledger and original
target schema. It then reselects the source, rechecks original source/registry relations and current
policy, and checks Session expiry using fresh database time after waits. Metadata escapes only
after COMMIT acknowledgement. Synchronous commit is restored before the deferred flush and commit.

Selected reads retain the original schema and exact row fingerprint without auditing unrelated
rows. The schema marker and a present row use FOR SHARE through the final authority checks and COMMIT, preventing a
concurrent update from changing that selected observation during a wait. `None` is an authorized
selected absence, not a lock against a later insertion. Corrupt selected own rows fail closed;
foreign hidden rows are not decoded merely to answer a private read. The old trusted store getters
remain independent READ ONLY, REPEATABLE READ snapshots with caller-supplied trusted ownership.

The transaction retains the existing ten-second deadline, two-second lock timeout and five-second
statement timeout. Cancellation or commit acknowledgement loss never proves rollback. In particular,
an upload may already exist after an unavailable result; a metadata read does not recover the
original command receipt or authorize adoption, automatic retry, overwrite or quota release.
Database administrators remain trusted; relation checks do not identify an entire database cluster.

## Permission and transport gates

The private-work guard is not a file publisher or recipient policy. Teacher authority remains
instance deployment authority, not permission to publish another account's file or read its private
metadata. The [trusted publication pipeline](large-file-publication.md) stays separate and is not
called by these commands. No staged files, chunk journals, attempts or receipts are opened here.

Reviewed publishing permission, exact-version recipient grants, physical disk admission and
receipt-loss/restart reconciliation remain required. An unprivileged centralized transport and
later authenticated desktop pairing are still absent. No HTTP route, startup installer, live
AuthService switch, OpenAPI/SDK/browser connection or migration is added by this adapter.

## Tests

Default `session_file_metadata_tdd` cases use lazy unavailable pools to check configuration and
cheap token/scope admission without a database. `scripts/test-session-file-metadata.sh` explicitly
inventories each ignored case, then runs it with `--ignored --exact` against disposable PostgreSQL.
The SQL cases exercise owner/account derivation, positive Published reads without file IO, invalid
Sessions/policy, account recreation, missing/corrupt records, quotas, configured defaults, trigger
faults, original relation replacement, real waits and cancellation. `sessionFileMetadata` common
guards check source composition and inventory only; they do not substitute for SQL behavior.

## Recorded execution

On **2026-10-11**, the source **0.5.5 Unreleased** slice passed the three default target cases and
all ten exact PostgreSQL cases on a separately initialized PostgreSQL 16 Unix-socket fixture. The
default target ignored those ten SQL cases. The initial reservation test failed against the inert
adapter, then passed after implementation. A real INSERT-trigger table clone separately reproduced
the discarded original source-relation pin; retaining that initial pin made the rollback test pass.
The positive Published read also caught and corrected a marker-table spelling error.

The same fixture passed **72 adjacent SQL cases**: 12 upload-intent, 13 trusted publication, 17
Session Task, 18 Session Artifact and 12 trusted/Session content-index cases. These establish the
selected storage/authorization regressions, not deployment, network, frontend, SDK, disk-full,
power-loss or large-file throughput acceptance. Generic lost-COMMIT-acknowledgement outcomes and
a specially staged final-auth wait after acquiring a selected read lock were not injected here.

The separate backend-only quality gate passed **813 default Rust tests** and **245 common script
checks**, with zero failures or compiler warnings. Its **742 ignored cases** were not counted as
passes. Source 0.5.5, public API baseline 0.3.0 and historical maturity evidence remained unchanged.
