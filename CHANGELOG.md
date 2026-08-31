# Changelog

All notable changes to Cyanrex Lab are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases use semantic versioning.

## [Unreleased]

### Added

- Generated TypeScript inputs, responses, access tiers, transports, and runtime descriptors for all
  56 browser-facing OpenAPI operation IDs.
- Added `CyanrexClient.operation()` without removing the stable task-oriented SDK namespaces; JSON,
  event-download, and WebSocket transports are supported while signed Runner Agent calls stay isolated.
- Added operation-generator regressions, compile-time SDK fixtures, and packaged `/operations` exports.
- Added a release preflight that rejects dirty candidates, reused/lightweight tags, version drift, and
  missing dated changelog entries, plus automatic validation for future pushed version tags.

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

[Unreleased]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/Team-silvortex/cyanrex-lab/compare/v0.2.9...v0.3.1
