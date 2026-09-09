# Changelog

All notable changes to Cyanrex Lab are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases use semantic versioning.

## [Unreleased]

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

[Unreleased]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.6...HEAD
[0.3.6]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.5...v0.3.6
[0.3.5]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.4...v0.3.5
[0.3.4]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.3...v0.3.4
[0.3.3]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.2...v0.3.3
[0.3.2]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.2.9...v0.3.1
