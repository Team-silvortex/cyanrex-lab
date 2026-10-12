# Persistent recipient grants

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased metadata preparation**, source **0.5.6**, API **0.3.0**, **2026-10-11**.
This separate ledger follows [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2)
and the [exact grant contracts](large-file-access-contract.md). It records a trusted host's recipient
grants and supports current-Session own-recipient metadata reads. It does not authorize downloads,
authenticate grant managers or connect transport. It is not mounted in the application.

## Exact file and independent accounts

`FileRecipientGrantStore` freezes an explicit PostgreSQL pool, workspace, RFC 4122 version-4
installation ID, restricted namespace and quota in `1..=MAX_FILE_RECIPIENT_GRANTS` (256).
Construction is inert. `install_empty_namespace()` explicitly installs schema 1 in a fresh namespace;
it does not adopt existing relations, migrate data or activate live authentication.
Only the exact bare or canonical quoted target is admitted in the raw search path; extra entries
are rejected even when nonexistent. Original raw spelling remains pinned within each transaction.

`issue(id, issuer, file, publisher, recipient, validity)` records three independently supplied
`FileAccountRef` values. Each contains a principal and exact account generation. Their authority must
match the configured workspace. Issuer need not equal publisher; self-recipient grants are valid.
Trusted attribution is not a proof of an audited identity, current publisher permission or publication.

The immutable `FileVersionRef` contains workspace/authority, file ID, version ID, byte length and
SHA-256. It can come from valid claimed manifest metadata without reading bytes or checking a
Published record. Future download admission must separately compare it with the exact trusted
publication and publisher account. Equal digest alone is insufficient.

Grant IDs are permanently unique within this ledger. Different grants may name the same file,
publisher and recipient; there is no implicit deduplication or version/recipient uniqueness rule.
The separate [publisher ledger](large-file-publisher-grants.md) may use the same ID independently.
Revoking publisher permission does not revoke recipient grants; each recipient grant requires its
own explicit revocation. Teacher, deployment and ownership roles create neither kind of grant.

## Persistence and revocation

The explicit `0022_collaboration_file_recipient_grants.sql` layout has three permanent ordinary tables:

- `collaboration_file_recipient_grant_schema` pins schema version, scope, installation, quota and
  namespace/marker/head/history OIDs.
- `collaboration_file_recipient_grants` stores ID, recipient/publisher/issuer generations, revision,
  validity and canonical `grant_payload BYTEA` with a 4096-byte ceiling. The complete file reference
  lives in the payload, not duplicated file columns.
- `collaboration_file_recipient_grant_changes` stores ordered immutable payloads, actor generations
  and bounded database-clock `recorded_at` Unix seconds. Revision 1's actor must equal the issuer.

Decoding checks strict canonical payloads against all duplicated head/history columns and the entire
selected history. File, publisher, recipient, issuer and validity cannot change. A grant starts Active
at revision 1; `revoke(id, actor, expected_revision)` permits only the CAS transition to Revoked at
revision 2, preserving issuer and recording the separate revoking actor. There is no reactivation,
renewal, update, deletion, adoption or retry operation.

All retained grants occupy quota, including expired, revoked and old-generation records. Writers
validate the bounded whole ledger before checking duplicate IDs or quota. Duplicate conflicts precede
quota errors. Whole-ledger validation also rejects orphan, extra or corrupt history.

Trusted writers select READ COMMITTED before reading, preserve configured READ ONLY policy and
acquire an ordered three-table SHARE ROW EXCLUSIVE fence. Original namespace/relation shapes,
installation, complete expected records and ledger are rechecked after `SET CONSTRAINTS ALL IMMEDIATE`.
They restore synchronous commit and return `StoredFileRecipientGrant` only after COMMIT acknowledgement.
Cancellation, timeout or a lost acknowledgement is not proof that nothing committed.

## Current Session own metadata

`DurableAuthSource::file_recipient_grant_workspace(...)` freezes an inert
`SessionFileRecipientGrantWorkspace`. `get_session_file_recipient_grant(token, workspace, id)` derives
the recipient from current audited active Human identity, workspace membership and account generation.
It does not accept a request-supplied recipient and does not require a teacher or grant-management role.

One source-owned transaction uses READ COMMITTED and a ten-second outer bound. The borrowed reader
retains original namespace/schema pins and full selected-history fingerprint. Its three-table SHARE
fence precedes row/marker `FOR SHARE` locks and remains held through final fresh Session/Registry/policy
checks and source COMMIT acknowledgement. The borrowed helper opens no independent connection,
transaction or commit and does not override READ ONLY policy.

Missing, foreign-recipient and old-generation grants return authorized `None`. Foreign payloads are
filtered before decoding. Selected corruption is rejected. Own future, expired and revoked records
remain readable metadata; Active state is not download permission and the read does not enforce the
grant's access window. Direct trusted `get` remains a selected READ ONLY repeatable-read metadata query,
not a whole-ledger health check or a current-Session decision.

`SessionFileRecipientGrantError` distinguishes Session, Grant and InvalidNamespace errors. No
unused permission-management error or role conversion is introduced in this read-only slice.

## Tests and remaining delivery gates

`file_recipient_grant_tdd` covers the trusted ledger. `session_file_recipient_grant_tdd` covers current
own-recipient reads. `scripts/test-file-recipient-grants.sh` and
`scripts/test-session-file-recipient-grants.sh` require an explicit disposable database and select
each ignored case exactly. `fileRecipientGrant` common guards check inventory and composition only;
they do not establish SQL locking, authentication or byte delivery behavior.

Recipient issue/revoke management under a current authenticated manager remains separate. Download
admission must check the exact trusted Published version, current recipient, active grant and fresh
time under appropriate retained fences; actual bytes and client delivery require their own lifecycle.
No download command, byte IO, HTTP, OpenAPI/SDK/UI, transport, startup installer, deployment, pairing,
recovery or live-auth cutover is added. Historical execution records and dated maturity scores remain
unchanged; this follow-up does not upgrade previous metadata observations into download authority.

The later October 12 [publisher-owned sharing](large-file-sharing.md) adapter supplies independent
current-Session issuance and original-publisher revocation over this ledger. Issuance compares an own
exact trusted Published version under current publisher permission; revocation needs neither continued
publisher permission nor recipient existence. It adds no download consumer or global manager delegation
and does not change this ledger/read slice's original execution record below.

Later [download-manifest observation](large-file-download-manifest.md) consumes the selected original
Active grant for one current metadata result: exact granted publisher/Published version, full original
history and final fresh grant/Session validity. It does not relax this old metadata reader, access files
or make either returned value a reusable byte permit; the execution record below is unchanged.

## Recorded execution · 2026-10-11

Both exact baselines compiled and failed on real missing behavior before implementation: fresh
ledger installation, then current role-free recipient reading, each **0 passed / 1 failed** after
fixture cleanup. Preliminary scaffold syntax and invalid empty-host fixture URL failures were corrected
before those runs; neither was counted as behavioral Red.

An additional compiled raw-path regression returned `Ok(None)` for an extra confirmed-nonexistent
search-path entry, instead of `UnsupportedSchema`: **0 passed / 1 failed** after cleanup. Restricting
only this new ledger's raw target to bare or canonical quoted form made it pass. Original per-transaction
identity checks, old publisher policy and shared namespace mechanics were not relaxed or changed.

The first full ledger matrix passed its baseline, then failed its independence fixture because its
namespace prefix plus UUID had 64 characters. The fixture prefix was shortened; production's 63-byte
limit stayed unchanged. Assertion panics reclaim the exact fixture before preserving the failure.
The complete ledger rerun and first complete Session matrix passed without warnings.

Final explicit PostgreSQL 16 execution used a fresh private Unix socket instance with no TCP listener
and no ambient application database. Each runner checked compiled exact ignored inventory before running:

| Runner | Passed SQL cases |
| --- | ---: |
| `test-file-recipient-grants.sh` | 12 |
| `test-session-file-recipient-grants.sh` | 7 |
| `test-file-publisher-grants.sh` | 12 |
| `test-session-file-publisher-grants.sh` | 7 |
| `test-session-file-grant-management.sh` | 10 |
| `test-session-file-metadata.sh` | 10 |
| `test-session-file-publication.sh` | 10 |
| `test-file-upload-publication.sh` | 13 |

These **81 explicitly executed SQL cases** had zero failures and warnings. New ledger defaults separately
passed 3 cases with 12 ignored; new Session defaults passed 3 with 7 ignored. The complete backend quality
gate then passed **850 default Rust cases**, with **800 ignored cases separately accounted for**, across
81 result groups including three compile-fail doc tests. All **266 common guards** passed; compiler warnings
were zero. Ignored inventory was not counted as executed SQL. No frontend logic or public API changed.

Two independent source reviews found no additional substantive defect. Actual pre-COMMIT deferred
cancellation plus independent drained rollback observation is not a general lost-ACK simulation. The
sync probe checks the deferred-flush setting, not fsync, power loss or acknowledgement-time durability.
Row waits and history-append blocking use actual backend blockers; elapsed time alone is not evidence.

All 106 historical status sections, 20 pre-existing detailed execution tails and the dated machine
capability-tensor SHA-256 were preserved. After execution, no private test namespaces or other database
clients remained. The fresh PostgreSQL instance was stopped and its reproducible data/socket removed
(about 181 MiB); verification logs were retained. No application database, version, commit or push changed.
