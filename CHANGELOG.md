# Changelog

All notable changes to Cyanrex Lab are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases use semantic versioning.

## [Unreleased]

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

[Unreleased]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.4.0...HEAD
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
