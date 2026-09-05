# Changelog

All notable changes to Cyanrex Lab are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases use semantic versioning.

## [Unreleased]

### Added

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

[Unreleased]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.2.9...v0.3.1
