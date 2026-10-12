# Current Session authorized file publication

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API baseline **0.3.0**, **2026-10-11**.
This separate adapter consumes a current publisher grant to publish a real, already staged upload.
It follows [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), the
[publisher ledger](large-file-publisher-grants.md), [Session grant management](large-file-grant-management.md)
and [trusted publication](large-file-publication.md). Central transport, recipients and restart recovery
remain separate; the adapter is not mounted in the application.

## Trusted configuration and opaque input

`DurableAuthSource::file_publishing_workspace(publisher, grants)` freezes a
`SessionFilePublishingWorkspace`. The existing `FileUploadPublisher` and
`SessionFilePublisherGrantWorkspace` must share the exact workspace/authority and use four distinct,
non-system namespaces. Construction performs no connection, file IO, installation, migration or role
conversion. The host configures the publisher, private root and ledger independently of requests.

`publish_session_file_upload(token, workspace, grant_id, attempt)` accepts an existing opaque
`FileUploadPublicationAttempt` from the trusted publisher's metadata-only `prepare` method. That
preparation still accepts trusted attribution and is not itself Session authentication. The command
derives its actor from current audited active Human membership and requires the attempt's exact owner
and account generation. Client owner claims, teacher/deployment roles and management configuration
cannot substitute for an active publisher grant. The old trusted publisher remains separately callable
by trusted code; this adapter does not silently gate that API.

## Permission and publication sequence

1. Cheap token/source checks precede issuer/control admission. Rejection for wrong issuer, non-Ready
   input or exhausted control capacity leaves the supplied state unchanged; eligible Ready input
   remains Ready when admission fails. Admission sets Unknown before the first await and retains the
   existing shared control slot through authorization, file work and final checks.
2. One READ COMMITTED source transaction checks the exact current actor and selected own grant,
   including complete original history, active state and fresh database-time validity. Original
   source/Registry/grant identities and exact Session survive in opaque memory. This commit confirms
   only admission metadata, not a publication or detached permission.
3. After releasing the connection and every SQL fence, existing workers check the complete confirmed
   journal and fresh real ordered chunk/whole-file bytes. The same original file observation survives
   for final checks. No source SQL transaction spans file work.
4. A fresh source transaction rechecks the first source/Registry identities and original exact Session,
   reacquires the original grant's three-table SHARE fence and rejects changed history or replacement
   installations. Original upload/journal metadata receive SHARE fences. The create-only publication
   writer uses its original installation and full bounded expected ledger.
5. The caller restores `synchronous_commit = 'on'`, flushes `SET CONSTRAINTS ALL IMMEDIATE` and
   rechecks publication/upload/journal/grant records and original pins. Final current actor/Session
   checks precede one fresh clock sample covering BOTH grant validity and Session expiry. No SQL
   follows before requesting COMMIT; configured READ ONLY policy is never disabled.
6. Only that source COMMIT acknowledgement can mint the private publication acknowledgement.
   Independent original upload/journal/publication and same-file checks still follow. Only their
   success returns a receipt and sets Confirmed; a stored Published row alone cannot.

The first grant-time sample is an admission observation, not continuous permission for byte IO;
expiry during later admission checks or file work must still reject the final publication.
Early metadata is not reused as a permission boolean. Cooperating revocation writers serialize against
the final grant fence. One fresh final sample checks expiry after all preceding SQL waits, rather
than using transaction-start time.
This is permission at the final pre-commit observation, not at acknowledgement or every instant of
dispatched file work. Cancellation does not kill queued/running jobs; worker slots remain job-owned.
Each source SQL phase has its own ten-second outer bound; this is not a whole-file workflow deadline
or a guarantee that a cancelled file job has stopped.

Borrowed upload/journal/publication checks accept only the exact bare or canonical quoted spelling of
the same original restricted namespace, with the original OID, relations, shapes and installation.
Extra raw search-path entries remain invalid, even if nonexistent. Direct trusted APIs and post-commit
acknowledgement checks retain their original strict raw search-path identity checks.

## Failure boundaries

`SessionFilePublishingError` separates Session, Grant, Publisher, InvalidNamespace and PermissionDenied.
Missing/foreign/inactive/time-ineligible grants do not authorize byte work; changed original history
can instead yield a typed integrity error. Admitted errors or cancellation leave Unknown, with no
automatic retry, restoration, adoption, cleanup or conversion from matching metadata.

The insert shares the source transaction with permission and final actor checks. Definite pre-commit
rejection can roll back that transaction, but timeout or lost acknowledgement is not rollback proof.
An acknowledged row can remain when later file/metadata checks fail. A grant is not deleted or spent;
it may authorize separate eligible uploads until expiry/revocation. Permanent upload/version uniqueness
and retained upload/publication quotas remain unchanged.

Original observations detect metadata/object drift, not WORM sealing or same-UID/root/database-admin
isolation. Publication checks bytes without assembling, copying, renaming, extracting, executing or
delivering them. The existing 64 KiB buffer, bounded manifest and job-lifetime budget remain; no physical
disk reservation, throughput/RSS or power-loss guarantee is added.

## Tests and remaining delivery gates

`session_file_publication_tdd` contains closed-pool configuration controls and opt-in real PostgreSQL/file
cases. `scripts/test-session-file-publication.sh` checks exact ignored inventory before execution;
`fileSessionPublication` common guards check composition, not actual database or file behavior.
Adjacent trusted publication, private metadata and grant-management tests retain independent scopes.

No recipient ledger/download command, authenticated chunk-upload transport, HTTP, OpenAPI/SDK/UI,
public Session issuer, startup installation, live-auth cutover, deployment or desktop pairing is added.
Reviewed uncertainty/restart reconciliation, physical disk policy and safe client delivery remain gates
before usable centralized distribution. Dated capability-tensor evidence is unchanged.

The later independent [recipient ledger](large-file-recipient-grants.md) now supplies exact-file
metadata persistence and own-recipient reads. It adds no recipient management or download consumer,
does not cascade publisher revocation and does not change this publication slice's execution below.

The October 12 [publisher-owned sharing](large-file-sharing.md) follow-up now consumes an own exact
Published metadata record for current recipient issuance. It does not recreate this publication's
original byte receipt, read files, enable downloads or change the recorded execution below.

## Recorded execution · 2026-10-11

The initial exact baseline compiled and failed at actual publication (`Publisher(Unconfirmed)`),
not at a missing symbol. Integration then exposed the original bare search-path pin versus the
source transaction's canonical quoted selection: a legitimate same-namespace publication was
rejected. The borrowed-only spelling bridge fixed that connection while preserving original
physical identities and strict direct/post-acknowledgement checks. Its exact baseline passed.

The first matrix passed six SQL cases before the grant-expiry wait collided with the existing
two-second lock timeout. The fixture window alone changed from database epoch +3 to +2 seconds;
the single-case rerun and complete final matrix passed, still requiring an observed real lock wait,
actual clock expiry, exact PermissionDenied and unchanged retained records/files. Production timeouts
and integrity assertions were not weakened. The staging source guard now allows only two named
borrowed transaction-type imports in the phase module, still rejecting raw SQL, pools and commits.

Final exact runs used a fresh private PostgreSQL 16 Unix-socket fixture, with no TCP listener or
ambient application database. Every runner checked its compiled ignored inventory before each case:

| Runner | Passed SQL cases |
| --- | ---: |
| `test-session-file-publication.sh` | 10 |
| `test-file-upload-publication.sh` | 13 |
| `test-file-upload-whole.sh` | 8 |
| `test-file-upload-chunk-journal.sh` | 11 |
| `test-session-file-metadata.sh` | 10 |
| `test-file-publisher-grants.sh` | 12 |
| `test-session-file-publisher-grants.sh` | 7 |
| `test-session-file-grant-management.sh` | 10 |

All **81** passed, with no failed cases or compiler warnings. The separate backend quality gate,
with application/test database variables unset, passed **844** default Rust cases (including three
new closed-pool controls), **261** common checks and zero warnings; **781** opt-in cases stayed
ignored there. Those ignored cases are not counted as executed by that gate.

The new cases cover real cross-chunk/empty bytes, exact grant/actor generations, revoked history
across the released SQL/file gap, immediate/deferred mutations, original relation replacement,
final-wait expiry, cancellation, queued RR quota and unchanged READ ONLY defaults. Snapshots compare
ordinary retained rows/relation OIDs and staged root/files' identity, mode, links, hashes, mtime and
ctime, not atime, trigger/sequence/all-catalogue state or the fixture's original input file.

Cancellation coverage is a real deferred pre-commit advisory wait followed by independently
observed rollback, not general lost-ACK reconciliation. Post-acknowledgement coverage first observes
the Published row independently, then a real original-source lock wait before replacing same-byte
inode/root objects or changing source/journal/publication records; all five retain Unknown and no
receipt. Logout is tested before the command, not both concurrent logout orderings. Setting probes
do not establish fsync/power-loss safety. No frontend/SDK/browser, transport, recipient, recovery,
LAN deployment, live authentication or kernel acceptance was performed in this slice.
The synthetic PostgreSQL instance was stopped; only its recreatable data/socket directories were
removed (about 159 MiB). Run and failure logs were retained; no application data was removed.
