# File publisher and recipient grant contracts

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Unreleased pure shared contracts**. Date: **2026-10-11**. Source: **0.5.5**.
These values define exact file-permission candidates for [centralized distribution](large-file-distribution.md).
They do not issue, persist or authenticate grants. A successful `matches` result is a comparison of
supplied metadata, not permission to publish, share or download.

## Exact bindings

| Value | Required binding |
|---|---|
| `FileAccountRef` | `PrincipalRef` and exact `LegacyAccountId` account generation, not a username |
| `FileVersionRef` | Authority/workspace, file ID, immutable version ID, byte length and SHA-256 |
| `FilePublisherGrant` | Contract schema, non-nil `FileGrantId`, positive revision, workspace, subject account, validity and state |
| `FileRecipientGrant` | Contract schema, non-nil `FileGrantId`, positive revision, exact file, publisher account, recipient account, validity and state |

All account authorities must equal the selected workspace authority. The version projection comes
from a supplied `FileTransferManifest` without IO; it neither proves publication nor retains an
original storage incarnation. Display names and paths do not determine permissions. Equal digests
on different files, versions, workspaces or account generations do not match.

`FileGrantValidity` requires integer Unix seconds with
`0 <= not_before < expires_at <= 253402300799`. There is no nullable or permanent lifetime.
The interval is `[not_before, expires_at)`; its first second matches and its expiry second does not.
An out-of-range supplied clock does not match. This represents time, not a trusted clock source or
an enforced grant-duration limit.

## Independent operations

`FilePublisherGrant::matches(workspace, subject, now)` requires the exact subject and workspace,
active state and a current validity interval. `FileRecipientGrant::matches(file, publisher, recipient,
now)` additionally requires all exact version fields and both account generations.

Publisher permission governs candidate publication and new-sharing operations. Recipient permission
is an independent candidate for reading an already selected version. Revoking a publisher grant
does not implicitly revoke existing recipient grants; stopping those shares requires their own
explicit revocation. An explicit self-recipient is valid, but ownership does not manufacture a grant.
Teacher, workspace-owner and deployment roles supply neither permission automatically.

Both selectors return only a boolean. Grant IDs, revisions and active state are supplied data;
they are not issuer attribution, audit provenance, current policy or a bearer capability. In particular,
a copied old active record can still match after the real record is revoked. Callers must not cache
that result as authority or accept a client's grant JSON as an authenticated permission.

## Strict metadata decoding

Both grant types provide `parse_json` with a **4096-byte** input ceiling. Every nested record must
be an object, including principal and workspace references. Missing, null, duplicate, unknown and
positional-array fields are rejected. IDs and digests retain their canonical scalar encoding;
schema, revisions, lengths and times require exact supported integers. Version length stays within
the manifest ceiling, including the empty-file digest rule. State accepts only the strings `active`
and `revoked`, not Serde's externally tagged object aliases such as `{"active":null}`.

Generic Serde decoding retains structural validation but does not impose the original JSON byte
ceiling; callers using it must enforce input admission themselves. Parsing reads no database or file
and never checks a Session, identity status, membership, publication or actual bytes.

## Boundary for durable authorization

The next persistent layer must authenticate grant management, record issuer/audit provenance and
revision changes, and pin its own scope and installation. Current-Session operations must derive the
recipient or publisher account from audited current identity, not accept it as a client claim.
Reading must compare the exact trusted Published manifest and owner/account, then recheck current
grant revision, revocation, membership, account generation and fresh database time in the protected
transaction. Byte work needs a separately reviewed admission and in-flight revocation boundary.

The existing [Session private metadata](large-file-session-metadata.md) commands remain owner-private
and do not consume these contracts. There is no new schema, migration, installer, live-auth change,
HTTP/SDK/UI connection, file access, transport, receipt recovery or deployment change.

## Verification

Default `file_access_tdd` cases exercise actual constructors, strict raw JSON and selectors.
`scripts/tests/fileAccess.test.mjs` checks composition and bilingual documentation only.
These pure tests do not establish database locking, durable revocation, current-Session authorization,
real publication, network delivery or LAN security acceptance.

## Recorded execution 2026 10 11

Compiled publisher Red: **0 passes / 1 expected failure**, with **8 temporary fixture warnings**.
Compiled recipient Red: **0 passes / 1 expected failure**, without warnings. The initial **17-case**
Green was followed by a separate compiled state-alias Red: **0 passes / 1 expected failure**.
String-only state decoding then passed all **18 default cases**, without compiler warnings.
Valid ordinary-Serde positional controls and isolated principal/account/authority drift checks guard
against negative cases passing for unrelated type errors. This run uses no database or transfer service.

The backend-only gate passed **832 default Rust tests** and **248 common checks**, with **742 ignored
cases**, **75 Rust result blocks**, zero failures and no compiler warnings. Version/API, formatting,
length and generated-course checks passed. Opt-in SQL, frontend/browser and live LAN acceptance were
not rerun. All **104 prior project-status blocks**, **12 prior large-file execution records** and the
dated capability tensor remain unchanged. Source stays **0.5.5**; no commit, push or deployment change.
