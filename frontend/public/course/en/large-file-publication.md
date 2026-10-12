# Persistent whole file publication metadata

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased trusted Unix preparation**, source **0.5.5**, **2026-10-11**.
This follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) combines an
exact complete confirmed chunk journal with a fresh check of the real ordered file bytes, then
inserts one immutable publication record. It is not a connected distribution service, a download
grant, recovered execution or a guarantee that the files will remain intact.

## Explicit composition

`FileUploadPublicationStore::new` freezes trusted PostgreSQL configuration. Its
`install_empty_namespace` explicitly installs a separate fresh namespace; construction, preparation
and normal application startup never install or migrate it. The existing upload and chunk-journal
schemas stay unchanged. `0020_collaboration_file_publications.sql` is not an automatic migration.

`FileUploadPublisher::new(local, journal, publications)` requires the same workspace and three
distinct namespaces. It reuses the local adapter's control admission and the original staging worker
budget. `prepare(upload_id, owner, account_id)` captures the original source reservation, complete
confirmed journal target and publication schema identity. Preparation reads no files and writes no
publication rows. Supplied trusted ownership is not current Session authentication or permission.

`FileUploadPublicationAttempt` is opaque and one-shot. Only its issuing publisher or a clone accepts
it. It cannot be cloned, serialized, restored or constructed from a metadata read. A control-capacity
rejection preserves Ready; after admission it becomes Unknown before the first await. Confirmed is
set only when the original attempt receives the final checked receipt.

## Required bytes and original identities

The journal check selects the complete target upload without filtering away a foreign owner or
account. It verifies bounded index order, count, chunk geometry, exact owner/account generation,
source/root bindings, manifest digests, Confirmed status and original row nonces. Valid missing or
Unknown chunks yield NotReady; corrupt, extra or mixed-binding rows are rejected. Empty files require
a healthy original journal identity and zero target chunk rows. Status alone is never byte proof.

The publisher checks original source and journal pins, reads the real private `u1` chunks in order,
checks every chunk and the whole-file digest, and rechecks the original source, journal and file
snapshots before requesting the publication insert. After its commit acknowledgement it again
checks the original source, journal, publication row/schema and the same original file observation.
It does not replace that observation with a fresh pathname or matching metadata. No await follows
the final file check before the receipt is returned.

File work retains the existing **64 KiB** byte buffer and a constant number of open chunk descriptors.
At most **16,384** bounded names and snapshots survive the scan. There is no new assembled file,
copy, write, chmod, fsync, rename, deletion or automatic cleanup in publication. Existing `u1` names
still hash the full manifest for each chunk; the quadratic naming work is not a throughput claim.
Queued/running jobs retain the original worker permit after their waiter is cancelled.

Fresh original-pin rechecks use READ COMMITTED, READ ONLY transactions. No SQL lock spans file work,
and the separate stores and filesystem observations are not one atomic snapshot. Same-UID/root and
database administrators remain trusted; these checks are not WORM sealing or a power-loss test.
Source/root/journal fingerprints do not identify an entire database cluster or authenticate a peer.

## Create only publication store

The store persists the full canonical bounded manifest once per publication, exact owner/account
generation and internal source/root/journal fingerprints and a unique UUIDv4 nonce. Public
`FileUploadPublicationRecord` exposes only upload, owner, account and manifest metadata, not those
internal binding fingerprints or an executable acknowledgement.

One fixed namespace has a trusted capacity of **1–256** publications, including empty files and all
owners/account generations. Both the upload ID and the `(file_id, version_id)` pair are permanently
unique there. Changing ownership, source or root cannot adopt an existing coordinate. Conflict is
checked before capacity. Existing logical upload reservations remain occupied; publication does not
duplicate their byte quotas or reserve physical disk space.

The writer uses READ COMMITTED and fixed table/marker locks, validates the whole bounded ledger,
inserts exactly one row, flushes deferred constraints, and rechecks the exact expected ledger and
original schema before requesting synchronous commit. It first reads at most 257 scalar heads,
then decodes one bounded manifest at a time, rather than accumulating 256 full manifests.
`FileUploadPublicationStoreError` distinguishes configuration/schema/record rejection, Conflict,
CapacityExhausted and StorageUnavailable. Configured read-only policy is not disabled.

There is no persistent publication Unknown row or later confirmation update. Every stored row is
immutable published protocol metadata. `get(upload_id, owner, account_id)` is a trusted exact
read-only snapshot, not a complete ledger audit, current file-health check, historical original-byte
receipt, restored attempt, retry permission or recipient grant.

## Failure and cancellation

| Situation | Stored publication | Original attempt |
|---|---|---|
| No control admission | Unchanged | Ready |
| Admitted error or cancellation before an insert | May be absent; an uncertain result is not rollback proof | Unknown |
| Commit succeeded but acknowledgement or final checks failed | May already contain the immutable published row | Unknown |
| Acknowledged insert and every final original check succeeded | Published metadata | Confirmed with an opaque receipt |

`FileUploadPublicationError` separates NotVisible, NotReady, capacity, source/journal/publication
errors, unavailable bytes and Unconfirmed. It never upgrades an uncertain attempt by querying
matching metadata. A fresh preparation is a new verification, not a retry or restoration of the old
attempt; if no insert occurred, no persistent publication coordinate has been burned. An existing
published row is never adopted or overwritten. Old reservations, journal rows and files are retained.

## Tests and remaining distribution gates

`file_upload_publication_tdd` covers default no-database admission and explicit disposable
PostgreSQL/file cases. `scripts/test-file-upload-publication.sh` lists and matches every exact ignored
case before execution. `fileUploadPublication` common checks guard composition and inventory,
not actual bytes, SQL waits or commit acknowledgements.

The independent [whole-byte observation](large-file-whole-verification.md) remains read-only and
journal-independent. Application startup, AppState, current-Session commands, HTTP, OpenAPI/SDK,
browser saving, deployment and desktop pairing are unchanged. Reviewed receipt-loss/restart
reconciliation, physical disk policy, current publisher/recipient authorization, central transport
and safe client delivery remain required before centralized distribution is usable.

Later [Session publication](large-file-session-publication.md) independently joins the real publisher
phases to current actor/grant checks and one final authorized source insert. No SQL lock spans file
work; the trusted API remains separate and both retain the same original post-ACK checks.
This later evidence neither rewrites the record below nor enables recipients, transport or recovery.

## Recorded execution · 2026-10-11

Two compiled baselines actually ran against a private disposable PostgreSQL 16 database. The
fresh installer first produced **0 passes / 1 expected behavior failure** (`StorageUnavailable`),
then passed the same exact assertion. A real 17-byte file with an already Confirmed journal first
produced **0 passes / 1 expected behavior failure** (`Unconfirmed`), then returned the expected
publication receipt and stored metadata. Neither Red was a missing-symbol compile failure.
Temporary unused warnings were **7 library** on installer Red, **8 library / 3 fixture** on installer
Green and file Red, and **3 fixture** on file Green before the remaining cases were mounted.
Final default and exact runs had no compiler warnings.

All **3 default no-database cases** and **13 exact PostgreSQL/file cases** passed. The runner listed
and matched every ignored name before execution. Real files cover 17 bytes, **4 MiB + 17 bytes** and
empty content. Coverage includes exact original source/root/journal binding, missing/Unknown versus
corrupt or extra journal rows, current bytes and whole digests, unsafe entries, upload/version
uniqueness, empty-file/global capacity, independent writers, configured read-only pools, schema
identity/shadowing, corrupt foreign manifests and suppressed/immediate/deferred insert faults.

The post-acknowledgement cases independently observe the committed publication **before** changing
source or journal identity, the original inode/root, or the publication row/schema. Connection-return
barriers and `pg_blocking_pids` establish real waits, rather than treating a deferred-commit wait as
an acknowledgement. Fault expectations are captured inside the injecting transaction **before COMMIT**.
Rejected operations match those exact expected snapshots of the three stores' markers/rows, six
relation OIDs, and chunk-root entries plus explicitly retained originals (bytes, identity, modes,
mtime/ctime, not atime). These snapshots do not cover the entire filesystem and are not an audit
of every auxiliary object in each namespace. A committed Published row can remain while the original
caller stays Unknown; a new attempt conflicts instead of adopting it.

The actual pre-commit cancellation case drains the connection and observes rollback for that case;
it is not general proof that cancellation or a lost acknowledgement rolled back. Independent capacity
writers use repeatable-read/synchronous-commit-off defaults. A triggered probe observes synchronous
commit restored before deferred constraint work, not independently at the final COMMIT or after power
loss. Configured read-only policy and pool defaults remain intact. During review, a TEMP-shadow fixture
was corrected to use fresh sessions: dropping TEMP tables does not erase `pg_my_temp_schema()` identity.
The production rejection was retained; this fixture issue was caught before its SQL execution.

Adjacent exact runners passed **8 whole-byte / 11 chunk-journal / 9 upload-staging / 12 upload-intent
cases**. The isolated backend gate, without any database URL, passed **810 default Rust cases /
241 common checks**, **73 Rust result blocks**, with **732 opt-in cases ignored**, zero failures
and no compiler warnings. Its three private whole-filesystem units also passed. Ignored cases are
not SQL acceptance. An in-progress focused inventory check initially had **20 passes / 1 failure**
before the new test modules were wired; the final focused **21 checks** and full gate passed.
Bilingual sources and generated course copies are synchronized. Current next-step descriptions now
link the implemented preparation layers; all **104 prior dated project blocks**, older slice execution
records and the capability-tensor digest remain unchanged. Source **0.5.5** and API baseline **0.3.0** stay fixed.

This run did not accept real lost-COMMIT-acknowledgement/restart recovery, physical disk-full/fsync/
power-loss behavior, maximum-size throughput/RSS, current-Session or HTTP authority, network transfer,
browser/SDK delivery, desktop pairing or LAN deployment. Earlier index SQL and frontend/SDK build
gates were not rerun. No version upgrade, commit, push, release or deployment was performed.

The socket-only private database had no synthetic namespaces or other clients remaining and is now
stopped. Its approximately **82 MiB** regenerable data directory and empty private fixture directories
were removed; execution logs remain under `/tmp/cyanrex-publication-4aucNvaV` (about **0.4 MiB**).
Existing project data and build caches were not removed.
