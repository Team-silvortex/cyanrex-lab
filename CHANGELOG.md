# Changelog

All notable changes to Cyanrex Lab are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases use semantic versioning.

## [Unreleased]

## [0.6.2] - 2026-10-16

This patch source release connects existing Session operations to retained platform storage, adds
strict format 3 marker identity declarations, and separates startup, HTTP and local Task controller
responsibilities. Public contracts remain compatible; the platform host, Session issuer and browser
saving remain unmounted. See [local release validation](reports/releases/0.6.2/README.md).

### Added

- Add complete trusted five-marker identity declarations and strict private input format 3. Compare
  all original relation OIDs/owners before any marker-row read, reuse the early check for retained
  observations, and keep format 1/2 diagnostics compatible without adopting or exporting a baseline.

- Connect retained platform storage to existing Session query/logout and the explicit Session HTTP
  adapter without a pool or issuer. Verify original catalog/markers inside the same business
  transaction, reuse Source pins and final fresh reads, and clear the dedicated Cookie only after
  confirmed logout. Preserve the old pooled API and leave host/CLI mounting and browser saving separate.

### Changed

- Replace independent optional startup declarations with one private storage-check mode, centralize
  strict missing/present parsing and exhaustively dispatch diagnostics. Move the shared pure Origin
  policy out of HTTP so host configuration no longer depends on a transport adapter. Preserve public
  APIs, input formats, transaction boundaries, deadlines and explicit connection/root teardown.

- Centralize pure HTTP Origin/capacity validation across seven state families and file distribution.
  Keep Origin-first errors, both distribution capacities checked before workspace matching, and each
  state's independent clone-shared admission budget and response-body lifetime unchanged.

- Share bounded raw file-query and canonical decimal parsing between manifest and chunk adapters.
  Keep their exact six/seven-field inventories, typed identities, geometry checks, trusted scope
  projection and separate JSON/binary response ownership; do not decode or normalize selectors.

- Separate the local Task draft controller from rendering and file-picker handles. Preserve reviewed
  confirmations, revision/generation guards, atomic imports and controlled-editor ownership without
  changing draft JSON, layout, persistence or network behavior.

## [0.6.1] - 2026-10-15

This patch source release adds explicit low-privilege platform startup checks, strict private
configuration, original catalog declarations and retained storage connections, and removes an
unnecessary credential-read privilege from prepared Session relation locking. The platform host,
public Session issuer and browser saving remain unmounted; no existing storage is migrated.
See [local release validation](reports/releases/0.6.1/README.md).

### Added

- Add a retained storage connector composing caller-selected original catalog, marker and Artifact-root
  bindings. Recheck roots before and after each fresh connection's bounded catalog/marker verification,
  refuse mixed or replaced targets, and close failed unpublished connections without adopting a baseline,
  adding a pool, mounting a host or changing the diagnostic CLI.

- Add complete trusted catalog identity declarations and strict private input format 2. Reject
  changed role/database-owner/namespace OIDs before first connection adoption, preserve format-1
  diagnostics, and reuse single-attempt bounded inspection/close without pools or host startup.

- Add an explicit `check-storage` command composing low-privilege private input with existing
  Artifact-root, physical-catalog and five-marker read-only checks. Bound one database attempt
  through close, recheck the original root after runtime teardown, and return no retained baseline,
  authorization, installer or running host.

- Add a check-only private platform configuration command behind the Linux single-main-thread gate.
  Require bounded owned no-follow input and strict named JSON records without database connections,
  environment defaults, secret reporting, first-target adoption or host startup.

- Add an explicit local single-attempt database connector, verifying every fresh connection against
  the original catalog before handoff. Reject ambient startup options, override active connection
  defaults and bound dialing/verification without pools, retry, peer-authentication claims or host mounting.

- Add explicit Linux pre-runtime process preparation and `cyanrex-platform check-process`.
  Verify exact numeric identities/groups, saved/filesystem identities and zero current capabilities;
  restrict and recheck no-new-privileges before workers. Keep generic errors and real child-only
  tests without privilege repair, a sandbox claim, Engine construction or a host listener.

### Fixed

- Remove the table-wide credential-read dependency from prepared Source-only Session relation
  locking. Acquire and verify transaction-held relation locks without planning/executing a probe;
  preserve original-source checks and fail closed on reserved-plan collisions. Add real restricted-role
  lifecycle/ACL/lock/trap regressions without granting production privileges or mounting a host.

## [0.6.0] - 2026-10-15

This minor source release improves Task, Settings and Events layouts and adds independent platform
Session lifecycle, bounded Session stock and read-only host preparation gates under proposal #3.
The teaching runtime and public API compatibility remain unchanged. The platform host, Session
issuer and generic browser saving remain unmounted; storage markers do not establish complete
schema compatibility or deployment health. See [local release validation](reports/releases/0.6.0/README.md).

### Added

- Add independent read-only observation of five storage version/scope markers under the original
  host catalog binding. Lock candidate markers before the observation snapshot, reject unsafe
  metadata and replacements, and keep credentials, business bodies and authority history unread.
  Marker acceptance is not complete schema compatibility, authorization or live host composition.

- Add independent read-only Artifact root preflight using the original private directory walk.
  Retain the original descriptor, exact host configuration/path and security metadata, and reject
  replaced or unsafe roots without content reads, permission repair or runtime capability handoff.

- Add explicit trusted platform host configuration and catalog-only database preflight. Borrow one
  caller-owned connection for bounded read-only inspection, pin physical database and namespace
  identities, and reject unsafe runtime roles without installing or repairing storage. Keep the
  library unmounted; empty namespaces do not establish schema, authority or content health.

- Add explicitly configured per-account/source Session stock limits to a separate trusted login
  entry point. Count expired and invalid stored rows without implicit deletion, retain original
  atomic OTP/source guards, and recheck stock after immediate/deferred writes under a fresh writer
  snapshot. Preserve old prepared login and teaching auth; no public issuer or host is connected.

- Add an explicit, unmounted platform Session lifecycle adapter: query the current durable Session
  and clear the dedicated host-only Cookie only after original-source logout is confirmed. Retain
  strict browser admission, private failure outcomes and response-lifetime capacity. Add disposable
  SQL fault/wait/cancellation regressions and bilingual boundaries under proposal #3; no issuer,
  normal-app mounting, teaching-auth switch, storage migration or browser Task saving is connected.

### Changed

- Compact local Task/editor controls, bound mobile content lists and move retained capability
  explanations below the source. Distinguish destructive actions and keep the mobile navigation
  close control outside its scrolling body without changing drafts, confirmations or authority.

- Separate settings, performance, Runner, AI metadata and documentation cards. Keep save/reload
  beside their fields and metric refresh in its own card; improve narrow-screen controls without
  unmounting drafts, changing request scopes or bypassing reviewed writes.

- Group event filters and refresh/export controls, separate destructive deletion and enlarge
  narrow-screen controls. Bound long payload display without truncating data or changing the
  original filter, download, reviewed deletion or account-wide read acknowledgement behavior.

## [0.5.9] - 2026-10-13

This compatible patch adds reviewed local multi-file text authoring, complete owned UI translation
with Korean as the fifth locale, bilingual public documentation and architecture/error-boundary
cleanup. Source/HTTPS verification strengthens the existing explicit file-delivery test chain;
normal-app distribution, server-backed task saving, Session issuance and peer/resume deployment
remain unconnected. Earlier dated evidence retains its original scope. See
[local release validation](reports/releases/0.5.9/README.md).

### Added

- Add Korean (`ko`, 한국어) as a fifth UI locale with all 865 owned entries, including local editor
  explanations and safety confirmations. Persist browser selection and label the actual fallback
  course language without changing source, task revisions, undo, permissions or wire contracts.

- Complete supported UI dictionary parity, localize editor explanations/accessibility and known
  legacy teaching/runtime messages without changing code, diagnostics identity or wire contracts.
  Add English architecture and Chinese public guides with translation/documentation regressions.

- Add reviewed multi-file UTF-8 append to local Task drafts with one atomic commit, existing
  selection/model/undo preservation, bounded validation and stale-read rejection. No server
  saving, execution, Session issuance or large-file service mounting is connected.

- Add explicit disposable Source/HTTPS integration coverage for controlled file delivery. Original
  Session/grant authorization, Published bytes and paired routes supply native TLS responses;
  verify complete selected saving, pre-dispatch revocation and actual TLS-body cancellation.
  This adds test-only transport framing, not a production listener, trust policy, issuer or mounting.

### Changed

- Organize the service index by existing responsibilities and add a concise bilingual current
  architecture map. Share private file metadata-error projection between manifest and chunk
  adapters without changing response status, error outcome, permissions or runtime mounting.

### Fixed

- Reject obsolete course bodies after language changes so a late response cannot overwrite the
  current document or its actual language label. Complete existing browser fixture responses
  without relaxing source preservation, confirmation, write or unexpected-request assertions.

## [0.5.8] - 2026-10-13

This compatible patch connects exact authorized metadata and ordered chunk fetching to private
whole-file assembly and explicit host-selected saving. Owned-directory targets, bounded local
progress and cooperative cancellation retain real-job resource ownership. A single-Source router
composition pairs the unmounted manifest/chunk endpoints, with separate HTTP budgets and native
HTTPS regression coverage. These are explicit internal entries, not normal-app/UI/SDK mounting,
Session issuance, privileged-process isolation, migration, resume or peer-to-peer deployment.
Dated implementation evidence keeps its original 0.5.7/Unreleased scope; source inclusion does
not establish a running deployment. See [local release validation](reports/releases/0.5.8/README.md).

### Added

- Add explicit unmounted distribution-router composition from one Source and one chunk workspace,
  privately deriving the exact manifest configuration. Reuse original handlers with independent
  metadata/binary budgets and one private fallback; no Session issuer, privilege isolation or live API.
- Add explicit native HTTPS verification of the existing controlled file-delivery chain,
  with disposable certificates, exact peer identity, independent saved-file checks and actual
  TLS-wait cancellation boundaries. No production trust policy, API or live mounting changes.
- Add a one-call cooperative cancellation handle for observed file delivery, with separate
  Cancelled/Delivery results and original input priority, transport and job-held budgets.
  Handle drop is not cancellation; requests do not instantly fence synchronous dispatch,
  kill file work, rollback outputs, retry or clean up. No UI/live mounting or resume is added.
- Add an optional local per-delivery phase/chunk-ACK observer using one latest snapshot slot.
  Preserve the existing call result, authorization and actual job-held budgets; slow/dropped
  readers add no consumption queue, closure is not job cancellation, and full staging is not
  saved output. No persistent progress, receipt recovery, retry, UI or live mounting is added.
- Add an explicit trusted-host user-directory save target for owned 0700/0750/0755 parents,
  pinned original identity/UID/GID/mode and no-follow traversal. Keep private staging/default
  targets strictly 0700, reuse actual checked saving and job-held budgets, and reject writable
  shared parents, overwrite, adoption and repair; no UI, live mounting or resumption.
- Compose exact authorized manifest/chunk download, original private assembly and host-selected
  saving in one explicit client call. Retain one whole-flow lease through actual file jobs after
  cancellation, preserve stage-specific errors and existing entry behavior, without retry,
  overwrite, resume, UI/SDK or normal-app mounting.
- Add trusted-host selected private file saving from the original acknowledged assembly, retaining
  private original descriptors/full-manifest binding and using exclusive checked copy/readback.
  Share real staging-worker ownership through cancellation; preserve uncertain output, never
  overwrite/adopt/delete/retry. No ordinary-directory policy, UI save dialog, resume or live mounting.
- Add exact-version manifest fetching through the existing controlled client transport and a
  separate bounded metadata gate. Strict framed EOF/JSON/full-version checks precede optional
  ordered private download; empty granted downloads still require metadata GET. Preserve old
  supplied-manifest behavior and error variants. No live mounting or UI save/resume.
- Add an independently constructed, unmounted current-recipient manifest JSON GET with strict
  exact-version selectors, private credentials and bounded response-lifetime admission. Reuse
  the existing Source authorization command; metadata never becomes later byte permission.
  No normal-app mounting, Session issuance, live API/SDK/UI or migration; a separate client entry
  connects explicit endpoint fetching without changing this adapter's authority.
- Compose explicit ordered HTTP chunk downloads with same-root private whole-file assembly.
  An independent clone-shared bounded flow slot follows actual save/assembly jobs through
  cancellation; the first failure stops without adoption, cleanup, retry or next-chunk continuation.
  Per-chunk current authorization remains separate from local whole-byte acknowledgement.
  This supplied-manifest entry does not fetch metadata; a separate new entry does. No normal-app
  mounting, user save path, resume or API/SDK/UI.

### Fixed

- Wait for the original staging worker's bounded final release in the observed-delivery cancellation
  regression. A freed whole-flow slot does not imply its enclosing worker has dropped; execute the
  real read-only verifier until admission is available and preserve every other error/byte assertion.
  Production cancellation, admission and file behavior are unchanged.

## [0.5.7] - 2026-10-12

This patch extends centralized file-distribution preparation with independent publisher/recipient
grant ledgers, current-Session management/publication/sharing and exact recipient manifest/byte reads.
An unmounted binary HTTP adapter, supplied-response receiver, controlled native single-chunk client
and private whole-file assembly connect explicit internal steps. Budgets survive actual queued/running
work; uncertain outcomes retain evidence without adoption, cleanup or automatic retry.
These source changes do not mount the normal application, issue platform Sessions, migrate live data,
or implement multi-chunk network scheduling, user-selected final saving, resume or desktop pairing.
Dated implementation records retain their original 0.5.6/Unreleased scope; this is a source release,
not acceptance of a running deployment.

### Added

- Add trusted Unix whole-file assembly from existing legacy private chunks, with fixed manifest-bound
  output, preverification, checked copying, actual original-inode readback and final input/root checks.
  Existing targets conflict; uncertain output is retained without adoption, deletion or retry.
  One staging job retains shared admission through queued/running cancellation. No user-selected
  save path, network orchestration, resume, publication, live API/SDK/UI or migration is added.
- Add an explicit Unix single-chunk HTTP client with canonical HTTPS or opt-in literal loopback,
  normal certificate/SAN checks and bounded trusted CA configuration. Freeze the target and typed
  selectors, use per-call sensitive Session cookies, disable retry/redirect/proxy/decompression,
  and separate header waiting from receiver body deadlines. Request capacity follows verified
  bytes into actual save work through cancellation. No runtime mounting, whole-file save or resume.
- Add a separate Unix receiver for supplied exact binary HTTP responses. Validate singleton
  headers, bounded data, explicit EOF and selected SHA-256 before existing exclusive private
  staging. Shared fail-fast controls and idle/total deadlines cover collection; the real queued
  or running save job owns the verified buffer/control through waiter cancellation. Opaque metadata
  acknowledges only this call's stage success, not a remote receipt, whole-file save or resumption.
  No active request, credentials, listener, live API/SDK/UI, migration or automatic cleanup is added.
- Add an independently constructed, unmounted exact-version binary chunk GET adapter over the
  existing current-recipient source command. Strict private cookie/origin/query/empty-body gates
  precede SQL; fixed headers expose only server-selected version metadata. A zero-copy Bytes owner
  retains both local budgets through data clones/slices, with a ten-second first-frame window and
  no detached timer or poll-time authorization. No listener, live API/SDK/UI, client saving,
  delivery receipt, resumption or whole-file rehash is added.
- Add a current-Session verified single-chunk read with exact independent recipient authority,
  original Published source/root binding, private bounded length/EOF/SHA-256 verification and a
  lock-free file phase between two original-pinned source transactions. Worker and response budgets
  survive cancellation; returned move-only bytes retain response capacity until drop. The final
  same-clock Session/grant check precedes commit and disclosure, with no later file work. This is a
  read-time chunk snapshot, not a fresh whole-file digest, durable receipt, transport or client save.
- Add current-recipient exact Published download-manifest observation in one source transaction.
  Require an independent Active recipient grant, exact publisher/account and full version match,
  original row/history pins and a final shared Session/grant clock sample before commit.
  Filter ungranted publication coordinates before decoding. Returned metadata is not a reusable
  byte permit; no byte command, transport, public mounting or publisher-role delegation is added.
- Add current publisher-owned recipient issuance for an exact trusted Published version and active
  publisher permission, with current recipient checks and final fresh permission/source commit.
  Original publishers may revoke old shares without an active publisher grant or recipient lifecycle.
  All original records and bounded ledger deltas are verified after deferred work; trusted store
  APIs stay independent. No teacher-wide delegation, downloads, bytes, transport or live mounting.
- Add a separate trusted exact-file recipient-grant ledger with independent issuer, publisher and
  recipient account generations, immutable bindings, CAS revocation and bounded complete history.
  Current-Session own-recipient reads retain original pins and selected history through final source
  authorization and commit. Publisher revocation does not cascade. No authenticated recipient
  management, download permission consumption, bytes, transport or live mounting is added.
- Add explicit current-Session file publication over an opaque trusted attempt: exact audited actor
  and active publisher grant, lock-free real-byte verification, original source/Registry/grant pins
  across the gap, and a shared source transaction for insert, deferred checks and final fresh expiry.
  Post-acknowledgement original checks and terminal Unknown remain; no recipient, transport, live
  mounting, retry, cleanup or recovery is added. The old trusted publisher remains separate.
- Add separate current-Session publisher-grant issue/revoke commands with explicit trusted-host
  manager/account-generation configuration, derived actor attribution, target lifecycle checks,
  original source/ledger pins, deferred-flush verification and one acknowledged source transaction.
  No implicit teacher/deployment grant, durable online manager revocation, publication permission
  consumption, recipient access, bytes or live mounting is added.
- Add a separate trusted publisher-grant ledger with immutable issuer/account-generation bindings,
  revision-fenced revocation and bounded audit history. Current-Session reads derive the exact subject
  from audited identity and retain original records through final authorization and commit. This
  metadata preparation does not authenticate grant managers, consume publishing permission, grant
  recipients, access bytes, mount transport or migrate existing data; teacher/deployment authority
  does not imply file permission.

### Fixed

- Poll both queued Session upload-reservation test callers concurrently after releasing their
  real database barrier; sequentially awaiting them could starve the lock-owning caller and cause
  a fixture-induced timeout. Preserve exact winner/quota and unchanged-record checks.

## [0.5.6] - 2026-10-11

This patch includes owner-private task metadata indexing and its separate unmounted HTTP adapter,
plus the centralized large-file distribution preparation from [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2).
The file slices cover bounded streaming, private staging, immutable upload quotas, chunk journaling,
whole-byte checks, trusted publication, current-Session private metadata and pure grant contracts.
They do not enable network transfers, downloads, durable grant issuance, restart recovery or browser
saving. The original source-relation pin is retained across private Session work. Dated implementation
records keep their original versions and test scope; this source release does not update a deployment.

### Added

- Add pure shared publisher/recipient grant contracts with exact workspace, file/version/digest/size
  and account-generation bindings, independent active/revoked state, bounded half-open validity and
  strict 4096-byte JSON admission. Matching supplied snapshots is not current audited authority;
  no grant issuance/storage, Session/HTTP integration, byte access or transport is added.

- Add current-Session owner-private upload reservations and upload/Published metadata reads in one
  authentication-source transaction. Derive owner and account generation from the audited binding,
  retain original source/target identities and exact records, and recheck fresh policy/expiry before
  commit. Selected reads retain row/marker locks; old trusted stores stay independent. Preserve the
  initial source/registry relation pin in the shared private-work guard instead of recapturing it.
  No actual Session publication, publisher/recipient grants, byte access, recovery or transport.

- Add separate trusted create-only whole-file publication metadata and an opaque one-shot publisher.
  Require exact complete Confirmed chunk metadata and fresh real ordered bytes; retain original
  source/journal/publication/file checks across the insert acknowledgement. Bound all publications,
  including empty files, and reject reused upload/version coordinates without adoption or overwrite.
  An acknowledged row can survive a later Unknown caller outcome; metadata grants no current file
  health, retry, recovery or download authority. No live Session, transport, client or startup wiring.

- Add read-only whole-upload byte verification over existing upload-bound chunks, with exact ordered
  chunk and whole-file digests, original source/root snapshots and final file rechecks. Reuse existing
  caller/worker admission without assembling a second file or retaining every chunk descriptor.
  The metadata observation does not adopt historical receipts, consult journal status, publish bytes,
  restore attempts or connect Session authority, transport, clients or deployment.

- Add a separate fresh-only trusted chunk journal and explicit journaled staging wrapper. Commit
  Unknown before file dispatch, bind each unique upload/index to original source/root metadata,
  and confirm only after local byte checks; recheck original source/file after the final journal
  acknowledgement. Compact bounded records never become restored attempts or original-file receipts.
  No old-schema migration, retry, cleanup, Session authority, publication or transport connection.

- Add a standalone Unix upload-bound local staging adapter, binding exact trusted reservations,
  original database/root incarnations and complete manifests to fixed exclusive chunk destinations.
  Caller-owned attempts are consumed before asynchronous validation; opaque local receipts retain
  the original read-only file descriptor and reject substituted entries. Fresh source rechecks use
  READ COMMITTED after lock waits. No durable chunk journal, recovery/retry, Session authority,
  publication, quota release, network transfer or live application/client connection.

- Add a standalone trusted PostgreSQL upload-intent ledger with immutable owner/account-generation
  reservations and persisted logical byte/count limits. Serialize quota admission, validate the whole
  bounded ledger, pin namespace/table identity and verify exact post-write facts; retain reservations
  on uncertain outcomes. No physical disk allocation, confirmed chunks, release/cleanup, resume,
  Session authentication, publication/grants, transport or live application/client connection.

- Add a separate Unix private large-file chunk staging API with full manifest/index fingerprint
  binding, descriptor-relative exclusive creation, 0700 roots, 0400 single-link files, sync and
  fixed-inode readback. Independent clone-shared worker admission belongs to actual queued/running
  jobs even after caller cancellation. Retain uncertain output without overwrite, adoption or cleanup.
  Trusted preparation only: no upload journal, reserved quota, resume, publication/grants, HTTP/client
  connection, live composition, desktop pairing or legacy Artifact/text-limit change.

- Add a transport-neutral large-file manifest and synchronous bounded streaming core following
  [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), centralized distribution first.
  Typed file/version pins, strict bounded metadata, fixed 4 MiB chunks and 64 KiB IO verify actual
  ordered bytes and whole-file integrity; preopened Unix descriptors reject observable changes.
  Copying may leave unverified output and supplies no durable receipt. No staging, publication,
  sharing grants, resume/restart state, live transport/UI/SDK, desktop pairing or legacy limit change.

- Add an explicit unmounted private metadata HTTP index at `/platform/v1/task-index`, invoking
  the existing current-Session index command once after strict raw cursor/limit, cookie, Origin
  and empty-body admission. Serialize only seven metadata fields, with explicit nulls and
  shared response-lifetime capacity; preserve existing manual/catalogue collection contracts.
  No live application, OpenAPI/SDK/browser connection, issuer, content-byte proof or migration.

- Add an independent owner-private schema 3 task-content metadata index and current-Session adapter.
  Bound UUID keyset pages to 1–32 entries plus one strictly checked lookahead; explicit neutral
  catalogue configuration admits manual and complete configured definitions without relaxing existing
  content read/write modes. Return metadata only after current authorization and confirmed commit,
  without Artifact/file access, HTTP/SDK/UI mounting, installation, migration, recovery or retry.

## [0.5.5] - 2026-10-11

This patch separates definition metadata from assessment providers, composes bounded multi-package
catalogues and text content, and preserves portable payload language hints independently of editor
profiles. It adds shared HTTP admission and the independent C2-X/Y adapters, C2-Z catalogue draft
publication and separate C2-AA target observation. These remain internal preparation: no application
mounting, public Session issuer, browser saving, authentication cutover, migration, recovery or automatic
retry is introduced. Source publication does not update running deployments. The dated development
records below describe each slice independently; C2-AA observation follows the create-only C2-Z slice.

### Added

- Add read-only observation to a surviving independent catalogue draft attempt. Original-token and
  unconfirmed-state admission selects one existing current-Session Artifact or catalogue Task reader;
  complete frozen definitions and ordered text distinguish matching or valid changed data from typed
  corruption. Return the existing body-free report without adopting confirmations, changing attempt
  state, claiming historical commit/rollback, enabling retry, converting manual/journal types or
  adding provider execution, HTTP/issuer, public SDK/browser saving, migration or recovery.

- Add a separate opaque catalogue draft publication attempt over the unchanged C2-O plan and typed
  catalogue content workspace. Pure preparation checks token shape/source authority and freezes an
  exact complete definition before random target allocation. Each explicit advance publishes one
  Session Artifact or finally creates a catalogue Draft, retaining the confirmed prefix and stopping
  on uncertainty. Final confirmation requires the full frozen `Some` definition; the manual `None`
  matcher and C2-P–W types/wire remain separate. Share only two pure receipt/manifest checks, without
  checkpoint/journal/observation, restoration, retry, provider assessment, HTTP/issuer, public SDK/
  browser connection, schema migration or whole-draft atomicity.

- Add an independent catalogue Task-content HTTP adapter over the existing typed Session workspace,
  with a separate `/platform/v1/catalog-tasks` prefix. Creation selects only an exact configured
  definition reference through strict object decoding; full metadata, old/new text and current
  authority remain checked without assessment. Preserve the manual state/request contract and reuse
  private security, bounded request/response admission and confirmed-result projections. Unknown
  definitions remain generic dispatched/unconfirmed errors. No application mounting, client mode
  switch, discovery, automatic router merge, Session issuer, publication/journal integration,
  live OpenAPI/SDK/browser connection or migration is added.

- Allow explicit definitions without automatic assessment bindings alongside bound entries in frozen
  metadata catalogues. `TaskDefinition` uses paired optional evidence-schema/policy fields; both JSON
  fields remain required, with two explicit nulls for absence and rejection of missing/half-bound input.
  Existing bound JSON encoding is unchanged, while internal Rust callers must adapt and old binaries
  reject new unbound records. Catalogue Tasks retain their definition and full-equality drift checks;
  typed-provider construction/dispatch and `TaskAssessment` still require both pins. No schema-number
  change, record migration, authorization relaxation, public HTTP/editor connection or Review/execution
  workflow is added.

- Add trusted explicit multi-package `TaskDefinitionCatalog::compose`: 1–256 frozen metadata handles
  produce at most 256 flattened definitions, with the same global bound for nested composition.
  Preserve input/entry order, admit disjoint same-package registrations and distinct exact versions,
  and reject every repeated complete reference even when metadata is identical. Existing metadata-only
  Task draft and schema 2/3 workspace constructors accept the result; typed single-package evaluators,
  exact-definition checks and authorization remain unchanged. No discovery, public HTTP/browser save,
  publication wiring, schema migration or rule execution is added.

- Compose catalogue definitions with Task text content through explicit metadata-only store and
  Session workspace constructors. Catalogue creation admits an exact full definition; reads, edits
  and transitions reject manual records, missing entries or metadata drift, while the old manual
  entry points remain definition-free. Reuse schema 3 and the source-owned content transaction,
  including old/new byte checks and final authorization. No rule execution, schema migration,
  catalogue HTTP/browser save or draft-publication wiring is added.

- Separate frozen `TaskDefinitionCatalog` metadata from provider-bound `TaskCatalog<P>` assessment.
  Task drafts and Session catalogue workspaces can use validated definitions without an evaluator;
  existing constructors delegate compatibly, and exact pins, current authorization and transactions
  remain unchanged. Local task drafts also accept bounded portable language hints independently of
  the 14 editor profiles: unknown valid hints are preserved and visibly edited as plain text; capability
  resolution itself changes no metadata/revisions or undo history and loads no tools. No schema,
  route, public API, payload kind, browser saving or LSP connection is added.

- Add the separately constructed, unmounted C2-X text Artifact HTTP adapter over current-Session
  publication and exact-revision reads. Fix the text kind/media type; independently bound JSON wire
  input to 2 MiB and decoded text to 256 KiB. State clones share response-body-lifetime admission,
  while separate text/Task states retain additive budgets. Returned references can be used in a
  caller's separate Task request, not an atomic batch or automatic router composition. No public
  mounting, Session issuer, SDK/browser saving, journal dispatch, adoption or recovery is added.

- Add shared in-flight admission to the explicit, unmounted C2-N content HTTP adapter: four slots
  by default, trusted configuration from 1 to 16, shared by state clones and routers built from them.
  Excess requests receive private 503 `capacity_exhausted` / `not_attempted` before body polling or SQL.
  Admitted slots cover body collection, the unchanged Session command, serialization and successful
  or error response bodies through completion/drop. Cancellation is not rollback or termination of
  dispatched blocking I/O. No global process/worker quota, retry, listener, public/browser wiring,
  Session issuer, schema or journal change is added.

## [0.5.4] - 2026-10-08

This patch adds internal immutable draft intents, opt-in journaled dispatch, journal inspection and
journal-bound resource observation (C2-T–W), and raises Next.js to 15.5.27. These remain explicit
preparation APIs: no public/browser saving, live-authentication cutover, schema migration, restored
attempt or automatic retry is introduced. Source publication does not update running deployments.

### Added

- Add C2-W internal observation of one recorded journal step through the exact current owner/account
  in one source-owned transaction. Capture the canonical intent, full prefix and first relation pins;
  read the exact Artifact revision or current Task contents, then recheck the original records/pins
  and fresh authorization before returning body-free recorded status and metadata agreement.
  This reads resources/files without business writes; C2-V remains journal-only. No receipt, resume,
  retry, cleanup, schema change, public/browser API or live-authentication cutover is added.

- Add C2-V internal, current-Session inspection of schema-2 intent and step records, filtered by exact
  owner/account generation and trusted workspace configuration. Validate the full committed prefix
  and optional final Unknown step, recheck relation pins and fresh authorization, and expose counts
  including the final Task step without nonces. No resource/file read, checkpoint mutation, recovery,
  retry, adoption, cleanup, public/browser API or schema change is added.

- Add C2-U opt-in journaled dispatch by consuming the real, pristine draft attempt. A fresh schema-2
  journal commits a unique step reservation before resource work; a separately authorized transaction
  rebinds owner/account/source, verifies the full prefix and table pins, and commits the step marker
  with resource SQL. Shared C2-M verification remains authoritative. Failures/cancellation stop the
  live wrapper; uncertain COMMIT acknowledgements do not prove rollback or authorize retry. No
  schema-1 migration, restored attempt, retry, public/browser API or live-authentication change is added.

- Add C2-T internal immutable intent registration from a real, pristine draft attempt and separately
  current-Session-authorized reading, bound to the exact owner and account generation. An explicitly
  installed, separate schema-1 journal stores bounded canonical metadata, not text or credentials;
  duplicates conflict and results require final authorization checks and confirmed database commit.
  This does not gate C2-P dispatch, attest resource incarnations, restore attempts, retry writes,
  mount public/browser APIs, change live authentication or migrate existing data.

### Fixed

- Raise the Next.js dependency floor and lock to 15.5.27 for GHSA-4jqv-mc3x-m676 and
  GHSA-mcj8-r9mp-w47p, with offline manifest/all-copy checks. The fresh production audit found these
  after the earlier 0.5.3 checks; historical audit results remain unchanged. This source update does
  not redeploy or establish deployment-specific exploit acceptance.

## [0.5.3] - 2026-10-07

This patch adds AI Agent host configuration and SDK tool adapters, internal draft publication and
inspection preparation, shared boundary mechanisms and sanitizer hardening. Provider calls, autonomous
AI delegation, generic Task browser saving, live-auth cutover and database migration remain separate.

### Added

- Add teacher-managed, revision-fenced AI Agent connection profiles and a four-language Settings
  panel, storing metadata and symbolic host credential references only. Add SDK helpers for OpenAI
  Responses/Chat, Anthropic, Gemini, MCP and custom tool formats over an explicit reviewed catalogue;
  writes require per-call trusted approval. No provider calls, secret vault, AI identity/delegation,
  autonomous orchestration, generic Task browser saving or signed Runner protocol change is added.

- Add C2-S explicit current-Session inspection of one reported unconfirmed checkpoint target through
  a trusted workspace and existing authorized readers. Return a body-free metadata comparison while
  preserving typed failures; parsing still performs no read and C2-Q still needs its original attempt.
  No original-caller/byte provenance, storage-incarnation proof, confirmation, journal, restored attempt,
  retry, cleanup or public/browser connection is added.

- Add C2-R bounded metadata checkpoints for caller-owned draft publication attempts: strict 64 KiB
  export/parse of allocated references and reported progress, without text or authentication material.
  Imported progress remains forgeable/replayable caller data, not confirmed provenance or authority.
  No save, durable journal, attempt restoration, read-on-import, retry or public/browser route is added.

- Add C2-Q explicit read-only observation of a surviving draft attempt's unconfirmed target through
  the original Session and existing authorized read commands. Report current match, difference or
  invisibility without body data or state changes; preserve typed read failures. A match is not prior
  commit provenance, and invisibility is not rollback or permission to retry. No durable journal,
  new-login recovery, cleanup, public route or browser connection is added.

- Add C2-P internal create-only draft publication steps over existing Session-authorized Artifact and
  Task-content commands. Caller-owned state fixes identities and Session context, marks each write
  unconfirmed before waiting, and retains confirmed progress when later work fails or is cancelled.
  Each advance dispatches at most one write; there is no whole-draft transaction, automatic retry,
  content deletion, durable receipt/recovery, public route, browser saving or live-authentication change.

- Add C2-O pure Rust task-draft import, bounded text-publication planning and exact supplied-content
  binding to the existing Task manifest. Reject blank save titles, malformed/ambiguous JSON and local
  identity misuse; preserve text, labels and ordering without granting editor or execution capability.
  Shared canonical-export fixtures check browser/Rust compatibility. This preparation performs no
  publication, Task write, Session issuance, HTTP mounting or browser save; supplied snapshots do not
  establish persistence or authority, and later publication/Task writes remain separate operations.

### Changed

- Consolidate private frontend fetch/cancellation/deadline mechanics in a neutral transport module.
  Runner administration and performance metrics no longer borrow the Settings transport; feature
  decoders retain their JSON, HTTP error, export and acknowledgement policies and existing timeouts.
- Share Task, Artifact and Review transaction setup, namespace and ordinary-table checks internally.
  Keep each store's table locks, schema/scope checks, error mapping and commit ownership, including
  the separate Task storage formats and Session adapters; add exact opt-in SQL fixtures to CI.

### Fixed

- Event and Runtime browser waits now end at their cancellation/deadline even when fetch or body
  decoding ignores abort. Late results cannot acknowledge the completed wait; no automatic retry,
  server rollback, execution cancellation or authorization change is implied.

- Raise DOMPurify's security floor to 3.4.16 for GHSA-p98j-92pf-mc4p and GHSA-6688-9rhm-gjv2.
  Monaco 0.55.1 embeds an older sanitizer outside npm's resolved dependency graph, so asset synchronization
  also replaces that verified vendor block with the official patched ESM implementation. Exact version,
  checksum and syntax checks reject unknown inputs; a new content-addressed chunk and rewritten AMD
  references avoid retaining the old sanitizer URL. Add offline dependency floors, asset-tool regressions
  and opt-in browser checks of the actual Monaco instance and hover consumer. No CDN fallback, editor
  version upgrade, public API or database change is added; running deployments still need a rebuild.

## [0.5.2] - 2026-10-07

This patch hardens prepared authentication and registry reads, with explicit bounded Session
maintenance. Auth source schema 2, registry schemas and public API compatibility baseline 0.3.0
remain unchanged. No automatic maintenance, public issuer, live-auth cutover or data migration is added.

### Added

- Add explicit trusted maintenance for at most 128 expired prepared Sessions at one database cutoff.
  Full-batch validation, exact deletion readback, original-source pins and selected-digest absence checks
  precede confirmed commit. The returned count describes only that batch, not global emptiness or a
  storage quota. Corrupt/orphaned candidates fail without repair; uncertain commit outcomes do not permit
  blind retry. No automatic schedule, CLI/HTTP entry, login/read cleanup or schema change is added.

### Fixed

- Raise frontend security floors to sharp 0.35.5 and source-map-js 1.2.2, including the matching
  cross-platform sharp/libvips packages. This addresses GHSA-wq5f-xc86-pv6w and GHSA-68fv-2mgg-jv7q;
  offline lock/override guards retain those minimums alongside the live production dependency audit.
  Unrelated dependency resolutions, public API and database schemas remain unchanged.

- Guard prepared registry retirement and identity/policy audit times before SQLx chrono decoding.
  Invalid non-null retirement stays rejected, never becoming an active binding through a projected
  NULL. Audit heads, history, replay and INSERT readback reject unrepresentable times; failed writes
  roll back binding/principal or membership/grant/receipt changes. Read-only reconciliation retains
  stage-specific errors. Legitimate NULL retirement and finite boundary dates remain compatible,
  without filtering corrupt rows, changing sequence policy, upgrading dependencies or migrating data.

- Reject infinite or chrono-unrepresentable prepared Session expiry before SQLx decoding. Shared
  Session reads return `InvalidRecord`; source reconciliation computes checked expiry in SQL and
  returns `InvalidSource` for corruption. Preserve normal expiry, source locks and snapshot cutoffs,
  with rollback coverage for malformed login INSERT readback. Registry retirement/audit time decoding
  is covered separately above; no dependency, schema or live-authentication change is included.

- Route genuinely missing prepared-login accounts through one synthetic password verification with
  the existing Argon2id profile and shared bounded worker gate, after confirming the lookup transaction.
  The original real account is still required independently; synthetic matches and concurrent same-name
  registration cannot issue a Session or consume an OTP. Corrupt records and source/storage errors retain
  their failures. This removes a missing-account KDF shortcut, not all timing or enumeration differences.
  The change is included in 0.5.2 with no public issuer, live-authentication, dependency or schema change.

## [0.5.1] - 2026-10-04

This patch hardens the explicit collaboration authentication preparation layer. Auth source schema 2
is fresh-install only: schema 1 is rejected unchanged, not upgraded. Legacy live authentication and
the public API compatibility baseline 0.3.0 remain unchanged; this is not a deployment or public issuer.

### Added

- Atomically consume prepared login/password-rotation OTP counters with Session issuance or credential
  replacement and revocation. Collision-aware selection, account-bound compare-and-update and final
  exact-counter rechecks prevent committed step reuse across independent sources. Fresh explicit auth
  installation/bootstrap now uses schema 2; existing schema 1 is rejected unchanged, without migration.
  This does not switch legacy live authentication or add public issuance/recovery.

### Fixed

- Recheck prepared login/password-rotation TOTP validity after writer waits and after the final SQL
  readback, before requesting commit. Expired codes fail without publishing a token or confirming a
  password change. Freshness alone is not one-time consumption; the atomic follow-up above adds that
  separate prepared-source boundary, not a live-auth cutover or commit-acknowledgement freshness promise.
- Validate prepared stored password hashes before worker admission and pin both hashing and verification
  to the existing Argon2id v19, m=19456/t=2/p=1, 32-byte profile. Reject malformed, duplicate or unsupported
  parameters as invalid records, without migrating credentials, revoking existing Sessions or changing
  legacy live authentication. This bounds accepted computation parameters, not wall-clock time or process RSS.
- Bound prepared password work across registration, login, password rotation and authority bootstrap
  with one in-process limit of four dispatched jobs and twenty admitted jobs in total. Blocking
  work retains its slots after caller cancellation or timeout until it actually finishes. Excess
  admission is rejected; legacy live authentication, password formats and public routing are unchanged.
- Pin source namespace and auth-table identities for prepared durable Session login, validation and
  logout. Recheck schema and authority before confirming results, retain the original pins across
  password verification, and check expiration after guard waits. A redirected, suppressed deletion
  can no longer acknowledge logout against an empty shadow table while the real Session survives.
  These source-only guards do not require a registry or enable public Session issuance/live cutover.

## [0.5.0] - 2026-10-03

This source release advances the domain-neutral task-content foundation. The existing teaching
runtime and local editor remain supported; the new HTTP adapter is not mounted in the application.
It does not enable public content publication, Session issuance, browser saving, migration or
cross-user collaboration. The frozen public API compatibility baseline remains 0.3.0.

### Added

- Added C2-N explicitly constructed Task-content HTTP routing over the current-Session transaction.
  Dedicated cookies, exact Origin admission, bounded strict JSON, ordered text reads and private
  versioned errors preserve the storage/authorization boundary. Normal startup, legacy authentication,
  live OpenAPI/SDK and browser saving remain unchanged; no login issuer or content upload is added.
- Added a bilingual architecture/capability/implementation/maturity tensor, a machine-readable
  inventory of 57 implementation coordinates and 73 explicit edges, and reference/score/view
  consistency checks. Independent implementation, connection, verification and operations scores
  retain dated evidence and missing boundaries instead of implying an aggregate completion rate.
- Added C2-K Task content metadata contracts and pure snapshot/byte checks: bounded title and text
  display labels, extensible language hints, exact ordered Artifact pins, owner consistency, UTF-8,
  byte limits and recomputed digests. Strict object decoding rejects alternate array/enum shapes.
  This does not persist metadata, authorize a Session, add public routes or connect browser saving;
  existing Task/Artifact storage formats and public APIs remain unchanged.
- Added C2-L separate schema-3 Task content storage with atomic title, filename/language and ordered
  payload edits, complete Task/manifest outbox records and revision-fenced lifecycle transitions.
  Legacy schema-2 handles remain fixed and mutually reject the new namespace; no automatic migration.
  Added exact PostgreSQL CI selection for corruption, commit and concurrency boundaries. This trusted
  store remains a trusted primitive, not authorization or browser saving.
- Added C2-M internal Session-authorized schema-3 content commands. One source-owned transaction
  checks old/new exact Artifact text, metadata, namespace identity and current authorization around
  atomic edits. Metadata-only changes and removal cannot bypass old-content validation; legacy
  schema-2 adapters stay separate. C2-M itself adds no HTTP boundary; the separate C2-N adapter above
  remains unmounted, with no browser save or live-auth cutover.

### Fixed

- Preserve local task drafts during same-page fragment navigation. Sidebar authentication and logout
  lifetimes now follow the pathname and query rather than the fragment; actual route changes still
  recheck identity and permissions, and login return paths retain their fragment.
- Keep truncated text-import and download filenames Unicode-safe. A supplementary character at the
  128-code-unit boundary no longer becomes an invalid surrogate that rejects otherwise valid content.
  Filename bounds, content validation and explicit import confirmation remain unchanged.

## [0.4.9] - 2026-10-03

### Added

- Added task-owned text payload editing at `/tasks/new`, with `/editor` as a compatibility entry.
  Empty tasks do not invent code; up to 32 optional contents retain their filename, language and text
  when switching editors. Controlled edits carry local revisions, confirmations bind reviewed targets,
  and whole-task replacement invalidates old editor generations, even with matching imported IDs.
  Strict local JSON import/export is capped at 8 MiB and each text item at 256 KiB. This local format
  cannot import server ownership or Artifact references; browser integration with server saving and
  Task-input rebinding remains pending. Downloads never claim persistence or submission.
- Added 14 built-in language profiles for task text contents: richer TypeScript and
  JavaScript services, structured JSON/CSS services, HTML assistance without diagnostics, eight
  highlighting/snippet profiles and plain text. Local Monaco assets have no CDN fallback; no external
  LSP, schema/package acquisition, Engine source request or execution is added. Individual contents
  support confirmed strict UTF-8 import and explicit download, not server saving, autosave or recovery.
  Language changes retain text in a new model but discard old undo history;
  shared JS/TS workers are not a security sandbox. Existing C/eBPF completion now rejects non-C models
  and invalidates pending work/cache on language changes, including same-model round trips. See the
  [editor guide](docs/en/editor.md) for capabilities and navigation limitations.
- Added C2-J Session-authorized Draft input replacement for manual and catalogue tasks. Expected
  revisions prevent lost updates; old and new exact owned inputs are checked before and after the
  atomic Task/outbox write, followed by the final Session check. A separate `inputs_replaced` event
  requires Task storage schema 2 on fresh installation; schema 1 is rejected without migration.
  Artifact publication remains separate, with no file deletion or automatic retry after failure.
  No browser save adapter, public HTTP API, live cutover or Task acceptance is added.
- Added C2-I Session-authorized catalogue tasks: trusted configuration copies full definition snapshots
  without retaining or executing a provider. Private tasks accept 0–32 exact owned inputs; even zero
  inputs require pinned Artifact namespace/schema metadata. Reads and transitions require complete
  definition equality, rejecting unknown versions and changed metadata under the same pin without
  fallback. The shared Task-input transaction retains namespace, post-write and final Session checks;
  C2-E/G manual adapters remain strict. Three default and sixteen explicit PostgreSQL cases cover the
  boundary. This is metadata admission, not typed evidence validation, policy execution or acceptance;
  no live cutover, legacy-writer fence or public API/SDK contract change is added.
- Added C2-H Session-authorized private human Reviews: current audited membership, exact owned
  Artifact targets/evidence, immutable Review history and outbox writes share one source transaction.
  Review heads precede globally ordered Artifact locks; content and pending records are rechecked
  after writes, then final Session checks and confirmed commit precede publication. The configured
  policy remains a label, not evidence of policy execution or historical Session authorship; no
  cross-user review authority, Task acceptance, live route or legacy-writer fence is added.
  Regression coverage comprises three default and eighteen explicit PostgreSQL cases, selected by
  an exact CI runner. Public API/SDK contracts stay unchanged apart from release metadata.
- Added C2-G Session-authorized private Task inputs: definition-free tasks with 1–32 exact owned
  Artifact references share source authorization, Task/outbox changes and content checks on one
  transaction. Namespace pins survive cross-store switching, locks follow Task then Artifact IDs,
  and inputs are revalidated after writes before final Task/Session checks and confirmed commit.
  Reads preserve input order; missing or corrupt content also prevents cancellation. No content
  file write/repair, teacher access to private work, domain policy, authenticated Review, acceptance,
  public route, migration or live cutover is added. Three default and twenty explicit PostgreSQL
  cases cover ownership, exact references, multi-namespace faults and Session/transaction races;
  CI verifies and executes each database case.

### Changed

- Reorganized project-wide English and Chinese documentation around the platform transition, with
  a current feature map, module/boundary testing guide and concise status/roadmap. Preserved prior
  development history and dated acceptance evidence separately. Architecture, contributor, SDK,
  teaching and operations guides now distinguish the live runtime, preparatory services and local
  editing; source publication does not imply deployment, database migration or a connected save flow.

## [0.4.8] - 2026-10-03

### Added

- Added C2-F Session-authorized private Artifact creation, immutable revision and exact-content reads.
  Task and Artifact adapters share source-owned identity, namespace and final Session checks; pending
  bytes are returned only after confirmed commit. Borrowed reads lock revision heads, preserving
  consistency under READ COMMITTED. Three default and eighteen PostgreSQL cases cover ownership,
  lifecycle, concurrency, file faults and publication failures; CI selects every case. Database rollback
  can retain unpublished private files and never authorizes automatic deletion/retry. No shared access,
  Task-input/Review authorization, public upload, live cutover or existing-data migration is enabled.
- Added C2-E Session-authorized private manual Task commands: current account incarnation, audited
  active Human/membership and Task/outbox operations share one source-owned transaction. Ownership
  is server-derived; private work requires no teaching/deployment role and managers gain no access
  to other owners' tasks. Pinned namespaces and post-wait Session checks fail closed. Three default
  and seventeen PostgreSQL cases cover lifecycle, concurrency, rollback and connection restoration;
  CI selects every database case. No domain/Artifact-backed Task authorization, authenticated Review,
  acceptance, public route, live cutover, schema migration or legacy-writer fence is introduced.
- Added C2-D revision-bound Review staging, with separate human and rule judgments, immutable
  opinion history, exact target/evidence/policy pins and atomic revision/outbox writes. Human edits
  are revision-fenced; rule results cannot be amended into approvals. Four default and seventeen
  PostgreSQL cases include a non-teaching Artifact/Task/Review fixture; CI selects every database
  case. Reviewer/source attribution remains trusted input, not authentication or verified evidence.
  No Task acceptance, public workflow, live auth change or teaching-data migration is enabled.
- Added C2-B PostgreSQL task-instance staging: manual or exact-definition snapshots, owner/scoped
  references, lifecycle transitions, optimistic revisions and atomic task/outbox commits. There is
  no automatic acceptance from Run or assessment success. Explicit empty-schema installation,
  bounded records, metadata/task locking and post-write checks fail closed. Four default and fifteen
  database regressions cover concurrency, corruption, cancellation and commit failures; CI selects
  every database case. No live route/authentication wiring, teaching migration or authenticated Review
  is enabled.
- Added C2-C immutable Artifact staging using PostgreSQL metadata/events and private Unix version
  files. Exact bytes/digests and parent revisions are verified without domain or Run dependencies;
  old Task inputs remain pinned after content edits. Publication failure rolls back database state
  but preserves files for future reconciliation, without overwrite, adoption or automatic deletion.
  Four default and seventeen explicit PostgreSQL cases cover concurrency, corruption, links, cancelled
  publication, pointer rewind and cross-service references. CI runs every database case. No live
  authorization, public file service, retention worker or existing-data migration is introduced.

### Changed

- Extracted C2-A versioned task definitions and typed rule dispatch from the eBPF teaching catalogue.
  The five existing labs now use a built-in domain pack through a compatibility adapter; source parsing,
  runtime/attachment requirements and legacy feedback remain domain-owned. A non-teaching text provider
  verifies the same core without a Run. Unknown pins fail closed; stored attempts, progress, permissions
  and HTTP/SDK contracts stay unchanged. The catalogue is not a plugin loader or a Review service.
- Updated functional-network lab discovery for the new domain-pack location without rewriting the
  historical inventory or source fingerprints.

## [0.4.7] - 2026-10-02

### Added

- Added C1-K `cyanrex-provision reconcile`: one bounded read-only snapshot checks staged account/
  Session incarnations, identity bindings, paired policies, complete audit histories and a current
  source-backed manager. No auth secrets, enrollment delivery or repair are involved; observations
  never authorize a retry. Twelve explicit PostgreSQL cases cover corruption, limits, cancellation,
  MVCC and read-only column privileges. Live runtime, frozen APIs and schema stay unchanged.

- Added C1-J native `cyanrex-provision plan/inspect/apply` for explicitly pinned local staging
  targets. Read-only preflight and target-bound confirmation precede C1-I bootstrap; private files
  keep passwords/TOTP out of arguments and console output. Existing output is never overwritten,
  and unconfirmed commit/delivery preserves evidence without automatic retry or reinitialization.
  Eight default and nine PostgreSQL cases cover safety, concurrency, cancellation and short writes;
  CI selects the database cases explicitly. No live runtime, public API or packaging change.

- Added C1-I trusted-operator bootstrap for a fresh empty namespace: the initial durable account,
  owner/teacher identity, legacy Workspace, explicit deployment grant and audit activation commit
  together. Existing/partial schemas and repeat claims are rejected; no Session or success replay is
  issued. Fourteen PostgreSQL fault/concurrency cases are selected by CI. No live API, migration of
  existing accounts, startup hook or deployment change is enabled.

- Added C1-H self-service password rotation on the explicit durable source: current Session,
  password and TOTP verification, exact-incarnation credential replacement and all-session revocation
  commit together. Seventeen PostgreSQL cases cover failures, concurrency and registry boundaries;
  CI selects each explicitly. No live endpoint, schema, grant, recovery flow or deployment is changed.

- Added restricted C1-G session-authorized deletion of another active bound source account: exact
  incarnation/Principal checks, audited retirement, all-session revocation and account deletion commit
  together. Self-delete, unbound/retired cleanup and successful deletion replay remain unsupported;
  old retirement receipts are not deletion evidence. Sixteen explicit PostgreSQL fault/concurrency
  cases are selected by CI. No live AuthService, public API, migration or deployment change.

### Fixed

- Removed stray patch markers from the local provisioning CLI help text.

- Corrected residual C1-F status wording: session-authorized binding/policy composition was already
  included in 0.4.6; bootstrap, general lifecycle integration and live cutover remain separate gates.

## [0.4.6] - 2026-09-27

### Added

- Added C1-F session-authorized identity binding and policy commands: source Session, exact account
  incarnation, current manager, mutation and audit share one transaction without a caller-supplied actor.
  Logout ordering, post-wait expiration and pre-commit source checks fail closed; a remaining manager
  must match a current source account when revoking grants. Sixteen real PostgreSQL cases are selected
  by CI, with bilingual ADR-007 and scoped maintenance rules. Account deletion/retirement composition,
  bootstrap, public routes and live cutover remain pending.

- Added C1-E durable authentication-source staging with committed account-incarnation IDs, exact
  incarnation-bound hashed sessions, password/TOTP login and confirmed logout without memory fallback.
  Explicit empty-source installation rejects existing accounts and incompatible schemas; 14 real
  PostgreSQL lifecycle/concurrency/fault cases are selected by CI. Bilingual ADR-006 records the next
  unified transaction gate. No live AuthService cutover, public API, role assignment, migration or
  deployment is enabled.

## [0.4.5] - 2026-09-27

### Added

- Added C1-D attributed identity binding/retirement commands with atomic lifecycle audit, durable
  replay and last-manager protection shared with policy revocation. Explicit identity-schema activation
  fences old maintenance writers; audited reads also lock absent binding keys. Sixteen real PostgreSQL
  cases are selected by CI. Bilingual ADR-005 and scoped maintenance guidance record the boundary:
  legacy credentials/sessions remain untouched, with no live auth cutover or automatic migration.

- Added C1-C attributed legacy policy commands with current-manager checks, revision fencing,
  durable command-ID deduplication, last-manager grant protection and atomic append-only audit.
  Explicit access-schema activation records observed baselines and fences unattributed writers;
  policy reads fail closed on missing/inconsistent audit heads. Sixteen real PostgreSQL cases are
  selected by CI. Bilingual ADR-004 documents the trusted-actor boundary and remaining lifecycle
  gates. No live authentication wiring, account migration, public API or deployment is enabled.

## [0.4.4] - 2026-09-27

### Added

- Started C0/C-M1 collaboration groundwork with typed stable IDs, Principal/Workspace/Membership
  contracts, scoped revision-pinned Artifact references and a versioned event envelope. Added a pure
  offline legacy-permission preview and regressions for ownership, teaching review and instance scope.
  Bilingual ADR-001 records the source baseline and migration gates. Existing runtime authorization,
  storage and routes are unchanged; no data migration or new public API is enabled.

- Added the explicit C1-A PostgreSQL identity registry: operator-pinned authority/legacy Workspace
  mappings, stable account-incarnation bindings, atomic Principal creation/retirement, historical
  tombstones and fail-closed storage. Fifteen real isolated PostgreSQL concurrency/fault cases are
  explicitly selected by CI. No startup migration, existing-account rewrite or live authorization
  integration is enabled; bilingual ADR-002 documents the staging boundary and remaining gates.

- Added C1-B durable legacy membership and explicit instance deployment-grant staging. Atomic
  revision-checked policy replacement rejects stale re-grants, while fresh database previews retain
  private ownership, student-only teaching review and scope boundaries. Fifteen real PostgreSQL
  lifecycle/concurrency/fault cases are selected by CI. No live authentication or policy cutover is
  enabled; bilingual ADR-003 records the check/use boundary and remaining migration requirements.

### Changed

- SDK patch compatibility applies to every pre-1.0 minor line. A member deprecated during `0.4.x`
  remains for the complete `0.5.x` line and can be removed no earlier than a reviewed `0.6.0` release.
  Frozen OpenAPI and SDK surface baselines are unchanged.

## [0.4.3] - 2026-09-25

### Fixed

- Updated the locked Rust TLS dependency from rustls 0.23.43 to 0.23.45 and its required
  rustls-webpki dependency from 0.103.13 to 0.103.15, fixing
  [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html) and the CI security
  audit failure without adding an advisory exception. Added an offline security-floor regression;
  rebuild and redeploy Engine and Runner Agent binaries/images to apply the TLS fix.

## [0.4.2] - 2026-09-24

### Fixed

- Runner administration validates Agent/job inventories before display and keeps failed reads read-only.
  Private requests now have header/body deadlines and Engine/navigation ownership; obsolete responses
  cannot overwrite current state. Probes require confirmation, cancellation rechecks reviewed targets,
  and matching job acknowledgements distinguish confirmed operations from uncertain writes. Uncertainty
  survives background polling until an explicit verified refresh; no mutation is retried automatically.
  Added four-language guidance, isolated regressions and a fifteenth functional-network bug-hunt record.

- Performance metrics now validate snapshots before rendering, preventing malformed responses from
  crashing settings and Runner controls. Reads are private, deadline-limited and bound to navigation;
  polling waits after completion and locale changes no longer restart it. Failed refreshes label the
  retained snapshot as stale, remove current-health styling, and keep their own four-language feedback.
  Saving or loading unrelated settings cannot hide metrics failures or block an independent refresh.
  Added isolated parser/browser regressions and a fourteenth functional-network bug-hunt record.

- Settings forms now require a verified event-settings read before editing or saving. Strict Mode,
  navigation and reload generations reject obsolete responses; private requests have read/write
  deadlines and no automatic mutation retries. Confirmation freezes validated values and locks editing.
- Event and compiler saves require exact acknowledgements. Unconfirmed or partial saves explicitly
  require reloading before another save; compiler-unavailable event-only saves are clearly identified.
  Confirmed event retention changes refresh unread state. Added four-language feedback, isolated
  browser/request regressions and a thirteenth functional-network bug-hunt record.

## [0.4.1] - 2026-09-24

### Fixed

- Event retention HTTP settings now read configured storage without cached/default success on failure.
  Invalid stored policies/limits, unavailable storage and bounded read waits return private `503`.
  Saves require a confirmed policy/trim transaction and persistence barrier, preserving memory on
  definite SQL failure; admitted saves retain owner ordering after cancellation or the ten-second caller
  deadline. Unconfirmed is not rollback. Success and JSON/content-type/body-limit errors are no-store;
  existing clamping, owner/CSRF rules, memory-only mode and legacy runtime helper contracts remain.

- Event mark-read/deletion HTTP acknowledgements now require confirmation from configured storage.
  Failed barriers, prior fallback and unconfirmed writes return private `503`, not volatile success;
  definite query/schema failures preserve memory and remain retryable. Caller waiting is limited to
  ten seconds; admitted SQL work retains owner ordering after cancellation/timeout, which is not rollback.
  Memory-only changes stay atomic while waiting for cache locks. Deletion query errors are private JSON.

- Event history, JSON/CSV export and unread HTTP reads now report generic no-store `503` responses for
  configured storage failures, decoding errors, ten-second read waits or previously latched fallback,
  instead of empty/partial/zero success. Read failures do not disable persistence; repaired storage is
  retryable. History SELECT plans no longer outlive column-type repairs. Explicit memory-only reads remain.
- Event reads reject invalid/unknown filters, unsupported export formats and overflowing/reversed time
  ranges with no-store `400` responses. Successful payloads, session ownership and legacy ignored fields
  remain compatible; isolated HTTP/PostgreSQL regressions and failure contracts are covered in CI.

- Cold-start DropNew admission now counts durable rows plus locally accepted pending events before
  history, unread or live publication. Owner-scoped reservations survive writer-cache expiry and are
  rebased after deletion, replacement or retention changes; cancellation cannot consume a slot.
- The event persistence worker no longer keeps its own producer channel alive. After the last producer
  leaves, healthy queued batches drain and the worker releases its state while the runtime remains alive.
  Added isolated cold-cache, cancellation, fallback and lifetime regressions; API and volatile fallback
  contracts remain unchanged. This is not crash recovery or a process-shutdown durability guarantee.

## [0.4.0] - 2026-09-24

### Changed

- Reduced local Rust development/test build disk usage with limited debug information and disabled
  incremental compilation; release builds remain unchanged. Removed tracked TypeScript build metadata
  and documented cache cleanup and explicit debugging overrides. Runtime data and deployment are unaffected.

### Fixed

- Event PostgreSQL batch/replacement inserts now generate valid SQL and bind native JSON. Ordered
  persistence barriers keep earlier publications ahead of read acknowledgement, scoped deletion,
  replacement and retention changes; queue overflow no longer spawns unbounded bypass writers.
- Event settings/trim and replacement use transactions before publishing memory. Per-owner mutation
  admission survives admitted SQL caller cancellation without blocking unrelated owners; memory-only
  cancellation cannot split settings/history/unread updates. Capacity caches are invalidated after
  mutations, cold settings reads cannot overwrite newer values, and equal-time retention preserves
  publication order. Added real disposable PostgreSQL and fault/queue regressions, explicitly wired into CI.
  Existing volatile fallback and raw Event/API contracts remain; this is not durable replay or cross-process coordination.

- Event history rejects invalid/reversed filters without crashing or issuing a wider query, hides
  previous-scope rows immediately, and ages rolling windows without reconnecting. Export is single-flight,
  context-bound and deadline-limited, verifies the requested MIME type and uses safe download filenames.
- Event deletion validates exact confirmation/counts, honors navigation cancellation and reports uncertain
  outcomes without retries. Confirmed deletion refreshes history and invalidates stale unread responses.
  Read acknowledgements are single-flight, filter-bound and visibly retryable; their existing all-owner
  scope is now explicit. Sidebar polling is nonoverlapping and shows unavailable instead of a false count.
  Added four-locale notices and isolated frontend/browser regressions; Engine authority/wire/storage stay unchanged.

## [0.3.9] - 2026-09-13

### Added

- Saved a bilingual functional-network map covering 12 modules, 40 workflows and 69 links, with a
  machine-readable inventory, source-drift checker and six linked bug-hunt reports. Original 0.3.8
  snapshots, fingerprints and pre-release test evidence remain unchanged; they are not 0.3.9 artifact
  acceptance. The fixes below preserve existing teacher authority and API compatibility.

### Fixed

- Breakpoint views now bind hits to the Engine, debug session and declared instrumented lines before
  rendering, rejecting malformed/out-of-scope line numbers. Editor/model ownership also governs glyphs,
  keyboard callbacks and disposal; obsolete callbacks cannot alter a replacement editor or model.
- Event recovery snapshots now request no-store and reject redirects. Malformed live frames leave a
  visible possible-gap notice without fabricating events or forcing reconnects. Trace markers require
  a complete positive decimal line number instead of accepting numeric prefixes. Added isolated browser,
  production-Monaco, transport and Rust parser regressions; raw Event format and server authority remain.

- Manual run and detach share a synchronous admission guard and navigation-bound cancellation. Draft
  changes hide obsolete results without unloading programs; completed runs no longer await supplementary
  inventory/progress reads. UTF-8 source limits and structured runtime responses are checked before use.
- Attachment reads are latest-wins and bounded, retaining the last successful inventory with an explicit
  warning on failure. Cleanup requires an explicit `clean: true`; verified removal retires result actions.
  Transport uncertainty reaches the confirmation failure view without retries, while normal compiler/
  validation reports and explicit authorization failures retain their meaning. Added four-locale notices
  and isolated portable/controller/production-UI regressions; server authority and bulk scope are unchanged.

- Semantic completion is now owned by its editor/model and exact header context, with independent
  cancellation, a ten-second whole-request deadline and bounded five-second caching. Late callbacks,
  malformed items and request failures cannot leak results or unhandled rejections; disposed editors
  unregister all language providers. Three SEC snippets now insert real line breaks.
- Manual header self-checks reject duplicates, cancel on input/context changes and distinguish service
  failure from compiler issues. Header refreshes are latest-wins and deadline-bounded, invalidate checks
  even when filenames stay unchanged, and retain the last successful list with a four-locale error notice.
  Diagnostic markers target the owned editor rather than Monaco's first global model. Added portable,
  real-React and production-Monaco regressions without changing teacher/student permissions or Run behavior.

- Browser compiler diagnostics now use editor-local, exact Engine/target/source/header cache keys;
  remounts and separate editors do not share cached results or cancellation ownership. Stale successes,
  errors and markers cannot overwrite the current draft, including during debounce.
- Inline checks bound the whole request to 20 seconds locally or 35 seconds remotely, including remote
  submission and polling. Late known job IDs receive best-effort cancellation, without retries or local
  fallback. Cancelled/expired remote jobs show unavailable rather than code issues. Added portable
  transport regressions, real-React browser lifecycle tests and production-editor confirmation coverage.

- Runner Agent claim admission no longer subtracts reported active jobs twice, while retaining reserved
  capacity and counting cancellation-pending leases. Lost leases stop the bundled client before execution
  or result submission without stopping subsequent polling.
- User-owned remote checks expire after 35 unclaimed seconds on the next queue interaction, releasing
  source and active-user quota. Claimed execution deadlines and staff-managed queue semantics stay unchanged.
- Agent response bodies enforce the 640 KiB limit incrementally, including chunked success/error bodies
  without Content-Length; oversized streams are rejected without waiting for EOF. Added lifecycle,
  signed capacity/owner/CSRF and loopback protocol regressions, with explicit OpenAPI behavior notes.

- Chain-guided learning-history, progress and teacher reads no longer hide corrupt/unreadable local
  snapshots or selected PostgreSQL read errors behind empty success. Errors are generic and retryable;
  selected SQL reads do not silently switch to stale local data. Successful payloads/access stay compatible,
  and successful learning projections/storage errors use no-store responses with explicit OpenAPI coverage.
- Local learning commits use exclusive, random same-directory temporary files, new Unix 0600/0700
  file/directory permissions and failure cleanup. Existing temporary symlinks are not followed or removed;
  failed commits preserve the published snapshot. Cancellation and feedback revision guarantees remain;
  this does not add fsync/crash recovery or change permissions of existing parents.

## [0.3.8] - 2026-09-12

### Fixed

- Resolved the nine module/boundary findings: login confirms session insertion and verified account
  credentials in one transaction; account deletion rejects surviving sessions. Stale login, suppressed
  writes and cancellation cannot publish premature cache tokens.
- Local scripts preserve corrupt/unreadable snapshots and failed-write cache state. Shared admission,
  private temporary files and atomic replacement serialize saves/deletes and finish admitted commits
  after cancellation; the existing JSON format remains unchanged (no fsync guarantee).
- Event deletion removes matches rather than their complement, rejects invalid/empty/unknown filters
  and unsafe time ranges, preserves unread state, and never replaces SQL history from a partial cache.
  Added concurrency/cancellation and SQL regressions; promoted auth, script and event database
  boundaries into guarded CI steps with explicit OpenAPI failure responses.
- Registration no longer reports success without creating an account when authentication schema
  initialization fails. The existing memory fallback now stores the account atomically, rejects
  duplicate names, and returns usable TOTP credentials. Fallback is still volatile.
- TOTP enrollment links encode issuer, account and secret components so custom issuer names containing
  Unicode or URL delimiters cannot corrupt the QR code's label, secret or authentication parameters.
- Added offline-failure/concurrent-registration regressions and real PostgreSQL authentication CI
  acceptance covering durable accounts, hashed sessions, duplicate registration and later DB failure.
- A database-observed revoked/expired session or deleted account now evicts its stale in-memory
  fallback, so a later database outage cannot revive that observed invalidation. Added real PostgreSQL
  regressions; this does not provide cross-process revocation while persistence is unavailable.
- Logout waits for explicit server success before navigating. Rejected, malformed, timed-out and
  network-failed responses show a four-locale warning and require a manual retry. Duplicate clicks and
  late navigation callbacks are guarded, with request-level and browser regressions.
- PostgreSQL-backed logout, password changes and account deletion no longer report memory-only success
  after a failed durable write or a latched database fallback. Unconfirmed writes return 503 without
  clearing the session cookie. Account/session deletion is transactional; rejected or zero-row account
  writes do not publish cache changes. Intentionally memory-only instances retain volatile operation.
  Repeated logout remains idempotent, but a suppressed deletion cannot leave a live durable session
  behind a success response.
- Account mutations use the verified normalized username and credential snapshot. Concurrent password
  changes cannot silently overwrite a newer password; added database fault/rollback, retry, zero-row,
  case-normalization and concurrent-writer regressions plus explicit OpenAPI failure documentation.

## [0.3.7] - 2026-09-09

### Added

- Two explicit entry paths: native SSH plan/apply for teacher-managed, pre-installed offline packages,
  and opt-in student discovery links with teacher-issued, username-bound, single-use 10-minute invitations.
  Teacher identity confirmation, independent join protocol/capability checks, password/TOTP enrollment,
  revocation, no-store secret handling, four-locale UI and typed SDK coverage. No automatic network scan,
  package upload, public exposure, VM isolation or remote kernel execution is implied.

### Changed

- Teacher-authoritative ownership: the seeded personal-use account and legacy administrator allowlist
  now report `teacher`; teachers manage deployment settings, modules/headers, Runner Agents and Terminal
  with the same session used for teaching. Legacy usernames, credential keys, API tier labels and wire
  schemas remain compatible. Review existing teacher allowlists before upgrading: they now grant full
  deployment authority. Updated role-aware navigation, four locales, launchers and architecture guides.

### Fixed

- Frontend Docker/package builds now accept an explicit Engine URL instead of fixing LAN browsers to
  localhost. Supported launchers forward classroom/CORS configuration; changing a distribution runtime
  environment still requires a frontend rebuild to update its baked-in API URL.

- Public registration cannot claim configured teacher/legacy-admin names or choose elevated roles.
  The seeded deployment teacher cannot delete itself and leave a student-only instance. Session, TOTP,
  CSRF, owner-scoped data and consequential-action confirmations remain enforced; remote loading and
  kernel isolation are unchanged. Added role, registration, owner-retention and browser regressions.

## [0.3.6] - 2026-09-09

### Added

- Retained source/binary-bound observations and native live-kernel evidence from an explicitly approved,
  bounded QEMU/KVM acceptance guest: real Aya attach, matched ring-buffer event, exact detach, unchanged
  kernel program/link inventories and a restricted-network canary. This is dev-build acceptance on one
  guest kernel, not a shipped VM Runner or a tagged/offline release acceptance result.
- Real PostgreSQL CI acceptance on a disposable loopback-only service, with an exact-test selection
  guard. The learning integration now also checks owner-bound historical submission reads, persisted
  source/current feedback, and storage failures without a local fallback. Documented the first local
  PostgreSQL and signed Runner Agent/Clang acceptance results and the remaining privileged-host checks.
- Shared, keyboard-accessible confirmations for kernel runs, attachment/script/header/event deletion,
  source replacement, remote compiler selection and consequential account/admin controls. Bulk cleanup
  requires an exact typed phrase; in-flight requests cannot be double-submitted or implicitly retried.
- Students can resume an exact historical lab submission from Learning Center, preview current teacher
  feedback, and explicitly replace or keep their editor draft. Pending/failed/cancelled requests and
  late lab templates cannot overwrite the draft; loading never runs or attaches a program automatically.
- Added owner-bound `GET /learning/attempt?attempt_id=...` with no-store responses, typed
  `learning.attempt()` / `getLearningAttempt` SDK access, permission regressions and browser coverage.

### Changed

- Opening a lab now preserves the current editor draft. Its template is loaded through an explicit
  review action, and an already-confirmed file import cannot replace a different page's draft after navigation.
- Task-focused UI: persistent desktop navigation, an accessible compact-screen menu, a sticky editor
  action bar, source/results beside runtime settings, and on-demand resource panels. Learning progress
  now precedes documentation, with section shortcuts; classroom rows adapt to labelled mobile cards
  and selected reviews receive focus. Existing permissions, drafts and manual execution stay intact.
- Local LearningStore first-load I/O, whole-file UTF-8 validation and JSON parsing run on a blocking
  worker with shared initialization/write admission. Records decode directly into shared pointers while
  retaining the plain JSON format and full-input buffer. Added cold-load cancellation, concurrency,
  legacy/invalid-input regressions and paired first-read, timer-responsiveness and process-RSS benchmarks.

### Fixed

- Raised the frontend Next.js floor to 15.5.24 and sharp override to 0.35.4 for the upstream Windows
  server and AVIF/libheif security advisories found by the safety-workflow dependency audit.
- Single detach cannot silently fall back to detach-all without a path. Event deletion reviews the full
  filter scope with a fixed time cutoff, not a misleading 200-row preview count. Header batches check
  every response, stop at the first failure and report partial completion without claiming rollback.
- The editor's loaded Monaco view can shrink on narrow screens instead of forcing the page and
  submission-resume confirmation beyond the viewport; stale lab progress cannot bootstrap another lab's template.
- Cancelling an admitted cold-load request no longer discards successful initialization or permits
  duplicate detached loads. A worker publishes the complete snapshot before marking the store ready.
- Local learning path errors are no longer mistaken for an absent record file: only `NotFound` initializes
  an empty store. Other read/decode failures remain retryable without publishing partial results.

## [0.3.5] - 2026-09-08

### Changed

- Local LearningStore writes stream the existing pretty JSON through a 64 KiB buffer on a blocking
  worker, eliminating the full encoded-file allocation while still replacing the entire file. Added
  cancellation, queued-worker, exact-byte, partial/interrupted-write and flush-failure regressions.
- Local LearningStore reads share immutable snapshots, select bounded recent pages before copying
  source, and aggregate progress/teacher views in one pass. Appends copy only the pointer index and new
  record; feedback copies only the index and edited record. JSON format, full-history responses, SQL
  queries and revision/failure semantics stay compatible; local writes still replace the whole file.
- Added snapshot/selection/aggregation regressions and paired local LearningStore benchmarks with
  regression-tested run ordering, retaining earlier diagnostic measurements without overwriting them.
- Event WebSockets now subscribe to bounded queues keyed by the authenticated owner, sharing immutable
  events and lazily encoded JSON across that owner's connections. Idle queues are removed when the last
  subscription closes or is cancelled; the legacy global EventBus subscription API remains available.
- Added owner-isolation, concurrent subscription lifecycle, shared-encoding, and compatibility regressions,
  plus release-mode fan-out comparisons and updated real-loopback slow-consumer pressure checks.

### Fixed

- Cancelling a local learning append or teacher-feedback request no longer releases the writer lock
  between disk rename and memory publication. An admitted worker completes despite request cancellation;
  queued callers remain cancellable before handoff. This is not crash recovery, fsync or exactly-once retry.
- Another user's event burst no longer overruns an unrelated WebSocket subscription. An owner's own
  overload still closes with `1013`, preserving the existing recent-history recovery protocol and deadlines.

## [0.3.4] - 2026-09-08

### Changed

- In-memory event histories now use FIFO deques and evict before insertion, avoiding steady-state
  head shifts and extra growth at full retention while preserving overflow, ordering, and unread behavior.
- Added isolated release-mode mainline benchmarks and event-history regressions covering rollover,
  capacity changes, filtering, replacement, broadcast delivery, and concurrent user isolation.
- In-memory event queries now select matching references within retention before cloning the requested
  page, stopping once the limit is satisfied and preserving FIFO responses, time filters, and full exports.
- Added private filtered-read benchmarks plus differential and bounded-visit regressions, without
  exposing new Engine endpoints or changing database queries.
- Event WebSockets preserve raw Event JSON while closing explicitly on lag and bounding data/close
  sends. Both browser consumers now reconnect with backoff, refresh recent history, cancel stale
  snapshots, bound buffers, and keep possible-gap notices visible; recovery is not durable replay.
- Added real loopback WebSocket pressure checks and client recovery regressions to cover overload,
  stalled peers, snapshot/live overlap, filters, and cancellation without kernel or database access.

### Fixed

- Cookie-authenticated event WebSocket handshakes now apply the existing Origin/Referer CSRF policy.
  Native clients must send an allowed Origin or explicitly opt into the existing missing-origin override.
- Engine Docker builds now include the OpenAPI document and SQL migrations required at compile time,
  while using the committed Cargo lockfile.
- Frontend Docker builds now use a context-local metadata cleanup hook instead of depending on a
  parent-directory script; the repository-wide cleanup command remains available.
- Added build-input and isolated frontend prebuild regressions to the common quality gate.
- Local and CI npm audits now retry transient registry failures with bounded attempts and timeouts,
  while preserving the production scope and moderate severity gate. Vulnerabilities and unusable
  reports still fail; no dependency fixes or registry substitutions are performed automatically.

## [0.3.3] - 2026-09-06

### Added

- Added teacher/admin feedback on student lab attempts, with student-visible history, Unicode-aware
  comment limits, reviewer attribution, optimistic revision checks, and PostgreSQL/local persistence.
- Added the typed `learning.saveTeacherFeedback()` SDK method and generated operation, while keeping
  automated lab acceptance separate from teacher comments and preserving the frozen API/SDK baselines.
- Added the native Rust `cyanrex-release evidence` CLI with strict duplicate-key rejection, candidate
  metadata binding, legacy v1 verification, and atomic non-overwriting report creation.
- Added native `cyanrex-release package extract` verification and staged extraction with strict bundle,
  archive, metadata, checksum, path, member-type, size, and non-overwrite enforcement.
- Added native `cyanrex-release candidate verify` for checksum-bound four-file Tag artifacts, including
  strict evidence validation, archived-metadata binding, and optional verified extraction.
- Added a native Runner Agent acceptance client with bounded HTTP responses, local-or-TLS credential
  transport, TOTP login, compile-job polling, result validation, and failure cancellation.
- Added a native privileged live-kernel acceptance client with strict environment validation, unique
  attachment/event binding, exact cleanup proof, and optional in-memory evidence creation.
- Added native extracted-package verification, checksum-bound Docker image identity inspection with
  timeout/output limits, and an Engine health probe for installation acceptance.

### Changed

- eBPF checks/completion now use owner-scoped Runner requests, per-manager compiler capacity,
  operation deadlines, and cancellation-safe metrics while preserving successful response contracts.
- Attachment inventory, detach, and debug-retry cleanup now use the selected Runner driver, with
  bounded deadlines, driver-owned cleanup reports, and no implicit local fallback on backend failure.
- Documented the Linux desktop/LAN classroom target and its unimplemented VM ownership, recovery,
  and ingress requirements, without enabling remote eBPF loading.
- Offline distributions now export the native release CLI from the Engine image and prefer it for
  live-kernel evidence, while retaining the Python implementation as a compatibility fallback during
  the remaining release-tool migration.
- CI and Tag validation now use the Rust release CLI for two-file package extraction; the Python package
  tool remains available as a compatibility and cross-implementation reference.
- Tag acceptance now uses native candidate verification after live-kernel smoke acceptance.
- Packaged `runner-agent-smoke.sh` now prefers the adjacent Rust CLI and retains its previous shell/Python
  path only for source checkouts and older manually assembled packages.
- Packaged `live-kernel-smoke.sh` now prefers the adjacent Rust CLI and retains its previous shell/Python
  path as a compatibility fallback.
- New offline distributions no longer bundle the standalone Python live-kernel evidence helper; its
  source copy remains for compatibility and cross-implementation regression coverage.
- Packaged installation acceptance now uses Rust for package/metadata checks, loaded image identities,
  and health JSON validation, removing its host Python requirement.

### Fixed

- Local compiler caches now separate users, and private source workspace guards clean up on cancellation
  as well as normal return; cancelled header setup cannot recreate a workspace after cleanup.
- Runner execution now rejects a request whose user differs from the execution lease owner before
  invoking the driver or changing lease metadata.
- Installation smoke preflight now preserves existing runtime configuration and Agent tokens; cleanup
  removes only state created by the current acceptance run.

## [0.3.2] - 2026-09-04

### Added

- Generated TypeScript inputs, responses, access tiers, transports, and runtime descriptors for all
  56 browser-facing OpenAPI operation IDs.
- Added `CyanrexClient.operation()` without removing the stable task-oriented SDK namespaces; JSON,
  event-download, and WebSocket transports are supported while signed Runner Agent calls stay isolated.
- Added operation-generator regressions, compile-time SDK fixtures, and packaged `/operations` exports.
- Added a release preflight that rejects dirty candidates, reused/lightweight tags, version drift, and
  missing dated changelog entries, plus automatic validation for future pushed version tags.
- Added an additive-only compatibility baseline for 77 public `CyanrexClient` member paths and a packaged
  stability/deprecation policy.
- Added machine-readable offline-package metadata that records Git source state, matching annotated Tag,
  image references, build mode, and the streamed SHA-256 of the bundled image archive.
- Added a Tag candidate workflow that binds a clean source revision to a locally built distribution,
  runs the extracted-package installation smoke, and retains the accepted archive without publishing it.
- Bound every packaged image reference to its Docker content ID and made installation acceptance reject
  inherited image overrides or loaded image identities that differ from the candidate metadata.
- Added privileged Tag-candidate acceptance that loads the built-in Aya `sched_switch` ring-buffer
  program, requires a uniquely bound real kernel event, detaches its exact pin, rejects residue, and
  retains checksum-addressed evidence bound to the candidate metadata and kernel environment.
- Added a packaged, strict live-kernel evidence CLI that creates self-contained v2 reports, preserves
  v1 verification, and rejects schema drift, duplicate JSON keys, tampering, or candidate mismatches.
- Added a single downloaded-candidate verifier that streams the outer archive without extraction,
  rejects unsafe members, checks every packaged file, and cross-binds release metadata to kernel evidence.
- Added non-overwriting verified extraction for both two-file offline packages and complete candidates;
  CI and Tag acceptance no longer pass release archives directly to `tar`.
- Added deterministic course-document mirror validation so stale committed frontend lessons fail every
  quality-gate mode instead of being repaired only as a build side effect.

## [0.3.1] - 2026-08-31

### Added

- Added a generated OpenAPI 3.1 contract served publicly from `/openapi.json`, with exact Engine route,
  access-tier, SDK coverage, operation metadata, and schema-reference drift checks.
- Added OpenAPI-generated JavaScript SDK wire types, explicit `/openapi` package export, and package
  manifest/declaration/consumer smoke tests.
- Added repository-wide semantic-version synchronization across Engine, frontend, SDK, lockfiles,
  generated API metadata, and release-facing documentation.
- Added a dynamically discovered, versioned v1 module manifest catalog with strict schema and semantic
  validation, duplicate detection, deterministic ordering, and state-only lifecycle control.
- Added catalog-backed module command/API behavior, Terminal catalog visibility, and module fixture tests.
- Added a frozen `0.3.0` OpenAPI compatibility baseline that rejects removed operations, access changes,
  narrowed requests, and weakened successful responses.

### Changed

- Hardened Docker and offline-distribution handling for module manifests and mirrored build inputs.

The canonical package metadata advanced directly from `0.2.9` to `0.3.1`. Version `0.3.0` identifies
the frozen API compatibility snapshot only; it was not a package release and must not be tagged.

[Unreleased]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.6.2...HEAD
[0.6.2]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.6.1...v0.6.2
[0.6.1]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.9...v0.6.0
[0.5.9]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.8...v0.5.9
[0.5.8]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/Team-silvortex/cyanrex-lab/compare/a9523ec052f701e447d2e59fcd38d3a8d8186740...v0.5.1
[0.5.0]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.9...a9523ec052f701e447d2e59fcd38d3a8d8186740
[0.4.9]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.8...v0.4.9
[0.4.8]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.7...v0.4.8
[0.4.7]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.6...v0.4.7
[0.4.6]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.5...v0.4.6
[0.4.5]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.4...v0.4.5
[0.4.4]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/Team-silvortex/cyanrex-lab/compare/bea50cde4a4cbe087dff9a602f1c7e03970ed946...v0.4.3
[0.4.2]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.1...bea50cde4a4cbe087dff9a602f1c7e03970ed946
[0.4.1]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.9...v0.4.0
[0.3.9]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.8...v0.3.9
[0.3.8]: https://github.com/Team-silvortex/cyanrex-lab/compare/f3f9faf585721df5e208c18da99652b040e35d50...v0.3.8
[0.3.7]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.6...v0.3.7
[0.3.6]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.2.9...v0.3.1
