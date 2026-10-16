# 0.6.2 source version validation

Date: **2026-10-16**. Previous release: **v0.6.1**. This patch source release adds existing Session
operations over retained platform storage, strict format 3 catalog/five-marker declarations and
internal startup, HTTP configuration/query and local Task controller separation.

## Fresh release validation

The native `scripts/quality-gate.sh --no-npm-install` completed successfully on **0.6.2**.
Installed locked dependencies were reused; no dependency installation or implementation change
was needed in this release turn.

| Selection | Actual result |
|---|---|
| Common checks | **446 passed**; limits, version/course synchronization, generated contracts, compatibility and management tools |
| Rust defaults and documentation | **1,216 passed**, **918 opt-in ignored**, **118 successful result groups**, zero failures and Rust warnings |
| Frontend | Next.js **15.5.27** production build and TypeScript passed; **250 tests passed** |
| SDK | Build, runtime, types and package-consumer checks passed; **42 tests passed** |
| Explicit PostgreSQL | **41 unique cases passed** across nine exact-selection runners, zero failures and Rust warnings |
| Real browser | **31 payload/navigation/import + 4 layout cases passed**, zero failures, cancellations or skips |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, with no exceptions or dependency changes |

The frontend gate retains **28 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings**. Module policy
was not changed to hide them. Database/PG settings, Node injection and native release-tool overrides
were absent from the full gate. Ignored Rust cases are not counted as passes.

Browser tests used the final **0.6.2** Next production build, Chrome and real Monaco at a loopback-only
fixture. The 31 cases include both delayed JSON-read guards; four layout cases cover desktop/mobile,
long filenames and all five locales. Engine identity/unread endpoints were mocked, so these tests are
not live authentication or StrictMode effect-replay proof. The layout runner retains one non-blocking
`MODULE_TYPELESS_PACKAGE_JSON` warning.
The fixture server was stopped afterward; its process exited and port 33217 no longer listened.

The PostgreSQL selections cover host preflight (4), storage
markers (4), Session permissions (4), database connections (4), storage diagnostics (5), catalog
declarations (4), retained storage connections (4), bound Session connections (7) and marker
declarations (5). The format 3 CLI case includes its 15-child-process matrix, counted once.

A private PostgreSQL **18** fixture used only a Unix socket, administrator-only local trust and SCRAM
for ordinary roles. Final audit found only the original postgres/template databases, the fixture
administrator, public base namespace and no other client backend; TCP listening was disabled and HBA
errors were zero. The stopped temporary database/socket directories were removed, reclaiming about
**55.2 MiB**, with logs retained. Existing application data and services were untouched.

## Release identity and boundaries

Engine, root Cargo/npm locks, frontend/SDK package metadata, OpenAPI info and current README/Runner
references advance to **0.6.2**. API compatibility stays at **0.3.0**; dependencies, public operations,
SDK surfaces and Task contracts remain unchanged. Historical execution dates, counts and tags are
preserved. Dated implementation-stage descriptions retain their original scope.

The release preservation check protects **2,817 files**, normalizing only intended root version
metadata in seven files. Implementation, dependencies and prior execution records match the completed
development state. Current guides gain source-inclusion notes without rewriting earlier evidence.

This report precedes the version commit. Committed-candidate and annotated-tag preflight bind
publication to the exact release tree; remote workflow results require separate verification.

The teaching runtime remains unchanged. The platform library has no mounted listener, public Session
issuer, automatic installation/migration or generic browser saving. Filesystem checks have no hard
deadline; catalog/marker/root comparisons are not atomic, continuous authorization or transport identity.

Tag publication triggers immutable metadata validation and a distribution workflow covering offline
Docker packaging, tag-bound extraction, live-kernel installation smoke and candidate verification.
Those remote results are separate from this local source validation. The workflow does not create a
GitHub Release, publish npm packages or deploy services.

This release turn does not cover Source-TLS suites, RustSec, local privileged live-kernel acceptance,
offline Docker extraction/smoke, disk-full/fsync/power-loss tests or performance benchmarks. Earlier
implementation evidence retains its original date, source version and test scope.

Raw release logs are retained outside the repository at `/tmp/cyanrex-release-0.6.2-5TGrLDNN`.
