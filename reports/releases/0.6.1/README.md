# 0.6.1 source version validation

Date: **2026-10-15**. Previous release: **v0.6.0**. This patch source release includes explicit Linux
process preparation, private configuration versions 1 and 2, one-shot storage diagnostics, immutable
catalog declarations, retained storage connections and the prepared Session relation-lock privilege fix.

## Fresh release validation

The native `scripts/quality-gate.sh --no-npm-install` completed successfully on **0.6.1**.
Installed locked dependencies were reused; no dependency installation or implementation change
was needed in this release turn.

| Selection | Actual result |
|---|---|
| Common checks | **416 passed**; limits, version/course synchronization, generated contracts, compatibility and management tools |
| Rust defaults and documentation | **1,179 passed**, **906 opt-in ignored**, **112 successful result groups**, zero failures and Rust warnings; includes **15 compile-fail doctests** |
| Frontend | Next.js **15.5.27** production build and TypeScript passed; **250 tests passed** |
| SDK | Build, runtime, types and package-consumer checks passed; **42 tests passed** |
| Explicit PostgreSQL | **29 unique cases passed** across seven exact-selection runners, zero failures and Rust warnings |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, with no exceptions or dependency changes |

The frontend gate retains **28 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings**. Module policy
was not changed to hide them. Database/PG settings, Node injection and native release-tool overrides
were absent from the full gate. Ignored Rust cases are not counted as passes.

The SQL selections cover four Session-permission cases, four original host-catalog cases, four marker
cases, four database-connector cases, five storage-check CLI cases, four explicit catalog-declaration
cases and four retained storage-connection cases. A private PostgreSQL **18** fixture used only a
Unix socket, administrator-only local trust and SCRAM for ordinary roles. Tests used synthetic data,
including real marker locks, replacement/restore, connection cancellation and root-open observations.
Final audit found no test database, auxiliary role, extra base namespace or client backend; the stopped
temporary database was removed, reclaiming about **56 MiB**, with logs retained.

## Release identity and boundaries

Engine, root Cargo/npm locks, frontend/SDK package metadata, OpenAPI info and current README/Runner
references advance to **0.6.1**. API compatibility stays at **0.3.0**; dependencies, public operations,
SDK surfaces and Task contracts remain unchanged. Historical execution dates, counts and tags are
preserved. Dated implementation-stage descriptions retain their original scope.

The release preservation check protects **2,764 files**, normalizing only intended root version
metadata. Implementation, dependencies and prior execution records match the completed development
state. Current guides gain source-inclusion notes without rewriting their earlier evidence.

This report precedes the version commit. Committed-candidate and annotated-tag preflight bind
publication to the exact release tree; remote workflow results require separate verification.

The teaching runtime remains unchanged. The platform library has no mounted listener, public Session
issuer, automatic installation/migration or generic browser saving. Filesystem checks have no hard
deadline; catalog/marker/root comparisons are not atomic, continuous authorization or transport identity.
Tag publication does not create a GitHub Release, npm publication or deployed-LAN acceptance.

This release turn did not rerun browser/Source-TLS suites, RustSec, privileged live-kernel acceptance,
offline Docker extraction/smoke, disk-full/fsync/power-loss tests or performance benchmarks. Earlier
browser and implementation evidence retains its original date, source version and test scope.

Raw release logs are retained outside the repository at `/tmp/cyanrex-release-0.6.1-RSlqvS8F`.
