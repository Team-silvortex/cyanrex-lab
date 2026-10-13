# 0.5.9 source version validation

Date: **2026-10-13**. Previous release: **v0.5.8**. This compatible patch includes reviewed local
multi-file text import, complete five-language owned UI dictionaries including Korean, bilingual
public guides and architecture/error-boundary cleanup. The Source/HTTPS test chain checks existing
explicit file delivery; normal-app file-distribution mounting, server-backed task saving, Session issuance,
restart/resume and peer pairing are not introduced. Korean course bodies remain unavailable and
the interface explicitly identifies the English fallback.

## Fresh release gate

The native `scripts/quality-gate.sh --no-npm-install` completed successfully on **0.5.9**,
without a failing release-gate run or test/source correction in this release turn.

| Selection | Actual final result |
|---|---|
| Rust defaults and documentation | **1,100 passed**, **861 opt-in ignored**, **98 successful result groups**, zero failures/Rust warnings; includes **15 compile-fail doctests** |
| Common checks | **369 passed**; source limits, version/course synchronization, generated contracts, compatibility and management tools |
| Frontend | Next.js **15.5.27** production build and TypeScript passed; **250 tests passed** |
| SDK | Build/runtime/types and package-consumer checks passed; **39 runtime + 3 package tests passed** |
| Production-page browser regression | **94 passed**, zero failures/skips; includes Korean, local import, model/undo, confirmations, runtime safety, learning and adjacent four-language paths |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, with no exceptions or dependency changes |

There are **28 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings** in the frontend Node tests.
They remain visible in the log; the module policy and dependencies were not changed to hide them.
Installed dependency trees were reused. Only root package versions changed in the Cargo/npm locks.
Inherited database, PostgreSQL, fallback, dotenv, Node and native-release-tool injection settings
were removed from the gate. Opt-in ignored cases are not counted as passes.

The fresh browser run uses the built **0.5.9** frontend, real Monaco and controlled Engine-response
fixtures. It does not authenticate to a deployed Engine, run real kernel programs or establish
server-backed task saving. The owned temporary frontend was stopped after the run.

## Release identity and retained evidence

Canonical Engine, root Cargo/npm locks, frontend/SDK packages, OpenAPI info, both root READMEs and
runner-agent documents advance to **0.5.9**. API compatibility remains frozen at **0.3.0**. Dependency
versions, public operations, SDK surfaces, Task contracts and capability scores are unchanged.
The completed Unreleased entries move into the dated changelog section. Existing tags and old
release reports remain immutable.

The preservation snapshot checks all **485 engine source files** and **1,218 protected records**.
It normalizes only the intentionally advanced source versions in package/lock/OpenAPI metadata;
dependency entries, backend/SDK source, all locale dictionaries, Task contracts and old execution
tails remain unchanged from the completed development state. Release inclusion notes update
current scope without relabeling previous dated validation as a new release run.
Separate historical checks preserve **106 status blocks**, **58 prior execution tails** and
the original protected capability/release records; version-only metadata is verified independently.

Earlier implementation results in [architecture](../../../docs/en/architecture-map.md),
[text import](../../../docs/en/local-text-file-import.md),
[localization](../../../docs/en/localization.md) and
[Source/HTTPS](../../../docs/en/large-file-source-https.md) retain their original 0.5.8/Unreleased
scope. Their SQL/TLS/browser counts are not fresh 0.5.9 release acceptance. The release report
precedes the version commit; committed candidate and annotated-tag preflight bind publication
to the exact release tree. Tag publication does not itself prove offline-package extraction,
deployed authentication or kernel/LAN acceptance, or create a GitHub Release.

## Validation limits

There is no fresh opt-in PostgreSQL/Source-TLS run, RustSec audit, deployment, privileged live-kernel
acceptance, offline Docker archive build/extraction/smoke, disk-full/fsync/power-loss test or new
performance benchmark in this release. Production npm audits and tag metadata preflight are
separate checks, not substitutes for those boundaries. The tag workflow independently builds and
validates its offline candidate after publication.

Raw release logs remain outside the repository at `/tmp/cyanrex-release-0.5.9-0WThQ4CR`.
