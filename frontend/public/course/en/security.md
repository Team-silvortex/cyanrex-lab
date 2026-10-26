# Security and deployment boundaries

Source **0.6.3** includes explicit loopback/TLS hosting, limited Session issuance and private
Task create/list/reopen/manifest replacement. Browser/SDK saving, LAN access
and migration remain absent from that host; earlier implementation-stage labels retain their original scope.

Unreleased [retained text publication](platform-text-artifact-connection.md) has a separate create-only
connector and POST adapter, explicitly selected by both loopback CLI hosts. Original root/storage checks,
deferred exact-head/byte validation and final
fresh Session authorization precede confirmed COMMIT; unknown outcomes preserve files without retry,
adoption or cleanup. Exact Origin/marker/Cookie, bounded body collection and response-lifetime admission
precede that command. The new constructor validates seven capacities before splitting one original
storage Arc; old constructors retain seven routes/six lanes without publication. HTTP response, actual
text workers (four) and global connections (16) remain distinct budgets, not 28 guaranteed requests.
The combined role retains old credential/Task/row-lock grants and adds only three Artifact INSERTs;
no automatic grants, pooled Artifact reads, browser save or teacher-wide permission are added.

Unreleased [manual Draft manifest replacement](platform-task-replace-connection.md) adds only exact
`PUT /platform/v1/tasks/{task_id}/content` with schema version, expected revision and complete manifest.
The current owner may revise a Draft title or existing exact owned text references; old and new bytes
are checked around the Task/outbox mutation before final fresh authorization and COMMIT. The route
does not publish Artifacts, change status, accept a claimed owner or grant teacher-wide access.
Its Unreleased browser metadata-only use below is not generic text/SDK saving. Unconfirmed results
still grant no retry, adoption or rollback certainty.

Unreleased [empty Task creation](platform-task-create-connection.md) adds only current-owner manual Drafts
at `POST /platform/v1/tasks`. Its strict body contains only `schema_version`, `task_id` and `title`;
owner and workspace derive from current audited membership and the original storage binding. Exact POST
Origin/marker/Cookie and final fresh Session checks precede confirmed creation. No payload/Artifact
publication, browser saving, teacher-wide permission, retry, adoption or rollback certainty is granted.

The separate Unreleased [retained Task content read](platform-task-content-connection.md) adds only
GET reopening of current-owner empty/text Tasks. Exact bytes inherit the original root descriptor,
actual workers retain slots after cancellation, and final root/storage checks precede fresh Session
authorization and commit. No content writes, cross-owner review or browser saving is authorized.

The Unreleased [retained Task index](platform-task-index-connection.md) exposes only the current
owner's metadata in the explicit workspace host. Original storage checks precede final fresh Session
authorization; a teacher role grants no cross-owner listing. No credential-wide SELECT, content route,
automatic grant, schema installation or normal teaching-auth fallback is added.

The Unreleased [native HTTPS loopback host](platform-loopback-tls-host.md) uses private TLS material
and the original unprivileged Session path, without exposing Engine or providing a plaintext proxy.
Clients must validate their explicit CA and exact IP; server key/SAN matching is not PKI trust.
Secure Cookies are not port-isolated. LAN/DNS listeners and Cookie-safe browser deployment remain separate.

The default-off `--session-ui` flag adds seven fixed compiled assets on the exact literal-loopback
Origin, with external scripts/styles, restrictive CSP and five-language labels. It calls existing
dedicated platform Session login/check/confirmed logout, current-owner Task index/content GETs and
the existing metadata-only manifest PUT. It adds no Next.js/teaching-auth fallback, raw-text saving,
Task creation, Run writes, CORS, proxy,
storage or database grants. Explicit 20-entry pages do not establish content readability; reopening
validates exact scope, safe revisions, ordered pins and local SHA-256 before inert text display.
Duplicate/escaped JSON keys are rejected. Limits are 128 KiB index/50 MiB content wire, 16,384 frames
and 32 × 256 KiB decoded text. The ten-second browser bound limits waiting, not synchronous parse time
or server work. Task 401/403, Session checks, confirmed-logout dispatch and pagehide clear old private
data; stale replies cannot revive it. Malformed replies or disconnects remain unknown, not rollback
or automatic retry authority. Clearing password/OTP inputs on handoff is not secure erasure, and
Cookies remain host-scoped rather than port-isolated. The local draft/save-attempt paths stay separate.
An **Unreleased** editor changes only an existing manual Draft's title and display filename/language
hints; pins/order/bytes/count stay fixed. Target/revision-bound preview and dirty/unknown leave dialogs
default to Cancel. A single PUT is bounded to 64 KiB; only strict matching HTTP 200 metadata is confirmed,
with a 128 KiB/4,096-frame reply and ten-second browser wait, not a fresh content read. Conflict or
unknown never replays. Metadata 401/403 headers immediately empty private DOM, including previews;
context changes/pagehide also quarantine work in closed page memory. Only that same bounded denial
envelope may refine Session status, never reveal the buffer; changed epochs fence its completion.
A later Session cannot reveal, restore or replay it; explicit discard is required before continuing.
This is not secure erasure or rollback. No new Artifact,
SDK surface, HTTP API or database grant is added.
The [browser runner](platform-loopback-tls-host.md#cookie-and-verification-boundaries) needs explicit
SQL/browser/trust prerequisites; local legacy NSS currently blocks TLS-browser coverage. Native TLS
acceptance does not establish browser TLS acceptance; do not bypass verification or modify system trust.

Optional [service notifications](platform-host-notifications.md) send only fixed Ready/Stopping values
to an explicitly selected local datagram peer. Delivery is not manager acknowledgement, continuous
health or rollback; notification errors cannot skip the original shutdown drain.

Source release **0.5.8** (**2026-10-13**) includes these follow-ups. Dated `Unreleased` labels retain
their original implementation stage, source versions and verification scope. Source inclusion does not
mount preparation APIs, connect browser saving, switch live authentication, migrate storage or validate
a running deployment.

The [ordered download](large-file-download-client.md), [manifest HTTP](large-file-manifest-http.md)
and [exact manifest client](large-file-manifest-client.md), selected saving, observation/cancellation
and paired distribution router are now included in 0.5.8; dated 0.5.7 evidence stays separate.

This guide covers the current runtime, local editing and general collaboration preparation. The
platform direction does not make the deployed Engine unprivileged or migrate its authentication.
For relationships between modules, see [architecture](architecture.md) and the [feature map](platform-network.md).

## Key Conclusion

Cyanrex Engine is a privileged kernel experimentation environment, not a multi-tenant safe sandbox.

Docker requires access to host/VM eBPF, tracefs, BTF, and bpffs. Authenticated users, including students,
can perform kernel experiments; teachers also control deployment settings. Do not expose Engine to
untrusted networks or treat application roles as kernel isolation.

## General platform security boundaries

| Surface | Current guarantee | Not a guarantee |
|---|---|---|
| Local task payload editor | Bounded text/JSON, parent-owned revisions and target-bound confirmations | Persistence, server identity, execution safety or isolated JS/TS workers |
| Opt-in private Task reader/metadata editor | Explicit private pages, epoch-fenced text and confirmed metadata-only Draft PUT with leave guards | Index/metadata proves fresh bytes readable, continuous revocation detection, raw-text saving or automatic recovery |
| Unmounted content-save attempt | Frozen existing-Draft pins, local hashes and single-use descriptors; supplied DTO consistency | Authentication, actual dispatch, network/COMMIT provenance, current-editor save or durable recovery |
| Direct Task/Artifact/Review stores | Explicit scope and trusted-owner filtering, bounded records and confirmed database writes | Authentication of a caller-supplied owner or historical Session provenance |
| Session resource commands | Current account incarnation, audited membership, namespace pins and final Session checks in one transaction | Teacher access to others' private content, cross-user Review authority or Task acceptance |
| Artifact publication and Task replacement | Immutable exact references; Draft changes check old/new inputs and expected revision | Atomic file/database rollback, automatic adoption or permission to delete newly published files on failure |
| Domain catalogue | Exact configured definition metadata | Trusted package installation, typed evidence validation by admission alone, or policy execution |
| Telemetry and business events | Separate live event history and transactional resource outboxes | Reliable dispatch, exactly-once workflow delivery or a complete security audit from the Events page |

The [browser-side attempt](task-draft-publication.md#unreleased-browser-side-existing-draft-save-attempt)
uses generation/revision/context-epoch guards only as local stale-work fences. Handed-out descriptors
stay conservatively unknown until a valid supplied confirmation; stale/late receipts cannot erase
pending uncertainty or authorize retry, deletion or restoration. It adds no transport or Cookie/Origin
configuration. The separate opt-in Session shell does not mount this attempt; Task-save transport
and Cookie-safe deployment still need review.

Preparatory installers accept only explicit empty namespaces. Task storage schema 2 rejects schema 1;
do not edit a version number, drop data or rerun bootstrap to bypass refusal. Live teaching tables,
credentials, private Artifact roots and rollback material are not synthetic test fixtures. Retain
uncertain outcomes for reviewed reconciliation; timeout or lost acknowledgement is not proof of rollback.

For source-owned private work, see [Task inputs](session-task-inputs.md), [Draft replacement](session-task-revisions.md)
and [private Reviews](session-review-commands.md). These paths are not yet public browser APIs, so
their service tests do not establish live HTTP/CSRF or deployment acceptance. Application roles and
source-level abstractions still do not isolate the shared Linux kernel.

### Private metadata index boundary

The independent **2026-10-11 Unreleased [task-content index](task-content-index.md)**, on source
0.5.5, returns only owner-private metadata. Trusted listing does not authenticate a supplied owner;
Session listing derives that owner, locks bounded heads and rechecks namespace/current authority with
fresh database time before commit. Selected records and lookahead fail closed on corruption or
unconfigured definitions. A same-scope foreign/nonexistent cursor is only a position, never a grant.
No Artifact namespace or blob is accessed, so listing does not prove readable content, unscanned-row
integrity or whole-store health. No HTTP/SDK/UI, write, installation, migration or recovery is added.

The separate same-day **Unreleased [index HTTP adapter](task-content-index-http.md)** on source
0.5.5 delegates only to that current-Session command. Raw query fields cannot select an owner,
scope or namespace; the dedicated Cookie/marker and optional exact GET Origin precede shared
response-lifetime admission. Metadata stays private with `no-store`; the route remains unmounted,
without a Session issuer, browser/SDK connection, content-availability proof or global capacity quota.

### Private chunk staging boundary

The **2026-10-11 Unreleased [Unix staging API](large-file-staging.md)** on source 0.5.5 is trusted-only.
It pins an existing owned 0700 root, binds each exclusive entry to the entire typed manifest/index,
checks exact bytes, synchronizes file/directory and reads back the originally created inode.
All existing targets conflict. Creation/post-creation uncertainty retains files without adoption,
overwrite or cleanup. Current verification is not an earlier write receipt or upload progress.

Clone-shared worker slots belong to actual queued/running jobs after caller cancellation; separate
handles have additive budgets. Neither 0400 permissions nor metadata observations isolate the same
UID/root, and file work has no kill/deadline or reserved-disk quota. No upload journal, current-Session
authorization, publication/grant, public route or live composition is added.

### Trusted upload-intent boundary

The separate [upload ledger](large-file-upload-intent.md) binds supplied owner/account generation,
fixed namespace/table incarnations and persisted logical limits; it is not a current-Session guard.
Writers serialize quota admission and validate all bounded records, including foreign owners and
all account generations. Errors never release reservations or adopt staged bytes. Metadata reads
do not prove uploaded bytes, old commit provenance or retry authority. No transport is connected.

The separate [upload-bound staging adapter](large-file-upload-staging.md) captures the original source
and exact reservation, then rechecks them before and after file work using fresh READ COMMITTED reads.
Its one-shot in-memory attempt becomes Unknown before its first admitted await; only original-object
readback can produce a local receipt, which retains the original read-only descriptor. No SQL lock
spans IO and no cross-storage atomicity, persistent progress, retry or current Session grant is claimed.

The independent [chunk journal](large-file-chunk-journal.md) requires an acknowledged Unknown row
before file dispatch. Unique upload/index coordinates remain occupied in either status, regardless
of changed source/root bindings. The journaled wrapper rechecks original source/file observations
after final confirmation acknowledgement. A later error can leave recorded Confirmed with a caller
still Unknown; metadata inspection neither recreates a receipt nor permits retry or cleanup.

Separate [whole-upload verification](large-file-whole-verification.md) reads current `u1` bytes,
checks ordered chunk/whole digests and rechecks original source/root/entry observations. It neither
uses recorded status as byte proof nor changes it. Metadata results do not retain chunk descriptors
or establish original write provenance, permanent integrity, current Session authority or publication.

Separate [persistent publication metadata](large-file-publication.md) requires both original complete
confirmed journal metadata and fresh real whole-file bytes. Its immutable insert is create-only;
upload/version conflicts are never adopted. Original source, journal, publication and file checks
continue after acknowledgement. A later rejection can leave a published row and an Unknown caller.
Neither the row nor a matching read restores a receipt, grants download authority or proves current
file health, permanent physical sealing, rollback or permission to retry or clean up.

Separate [Session private metadata](large-file-session-metadata.md) requires a current audited active
Human/member, derives owner/account generation on the server and uses one source transaction with
original source/target pins, selected row/marker locks and a final fresh policy/expiry check.
Recreated accounts and teacher status cannot inherit another owner's records. Private reservations
and metadata reads grant no publishing, file-byte access or recipient permission.

Separate [pure file-grant contracts](large-file-access-contract.md) compare exact account generations
and versions with active state and a bounded expiry interval. Neither JSON nor a true `matches`
result establishes issuer provenance, current audited policy or download authority. A stale active
copy can still match after revocation; future commands must read and recheck current grants, not
reuse that boolean. Publisher revocation does not implicitly revoke independent recipient records.

The [Unreleased persistent publisher ledger](large-file-publisher-grants.md) records trusted issuer
and revocation actor attribution; it does not prove either caller's Registry permission. Own Session
reads require current audited Human membership, exact account generation, original grant/history
pins and final fresh authorization before commit. A coarse table SHARE fence also prevents history
append phantoms during that transaction. Expired/revoked records remain readable metadata, not
usable permission. Future management needs explicit `ManageFileGrants`; deployment grants and
teacher status cannot substitute. No recipient or byte-access authorization is added.

Later [Session grant management](large-file-grant-management.md) authenticates the exact current
account selected by trusted-host configuration, never a request-selected issuer or inherited role.
Issue rechecks the target's active audited Human membership; revoke permits retired/deleted subjects.
Replacing the host configuration does not revoke surviving clones/in-flight work; drain and destroy
old handles or restart. Persistent online management revocation, publication consumption and recipient
or byte-access authorization remain separate gates. Historical trusted attribution is not upgraded.

The independent [Session publication](large-file-session-publication.md) now requires the current
exact actor and an original active publisher grant before file work, then rechecks the first
source/Registry/grant pins in a fresh final source transaction. Original grant/upload/journal fences
cover the insert and deferred/final checks; the last clock sample covers grant and Session expiry.
No SQL lock spans files, and dispatched work is not continuously revoked or killed by cancellation.
Post-commit failures may leave Published with Unknown; downloads, recipient authority and recovery remain absent.

The later [recipient ledger](large-file-recipient-grants.md) stores trusted exact-file/account bindings
independently from publisher grants. Current own-recipient metadata remains selected and fenced through
source authorization and commit. Future/expired/revoked metadata is readable, not download permission;
neither a claimed file reference nor Active state proves an exact trusted Published version.

The October 12 [publisher-owned sharing](large-file-sharing.md) adapter authenticates the current
original publisher, matches every expected version coordinate to an own selected Published record,
and holds permission/history/publication/write fences through deferred and final fresh source checks.
Revocation matches original publisher/account before UPDATE and needs no live publishing grant,
publication or recipient. It grants no bytes or global teacher delegation; unknown commit is not rollback.

Independent [download-manifest observation](large-file-download-manifest.md) requires the current
audited recipient and an original Active exact-file grant. Recorded publisher/account and file/version
filter Published selection before decoding; full version comparison and original pins precede final
fresh grant/Session validity and acknowledged commit. A teacher or owner without a recipient grant is
denied. The manifest does not authorize later byte disclosure; publisher revocation does not cascade.

The separate [chunk reader](large-file-chunk-read.md) privately verifies at most 4 MiB before original
authorization reentry and commit. No caller sink sees an unverified prefix. Final same-clock Session/
grant expiry precedes disclosure, with no later IO; the result is a verified read-time byte snapshot,
not ACK-time pathname health or continuous authority. Returned bytes retain their capacity lease.

The independent [binary HTTP adapter](large-file-chunk-http.md) adds strict seven-key raw query,
dedicated cookie/marker/origin, empty-body admission and rejection of ranges/conditionals. Binary
data retains BOTH local budgets through the last owner-retaining Bytes reference, not merely body EOF;
an empty slice can detach without retaining sensitive data. The first
frame must be polled within ten seconds of source return; there is no autonomous lease-release timer,
poll-time authorization or remote-delivery guarantee. Source-approved bytes cannot be reclaimed by
later revocation. The router remains unmounted and emits no user filenames, hidden keys or partial bytes.

The separate [supplied-response receiver](large-file-chunk-receiver.md) rejects duplicate/mismatched
metadata, encodings/ranges/cookies, oversize/short data and any trailing error or trailer before
file dispatch. Explicit EOF, SHA-256, idle and fixed-total deadlines precede private exclusive
staging; queued/running work keeps the receiver's buffer/control after waiter cancellation. No
remote metadata selects a path. Conflict is not adoption; an ambiguous post-creation file outcome
is not rollback. This adds no requester, credential policy, current remote authority or live mounting.

The separate [explicit client](large-file-chunk-client.md) admits only a host-frozen canonical origin
and typed selectors. HTTPS validates certificates and SANs; plaintext requires literal-loopback
opt-in, not DNS localhost. Per-call sensitive cookies never enter query/errors/default state.
Retry/redirect/proxy/decode are disabled; header and body deadlines differ, and whole-call admission
travels into actual save work. It proves neither raw socket EOF, hard cancellation of remote/OS work,
current remote identity/workspace authority nor a mounted distribution service.

Separate [private whole-file assembly](large-file-chunk-assembly.md) prechecks legacy chunk bytes,
then exclusively creates a fixed manifest-bound output in the original 0700 root. Checked copying,
0400 sync/readback and original input/root rechecks precede acknowledgement. No user filename/path,
Source key or retained output is adopted. Creation uncertainty keeps evidence; cancellation neither
kills actual work nor permits cleanup/retry. Three payload passes use bounded buffers/metadata,
not disk reservation, atomic public publication, current remote authority or permanent same-UID sealing.

### Prepared HTTP admission follow-up

The [ordered downloader](large-file-download-client.md), included in 0.5.8, retains one
whole-flow slot across each actual save/assembly job. Cancellation cannot start the next request or
release a live job's slot. Empty success is zero-HTTP local assembly, not remote authentication.
Each nonempty server request still requires current recipient authority; no final continuous grant
check or original receipt-identity continuity is promised. Existing files conflict, never become resume.

The independent Unreleased [manifest GET](large-file-manifest-http.md) exposes only Source-selected
private metadata after current exact recipient authorization. Six selectors choose no scope/owner;
the existing credential/body gates and sanitized errors precede bounded JSON projection. HTTP slots
end with the body, not retained JSON Bytes. Returned snapshots and later revocation do not authorize
bytes; normal startup, Session issuer and live API remain unchanged.

The new [exact manifest client](large-file-manifest-client.md) checks bounded singleton headers,
actual framed EOF, strict raw JSON and the full expected version through the existing controlled
transport. Its composed empty-file case still requires remote metadata GET. That observation is not
continuous authorization or authenticated principal provenance; later chunks reauthorize independently.
Manifest cancellation releases metadata/flow gates before file work; real jobs retain the old flow
lease. No existing output is adopted, and there is no live mounting, UI save dialog or resume.

The new [explicit private save](large-file-save.md) uses the original pinned assembly descriptors,
not matching version metadata or an adopted replacement inode. The host's existing owned 0700 parent
and independent basename select an exclusively created file, with actual copy and original-inode
readback. No remote filename, overwrite, cleanup, remote grant or ordinary-directory/UI policy is
inferred. Real jobs retain staging admission after cancellation; uncertain output remains evidence.

The new [authorized delivery call](large-file-delivery-client.md) joins these stages in one explicit operation.
One original whole-flow lease remains owned by actual chunk/assembly/save work after cancellation;
first failure stops with no retry/adoption, and UI saving, resume and normal mounting remain separate.

The separate [owned user-directory target](large-file-user-save.md) explicitly accepts 0700/0750/0755
without relaxing private staging. Original directory UID/GID/mode checks and checked saving remain;
file picking, writable shared folders, resume and live mounting are separate.

The optional [live delivery observer](large-file-delivery-progress.md) exposes one latest per-call phase
and actual chunk-ACK prefix without callback/back-pressure queues. Full staging is not saved output;
closure is not job termination or a receipt, and UI/durable progress/resume remain separate.

The optional [cooperative cancellation handle](large-file-delivery-cancellation.md) stops one call's
waiting when its request is selected. Handle drop is not cancellation; already-dispatched jobs keep
resources and may finish, and a request is not an instantaneous dispatch fence or cleanup authority.

Explicit [whole-delivery HTTPS verification](large-file-delivery-https.md) exercises the existing
chain with disposable native TLS peers. Transport identity is not current file authorization;
loopback acceptance does not establish deployed LAN, UI or unprivileged-service isolation.

An explicit [distribution router composition](large-file-distribution-router.md) now derives both
endpoints from one supplied Source and frozen chunk workspace. HTTP budgets remain independent;
this cached configuration consistency is not a Session issuer, live mounting or privilege isolation.

Unreleased after the 0.5.8 tag, [real Source over HTTPS](large-file-source-https.md) adds one
combined native authorization/file/router/TLS/saving test chain, including pre-dispatch revocation
and actual response-body cancellation. It changes no production listener, policy or dated score.

The **2026-10-08 Unreleased C2-N follow-up** is not part of the published 0.5.4 tag.
Its [explicit content router](task-content-http.md) shares four request/response slots by default
(trusted range 1–16) across state clones, including independently built routers using those clones.
Cheap endpoint and credential-header checks precede admission; a full budget returns private 503
`capacity_exhausted` / `not_attempted` without body polling, SQL or `Retry-After`.

The slot covers collection, the existing Session command, serialization and the returned response
body until terminal frame/end, error or drop. Admitted error bodies also retain slots. Cancellation
does not prove rollback or terminate already-dispatched blocking I/O. Separate states have separate
budgets; this is not a global memory/worker/network/proxy-buffer quota, isolation or retry mechanism.
It adds no listener, Session issuer, public/browser API, live-auth cutover or journal integration.

### Prepared text publication boundary

The **2026-10-08 Unreleased C2-X** [text Artifact router](text-artifact-http.md) is also outside the
published 0.5.4 tag. Trusted configuration selects the source, scope and private file root; requests
cannot choose them or an owner. The dedicated Cookie/Origin/marker boundary delegates current
authorization to existing Artifact commands, fixes text kind/media type and rejects invalid text.
The 2 MiB wire budget is distinct from the 256 KiB decoded limit. Exact reads can materialize up to
1 MiB in the generic reader before stricter text projection; this is not a 256 KiB peak-memory guarantee.

Cloned text-router states share response-body-lifetime slots, but separate states and Task-router
budgets add together. Post-dispatch errors are unconfirmed; missing/foreign targets share 404, corrupt
content remains an error, and duplicate publication conflicts even when bytes match. Neither a lost
response nor a later Task failure permits retry, adoption or cleanup. No public mounting, automatic
router composition, journal dispatch, Session issuance or browser save is added.

### Prepared catalogue observation boundary

The independent **2026-10-11 Unreleased [C2-AA observation](session-catalog-task-draft-observation.md)**,
outside the published 0.5.4 tag, admits only the exact unknown target of its surviving catalogue
attempt. Its original token fingerprint is not authorization: one existing reader rechecks current
Session/account/membership and namespace pins. Full definition and actual-byte corruption remains a
typed error rather than a valid difference or absence. Body-free metadata is still private; a match
does not prove earlier commit provenance, and `NotVisible` does not authorize cleanup or retry.
Observation cannot change the attempt, adopt progress, restore it, cross into manual/journal types or
grant HTTP/browser access, provider execution or Task acceptance.

## AI Agent host boundary in 0.5.3

[Connection profiles](ai-agent-integration.md) store nonsecret metadata and symbolic credential names
only, behind teacher authority/CSRF. The Engine never contacts the configured URL or resolves its
reference; the browser has no secret input/storage. HTTPS or literal-loopback HTTP URL validation is
not provider identity verification. The trusted host owns secrets, transport and disclosure of private
tool outputs. The explicit SDK catalogue excludes authentication/settings/kernel writes, retains
server authorization and requires a separate trusted approval for every selected mutation. Model text
cannot approve itself. Call IDs are an in-memory guard, not durable exactly-once execution; cancellation
or a generic unconfirmed error must not become permission to repeat a write with a fresh ID.

## Default Protections

The protections in this section apply to the existing runtime unless a preparation-layer contract is
explicitly named. They must not be treated as interchangeable authentication or storage policies.

- Docker published ports default to `127.0.0.1`. The native/WSL launcher currently sets
  `ENGINE_HOST=0.0.0.0`; verify actual listeners and restrict access before starting that mode on a LAN.
- First startup generates random DB, deployment-teacher, and TOTP secrets (legacy `CYANREX_ADMIN_*` keys).
- `.env` permissions are `0600` and ignored by git.
- User registration and OTP bootstrap are disabled by default.
- eBPF checks, semantic completion, script list/save/delete, and session-bound operations are available to authenticated users.
- Teachers own module changes, system settings, Runner Agent management and teaching; students cannot
  use these management APIs. Old administrator identities are compatible teacher identities.
- Public registration cannot select a role or claim a configured teacher/legacy-admin name. The seeded
  deployment teacher cannot delete itself, preventing a student-only, unmanaged instance.
- Temporary lockout after repeated login failures.
- Global and per-user Runner capacity, source size, and execution time limits are enforced for eBPF tasks.
- Runner status reports `shared_kernel`; local quotas do not claim tenant isolation.
- Local check/completion caches are owner-scoped. Their source workspaces use `0700` on Unix and
  cleanup guards run on return or cancellation; failed cleanup is logged. These are hygiene measures,
  not an independent-kernel boundary.
- DB stores session token hashes, not raw usable tokens.
- Cookie-authenticated `/ws/events` handshakes also enforce the configured Origin/Referer policy;
  CORS headers alone do not protect WebSocket connections. Native clients must send an allowed Origin.
  The existing `CYANREX_ALLOW_MISSING_ORIGIN` override also affects this endpoint; keep it disabled on
  a LAN unless its security tradeoff is explicitly accepted. See [Event Stream Recovery](event-stream.md).
- Event live queues use the authenticated owner rather than a client-supplied identity, preventing
  cross-owner queue eviction. Capacity is per active owner; this is not a global connection/memory quota
  or a tenant-isolation boundary. The existing Origin/session policy and shared-kernel limits still apply.

These measures reduce accidental exposure, but they do not make the privileged Engine a shared safe runtime.

### Teacher-authority upgrade

All existing teacher allowlist entries now grant deployment management, not merely teaching reads.
Review both `CYANREX_TEACHER_USERNAMES` and `CYANREX_ADMIN_USERNAMES` before deploying this change.
The default deployment account is the teacher in personal use; this never promotes the first public
registrant, trusts a client-supplied role, bypasses TOTP, or changes kernel/Agent privileges. Configure
additional teachers only after verifying an existing account's owner; see the [teacher guide](teacher-guide.md).

### Accidental-action safeguards

The UI asks for an explicit target/impact review before kernel runs, detach, destructive deletion,
draft replacement, administrative state changes and remote compiler selection. Bulk deletion/cleanup
requires a typed phrase; the default focus is Cancel. In-flight duplicate clicks are blocked, and failures
require checking state before a new confirmation. Event deletion freezes its time cutoff and does not
claim that the bounded visible count is the deletion total. These controls are browser-side ergonomics,
not server authorization, idempotency keys, transactional rollback or kernel isolation. A dispatched
request may complete even if the browser leaves the page. API clients still rely on the existing Engine
session, teacher/student role, CSRF, password and OTP policies. Role consolidation does not replace
these server-side checks.

### Sessions and database outages

Logout ends the current session, not every device's session or running kernel experiments. The browser
opens the login page only after an HTTP-success response explicitly confirms logout. Rejection, an
invalid response, timeout or network failure instead shows that the session may still be active and
requires a manual retry. Repeated clicks are blocked while waiting. Navigating away discards late UI
callbacks, but does not undo a request already received by Engine.

For PostgreSQL-backed authentication, logout, password change and account deletion require confirmed
storage writes. Failures return `503` without clearing the session cookie or publishing a memory-only
mutation. Session removal and account deletion commit in one transaction. Password/account writes use
the verified normalized username and credential snapshot; zero affected rows are rejected rather than
reported as success. A concurrent password change cannot silently overwrite newer credentials.
Logout with no affected row succeeds only after confirming the session is already absent.
Account deletion also verifies no session survived, including a suppressed cascade.

Login admitted while PostgreSQL is active must insert exactly one session and recheck its verified
credentials in the same transaction before publishing a cache token/cookie. A deleted/recreated
account or concurrent password change cannot accept the old pending login. An unconfirmed login write
returns `503` without a new cookie or memory-only success; cancellation before commit rolls back.
An already committed but unacknowledged login can leave a session until expiry. This does not remove
the existing read/registration fallback or provide distributed revocation.

A `503` or lost response is not proof that nothing committed: restore storage and verify the current
account/session state before explicitly retrying. A failed write alone does not disable persistence;
if an earlier read failure already latched permanent auth DB fallback, restore PostgreSQL and restart
Engine before these mutations. This also applies to temporary accounts registered during that outage.
Intentionally memory-only instances keep volatile account/session behavior. Do not switch a durable
deployment into memory-only mode merely to bypass an authentication storage error.

When PostgreSQL reports a missing or expired session, Engine also removes its memory fallback entry.
A missing account lookup removes that cached account and its cached sessions. Those observed
invalidations cannot reappear merely because a subsequent query fails. The existing volatile memory
fallback still cannot discover revocations or credential changes made elsewhere while the database is
unavailable; this is not a distributed revocation guarantee or a replacement for incident recovery.

The [C1-H source adapter](collaboration-password-change.md), included in 0.4.7, separately supports atomic
self-service password change and all-session revocation with current Session/password/TOTP checks.
It is not wired to the live endpoint described above, does not restore retired identity permissions,
and must not share activated source tables with the legacy live writer.

[C1-I bootstrap](collaboration-bootstrap.md) is explicit trusted-operator code for a fresh namespace,
not an unauthenticated setup endpoint or first-registration claim. It rejects existing/partially installed
schemas, commits initial identity/grants/audits together, and issues no Session. Lost acknowledgement
requires reconciliation, not deleting data or automatically creating a replacement manager.

The separate [C1-J local CLI](collaboration-provisioning.md) restricts endpoints to Unix sockets or
literal loopback, ignores ambient database/runtime settings, and requires private owned inputs plus
an explicit reviewed target digest. TOTP is written only after confirmed commit into an exclusively
reserved private file, never the console. Database commit and file delivery are not atomic: failure
can leave a committed account with missing/partial enrollment material. Preserve evidence and reconcile;
catalog-only `inspect` is neither a healthy-authority check nor a secret recovery or replay mechanism.

[C1-K reconciliation](collaboration-reconciliation.md) reads source/registry/full audit histories in
one bounded read-only snapshot. It rejects filtered/incompatible layouts and reports aggregate counts,
not credentials, token digests or individual identities. A consistent snapshot can precede an in-flight
commit; it neither proves an uncertain command's outcome nor authorizes reinitialization, secret
recovery or protected actions. Existing authorization still needs fresh Session checks and row locks.

The [durable Session-source follow-up](collaboration-auth-source.md), included in 0.5.1, pins namespace and
auth-table identity for login, validation and logout, including login's password-worker gap and
post-write path drift. Only one effective private schema and unfiltered permanent source tables are
accepted; no registry, migration or live authentication switch is implied. This prevents an empty
shadow read from falsely confirming logout, but is not public-issuer readiness.

Prepared registration, login, password rotation and bootstrap now share a process-local gate of four
dispatched password jobs and twenty admitted jobs in total. A blocking job retains its capacity after
caller cancellation/timeout until it actually finishes; work waiting for a slot stays inside the
caller's existing deadline. Separately, prepared account reads and pre-admission verification enforce
the existing Argon2id v19/m19456/t2/p1/32-byte profile, including bounded PHC text, decoded salt and
rejection of duplicate/unknown parameters. Other profiles return `InvalidRecord`, without credential
rewrites or implicit Session revocation. This does not limit old live AuthService work, other processes,
wall-clock time or process RSS, and does not kill running computation. Timing enumeration
and ingress/session limits remain gates before untrusted Session issuance.

The 2026-10-07 login follow-up included in 0.5.2 gives a genuinely missing account the same supported password
profile through one synthetic verification on the shared gate, after source-pin rechecks and confirmed
commit of the lookup transaction. It never substitutes for corrupt credentials or a source/database
error. Login still separately requires the original real account, so matching the public synthetic
record or registering that name during verification cannot issue a Session or consume an OTP.
Input and attempt limits remain earlier checks, and dispatched work retains its permits on cancellation.
This removes the missing-account KDF shortcut, not every timing difference or enumeration channel;
there is no constant-time promise, new public issuer, live-auth switch or schema change.

Prepared login and rotation also recheck OTP freshness behind the writer fence and after the last
SQL wait before requesting commit. Expiry at that point rejects issuance or rolls back rotation.
The schema-2 follow-up also consumes a monotonic per-account counter in the same transaction as
Session issuance or password replacement/revocation. Current full credentials and the old watermark
guard the update; post-write checks and a bound exact-counter recheck precede COMMIT. Independent
sources cannot both commit the same step. Password changes preserve consumption; re-login needs a
later OTP. Collision handling rejects any matching consumed counter, not all future appearances of
the same six digits. Pure proposals alone are not durable state or account authority.

Only fresh explicit installation/bootstrap creates schema 2; existing schema 1 is rejected unchanged.
There is no counter reset, secret recovery, automatic migration or live AuthService switch. A lost
COMMIT reply may leave consumption committed without delivering a token: do not assume rollback or
reset state to retry. This is neither freshness at acknowledgement nor protection against every clock
rollback. Version fencing applies to cooperating prepared writers, not legacy or privileged SQL
writers; they must remain isolated. See [the full OTP contract](collaboration-auth-source.md).

The explicit `prune_expired_sessions` maintenance included in 0.5.2 selects at most 128 expired rows under the
source writer fence and one fresh database cutoff. It validates the complete batch before deletion,
compares returned full rows and confirms all selected digests are absent in the original pinned source,
including attempted reinsertion with future expiry. SQL rejects selected candidates' infinite or
out-of-range timestamps before decoding on this maintenance path. Corrupt/orphaned candidates fail
rather than being repaired. Only a confirmed commit returns the batch count; an empty or partial count
is not proof of global emptiness or a bound on Session storage. Cancellation or lost acknowledgement is not rollback
evidence. The primitive reads no credentials/OTP state and its explicit mutations target only Sessions;
these checks do not audit arbitrary trigger effects throughout the database. There is no automatic
schedule, CLI/HTTP entry or login/read cleanup side effect. Active-Session quotas and maintenance
admission/recovery remain separate policies; see [the cleanup contract](collaboration-auth-source.md).

The 2026-10-07 Session-expiry follow-up included in 0.5.2 rejects infinite and out-of-range `expires_at` values
inside SQL before the shared prepared Session reader decodes them. Validation, login writeback and its
calling guards receive `InvalidRecord`, not a panic or silent absence; normal finite expiry remains
unchanged. Read-only reconciliation keeps the range/expiry comparison in SQL, fetches only an expiry
boolean and returns `InvalidSource` for bad values, without exposing raw expiry. Registry timestamps
were outside that stage and are covered by the separate follow-up below. It changes no
dependency, live issuer, schema or public API; see [the Session contract](collaboration-auth-source.md).

The subsequent 2026-10-07 [registry guard](collaboration-reconciliation.md), also included in 0.5.2, checks stored
retirement and both identity/policy audit times in SQL before decoding. Legitimate NULL retirement
remains valid, while an independent validity flag rejects corrupt non-null retirement; mandatory audit
times cannot become missing receipts. Head/history/replay/append-readback and reconciliation paths
retain explicit errors rather than panic, skip or repair records. This changes no age/order rule,
dependency, schema or live authorization boundary and is not blanket timestamp-safety assurance.

### Browser diagnostic boundary

Browser inline compiler caches and pending checks belong to one editor mount and exact Engine/target/
source/header context, not a process-global pool. Leaving/re-entering the editor discards that cache;
cancelled callbacks cannot publish stale diagnostics. This does not observe cookies changed in another
tab, revoke sessions, erase saved drafts or create an authorization boundary. Diagnostic requests reject
redirects and request no-store; queue/lease ownership is still enforced by Engine. Timeouts and best-effort
job cancellation do not prove server-side rollback. See [bug hunt 03](functional-network-bug-hunt-03.md).

Semantic providers also reject foreign/disposed models and have editor-owned requests/cache; manual
header checks cancel on source or context changes. Completion and selected-header reads have ten-second
whole-request limits; manual checks use the local 20-second limit. All three request paths reject redirects
and request no-store. A failed metadata refresh displays the last successful list explicitly, never a
claim that no headers are selected. Client context revisions only invalidate UI work: Engine still chooses
the session owner and current headers, and teachers retain exclusive header mutation authority. There is
no atomic header snapshot, cross-tab revocation detection, remote completion or automatic execution added.
See [bug hunt 04](functional-network-bug-hunt-04.md).

### Browser runtime boundary

Manual run/detach requests include credentials, request no-store and reject redirects. A local admission
guard prevents concurrent dispatch from one controller; it is not cross-tab idempotency. Navigation aborts
browser waiting; editing hides old results, and neither action unloads an already dispatched program.
The 330-second mutation and 20-second inventory limits are active-browser waiting bounds, not proof of
server rollback. Uncertain results require review and never trigger an automatic retry or detach.
Only explicit `clean: true` confirms cleanup; a failed/malformed inventory is not empty success, and stale
inventory disables cleanup controls until refreshed. Engine still enforces session ownership, CSRF and
Runner authorization. The explicit `pin_path: null` bulk action retains its existing current-account scope,
including attachments added before execution, as disclosed in its typed confirmation; it is not a frozen
server snapshot. No new session-revocation detection or kernel inspection is added. See
[bug hunt 05](functional-network-bug-hunt-05.md).

Breakpoint session/line filtering, strict trace-marker parsing and model-owned highlights prevent stale
or malformed observations from being attributed to the current draft. They do not authenticate a kernel
producer: session markers in a shared trace log are not secrets or a tenant boundary. Only the Engine's
session/Origin checks and owner-scoped event subscription confer access. Event recovery snapshot reads
also request no-store and reject redirects; malformed frames retain a possible-gap notice, not a claim
of complete recovery. No new revocation, replay guarantee or automatic kernel action is added. See
[bug hunt 06](functional-network-bug-hunt-06.md).

### Event history and acknowledgement boundary

Event export/deletion and unread/acknowledgement requests retain credentials, request no-store and reject
redirects. Browser waiting is bounded to 20 seconds for export/deletion and ten seconds for unread state;
navigation cancellation is not server rollback. Failed or malformed deletion/acknowledgement responses
are never treated as success, and mutations are not automatically retried after failure. The unchanged
mark-read endpoint covers ALL events for the session owner, including rows outside the current filters
or 200-row view; the UI explicitly discloses this. It is not a per-record or durable acknowledgement.
MIME/filename checks prevent accidental wrong-format downloads, not spreadsheet formula sanitization,
archive authenticity, complete event replay or a new payload byte budget. See [bug hunt 07](functional-network-bug-hunt-07.md).

Engine event mutations now wait for earlier queued writes and retain per-owner admission through admitted
SQL work and cache publication. This closes local ordering/cancellation races without changing session,
CSRF or teacher authority. The private replacement helper rejects foreign-owner records. Queue overflow
and storage deadlines warn and latch volatile fallback; successful read/deletion HTTP responses still do
not promise durable revocation of events across restart or other processes. Do not use this stream as an
exactly-once security audit log. Transactions protect settings/trim and replacement, not the whole
publication/HTTP/recovery lifecycle. See [bug hunt 08](functional-network-bug-hunt-08.md).

### Learning data boundary

Learning record reads fail visibly on corrupt/unreadable storage instead of showing an empty classroom.
Successful learning projections and their storage errors are no-store. New local learning directories
and snapshots use Unix 0700/0600 and exclusive random temporary files with failure cleanup. Keep existing
data-directory ownership and parent permissions restricted; old directories are not automatically chmodded,
and this does not provide crash recovery or hostile-parent path protection. See [learning storage](learning-storage.md).

### Classroom connection boundary

Classroom onboarding uses a separate [discovery/invitation boundary](classroom-connection.md).
Discovery labels and IDs are not certificates; verify the exact HTTPS teacher origin independently.
Never send the shared Agent bootstrap token to students. Invitations are opt-in, short-lived,
name-bound and atomically single-use, with no automatic login or teacher promotion. Compatibility
checks the join protocol and required capabilities, not merely matching product patch numbers.
SSH management stays in the native operator CLI with strict host checking and explicit target review;
it cannot be triggered by a student invitation or discovered command. Neither entry changes kernel
isolation or listener exposure, or removes the need for trusted ingress and per-student isolation.

### Runner Agent credential boundary

Remote Agent registration is disabled by default. Enabling it requires a secret
`CYANREX_RUNNER_AGENT_TOKEN` of at least 32 characters. Keep the endpoints on a private management
network or behind TLS and source restrictions; never place the token in frontend code, logs, or a
repository. Rotate it when a node is lost or compromised.

The pre-shared Bearer token is accepted only for registration. Registration returns a distinct
256-bit Agent credential once with `Cache-Control: no-store`; re-registering the ID rotates it.
Heartbeat and job endpoints require HMAC-SHA256 over method, path, Agent ID, Unix timestamp, nonce,
and body hash. The timestamp has a bounded freshness window and each nonce is accepted once.

A holder of the bootstrap token can still re-register and impersonate an Agent ID, so it remains a
high-value secret. Jobs have per-claim lease tokens, deadlines, cancellation acknowledgement, and
size-limited results. The editor's local compiler remains the default; explicitly selected remote
checks carry source to a capability-matched Agent but never load or return the object. The public
backend inventory omits labels and kernel details, remote jobs are bound to the authenticated user,
and each user is limited to two active checks. `/ebpf/run` remains local. Signed transport proves
which registered credential sent a result; it does not prove that compilation was honest. Consider
mutual TLS, node-bound keys, and attestation across a stronger trust boundary.

Lost leases observed before execution are discarded rather than executed or acknowledged as cancelled;
the polling loop remains available for later jobs. This does not interrupt compilation already in progress.
Unclaimed user checks expire after a 35-second wait on the next queue access, releasing source and user
quota; staff-managed unowned jobs keep their explicit cancellation policy. Response buffering rejects
more than 640 KiB while streaming, even without a declared length, before reading the whole response.

The bundled standalone Agent fails closed on non-loopback plain HTTP unless an explicit insecure-lab
override is set. It disables HTTP redirects and environment proxies, caps response bodies, and can
read the bootstrap secret from a mounted file. Compile mode is opt-in and rejected for
`shared_kernel`; it invokes only an absolute Clang path with fixed arguments, clears the environment,
uses private disposable workspaces, bounds process resources and captured output, and deletes the
object after hashing it. Source preflight also rejects non-literal or path-escaping include forms.
Run the Agent unprivileged without host PID mode, kernel mounts, Docker socket, or Linux
capabilities. Clang processes untrusted input, so retain a container or VM boundary.

The optional managed Compose profile applies that baseline automatically: numeric unprivileged UID,
read-only root filesystem, no published ports, all capabilities dropped, `no-new-privileges`, a
`noexec` temporary workspace, and PID/CPU/memory caps. Its manager writes the bootstrap value to a
mode-0600 host file mounted as a Docker Secret and never prints it. The profile remains disabled
until the teacher explicitly starts it. A dedicated internal network permits Engine control
traffic but blocks direct access to the default database/frontend network and external networks.

## Recommended Isolation

For multiple students submitting their own code, the chosen next-stage target is an unprivileged
teaching control service plus an exclusive Linux VM per active student. Desktops may host these VMs;
centralized execution hardware is optional. This target is not yet implemented. See
[Linux Desktop and LAN Classroom Isolation](classroom-isolation.md) for ownership, recovery, and the
limits of trusting results from student-administered machines. Do not enable remote loading by simply
giving the current compile-only Agent elevated privileges.

### Personal Computer

One instance per student. Windows users should use WSL2, Linux users choose native Linux or Docker. This is preferred.

### Remote Personal VM

One VM per student, accessed via SSH tunnel. A VM can be fully destroyed after class.

### Centralized Server

Do not share one Engine among multiple untrusted students. If resources are limited, isolate with VMs/microVMs,
independent lab nodes, and limit network, CPU, memory, and lifetime per node.

## Prohibited Operations

- Do not set `CYANREX_BIND_ADDRESS` to `0.0.0.0` and expose directly to public internet.
- Do not commit or share `docker/.env`.
- Do not mount Docker socket into Engine.
- Do not disable auth or treat public registration as classroom isolation.
- Do not allow unknown source programs from students.
- Do not run production workloads on the same trust boundary as teaching Engine.
- Do not treat verifier as a full malware sandbox.

## Remote Access

Prefer SSH tunnel:

```bash
ssh -L 3000:127.0.0.1:3000 \
    -L 8080:127.0.0.1:8080 \
    USER@SERVER
```

Long-running services should use TLS reverse proxy, source restrictions, and host firewall, and set
`CYANREX_SECURE_COOKIES=true`. Enable Secure Cookie only when browser access is HTTPS.

Current Compose uses `CYANREX_BIND_ADDRESS` for the published database, Engine, and frontend ports.
Changing it to a LAN address exposes all three, not just the browser entrance. Keep internal ports
private and verify listener/firewall behavior; dedicated classroom ingress configuration is pending.

### Optional persistence warning tuning

The following variables are optional and only affect diagnostics tuning for high-throughput event classes:

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default: `true`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default: `80`)
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default: `40`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default: `10000`)

## Pre/Post-Class Checklist

Before class:

- Update dependencies and run `npm audit`, `cargo audit`.
- Verify random secrets and loopback binding.
- Snapshot or backup lab VMs.
- Run through all labs in dry-run mode.
- Ensure host is not running sensitive production workloads.

After class:

- Detach all eBPF attachments.
- Stop the lab stack.
- Clean temporary accounts/credentials.
- Check for stale files under bpffs.
- Prefer destroying and recreating VMs in shared environments instead of reusing unknown states.

## Dependency Audit Policy

- Unreleased frontend dependency floors are Next.js 15.5.28, DOMPurify 3.4.17, sharp 0.35.5 and source-map-js 1.2.2.
  The 2026-10-23 follow-up CI audit reports two affected packages, Next.js and DOMPurify.
  Next.js 15.5.28 addresses [SVG image optimization](https://github.com/vercel/next.js/security/advisories/GHSA-2464-fhr4-w9xw)
  and [Server Actions denial of service](https://github.com/vercel/next.js/security/advisories/GHSA-p3m5-59mp-jv5r);
  the existing sharp 0.35.5 floor is retained. DOMPurify 3.4.17 addresses
  [shadow-root template expressions](https://github.com/cure53/DOMPurify/security/advisories/GHSA-cg2g-xh64-v4q7),
  [detached parent/sibling hooks](https://github.com/cure53/DOMPurify/security/advisories/GHSA-mrv2-3pqr-j5jr)
  and [XHTML SMIL attributes](https://github.com/cure53/DOMPurify/security/advisories/GHSA-c6gr-qvv4-5v3f).
  Audit severities may differ between npm and maintainer records; no threshold or exception is changed.
  The 2026-10-08 Unreleased gate found one moderate dependency finding in Next.js 15.5.24, covering
  the upstream [SSG/ISR page cache poisoning](https://github.com/vercel/next.js/security/advisories/GHSA-4jqv-mc3x-m676)
  and [catch-all route cache poisoning](https://github.com/vercel/next.js/security/advisories/GHSA-mcj8-r9mp-w47p)
  advisories; the Next.js floor is raised to the patched 15.5.27 release, included in source version 0.5.4.
  This dependency finding and patch do not establish that a deployment was exploitable, that deployed
  instances have been updated,
  or that deployment-specific exploit regression testing has passed.
  The Next.js/sharp floors retain fixes for the upstream
  [Windows server](https://github.com/vercel/next.js/security/advisories/GHSA-p293-qw3h-jr36),
  [AVIF optimization](https://github.com/vercel/next.js/security/advisories/GHSA-2xp9-vwfh-vxw4) and
  [sharp/libheif](https://github.com/lovell/sharp/security/advisories/GHSA-rgj7-g3m4-5g8c) advisories.
  The sharp update also addresses the [bundled librsvg issue](https://github.com/advisories/GHSA-wq5f-xc86-pv6w),
  and source-map-js addresses [indexed-map amplification](https://github.com/advisories/GHSA-68fv-2mgg-jv7q).
  Offline checks require the supported Next.js 15.x minimum and patched DOMPurify/sharp/source-map-js
  overrides and all locked copies,
  including cross-platform sharp 0.35.5/libvips 1.3.4 packages; live audit remains a separate check.
  Rebuild and redeploy to apply updated dependencies; editing the lockfile does not update a running
  container. A custom global libheif must also be at least 1.23.2; an npm audit is not host-library acceptance.
  The published 0.5.2 check on 2026-10-07 retained one low DOMPurify finding and no moderate/high/critical
  findings; that historical result is unchanged. The later 0.5.3 DOMPurify follow-up's fresh production
  audit reports zero findings in every severity category. Neither result proves all dependencies or deployed
  libraries safe, and the audit does not inventory vendor code embedded inside generated assets.
- We run `cargo audit` for Rust backend dependencies and track accepted exceptions in
  `scripts/security-audit-exceptions.json`.
- The Unreleased dependency floors are rustls 0.23.46 and quinn-proto 0.11.18. They address the
  0.6.3 CI findings RUSTSEC-2026-0346 through 0349 (rustls) and 0338 through 0342 (quinn-proto),
  while retaining the [TLS handshake fix](https://rustsec.org/advisories/RUSTSEC-2026-0285.html).
  The [quinn-proto 0.11.18 floor](https://rustsec.org/advisories/RUSTSEC-2026-0338.html) is necessary;
  0.11.17 does not fix all five findings. QUIC is not enabled by the current reqwest features, but its
  optional locked dependencies are still audited. No audit exception or TLS policy relaxation is added.
  A fresh audit also requires [rustls-webpki 0.103.16](https://rustsec.org/advisories/RUSTSEC-2026-0382.html)
  and [webpki-roots 1.0.10](https://rustsec.org/advisories/RUSTSEC-2026-0381.html) to reject distrusted
  emSign certificate paths and remove the affected bundled root.
  Rebuild and redeploy Engine, platform host and Runner Agent binaries/images; changing the lockfile
  alone does not patch a running process or published 0.6.3 artifacts. Offline preflight checks these
  known version floors; live audit checks for additional advisories.
- There are currently no accepted advisory exceptions. The former `rsa`/`sqlx-mysql` dependency
  chain is no longer in the lockfile.
- Each accepted advisory has a review deadline and must be re-evaluated before it expires.

### DOMPurify and Monaco asset hardening in 0.5.3

The 2026-10-07 follow-up raises DOMPurify to 3.4.16, the upstream fix for
[detached hook subtrees](https://github.com/cure53/DOMPurify/security/advisories/GHSA-p98j-92pf-mc4p) and
[force-removed rawtext roots](https://github.com/cure53/DOMPurify/security/advisories/GHSA-6688-9rhm-gjv2)
in `IN_PLACE` mode. This follow-up is included in source release 0.5.3, not the earlier 0.5.2 release.
The production audit threshold is unchanged, and no advisory exception is added.

Monaco 0.55.1 embeds a separate DOMPurify 3.2.7 copy in its AMD assets; changing the npm override alone
does not replace that copy. Asset synchronization checks the exact supported Monaco version, original
chunk SHA-256 and JavaScript syntax-tree shape before replacing only the vendor block with the official
DOMPurify 3.4.16 ESM implementation inside an isolated IIFE. A new content-derived chunk filename and
rewritten AMD references avoid reusing the old chunk URL. Unknown versions, digests or shapes fail the
operation instead of falling back to the old sanitizer. Only generated assets change: `node_modules`
and the Monaco package version are not modified.

Validation, copying and patch writes finish in a private temporary directory outside `public` before
replacing the generated asset directory. Preparation failures preserve existing public assets; the
final removal-and-rename publication is not an atomic transaction.

The frontend Docker recipe copies verified postinstall `public/monaco` assets from the dependency stage
after copying the checkout. Its build context excludes the host's `public/monaco`: Docker directory
copies merge, so copying fresh assets alone would not remove stale host chunks. A common source guard
checks this input boundary; it does not establish that an image was built or deployed.

Default tooling tests cover the replacement contract. Nine cases in the explicit
[browser fixture](testing-guide.md) instrument the actual generated AMD chunk to capture the sanitizer
instance used by Monaco. They check sanitization, hook-detached subtrees, rawtext-root refusal,
healthy-root identity and the real hover consumer. A tenth case uses the unmodified AMD loader and
editor entry to resolve the new chunk and language-registration graph from intercepted local files.
The fixture starts no Next or Engine service and makes no external network contact; it does not prove
an application exploit or validate a deployed frontend. Rebuild and redeploy generated assets to apply
the change; a clean dependency audit alone does not replace those steps.

### Unreleased frontend audit follow-up

The current asset contract requires DOMPurify 3.4.17 while retaining the original Monaco 0.55.1
chunk digest, syntax checks and exact reference inventory. Regeneration replaces the generated
sanitizer chunk and all AMD references together, with a new content-derived URL; changing the
override alone leaves previously generated or deployed assets unchanged. The 0.5.3 evidence above
remains historical. The explicit browser fixture additionally exercises shadow-root template
reassembly, hooks removing a parent or unvisited sibling, and XHTML SMIL attributes, with safe
content controls. These isolated sanitizer checks do not establish Next.js exploit resistance or
deployed-environment acceptance. Rebuild and redeploy the frontend to apply both package fixes.

## Security Incident Response

If you suspect credential leakage or unknown programs:

1. Immediately disconnect lab node network.
2. Stop Engine.
3. Do not keep using the same password after only an in-app change.
4. Export required logs and attachment details.
5. Destroy single-use VM and rebuild.
6. Rotate DB, admin, TOTP, and SSH credentials.
7. Check host workloads for secondary impact.
