# 0.6.0 source version validation

Date: **2026-10-15**. Previous release: **v0.5.9**. This minor source release includes Task,
Settings and Events layout improvements, an independent Session lifecycle adapter, explicit Session
stock limits and read-only platform host catalog, Artifact directory and storage marker observations.

## Fresh release gate

The native `scripts/quality-gate.sh --no-npm-install` completed successfully on **0.6.0**.
Installed locked dependency trees were reused; no dependency installation or implementation change
was needed in this release turn.

| Selection | Actual result |
|---|---|
| Common checks | **388 passed**; source limits, version/course synchronization, generated contracts, compatibility and management tools |
| Rust defaults and documentation | **1,123 passed**, **884 opt-in ignored**, **102 successful result groups**, zero failures; includes **15 compile-fail doctests** |
| Frontend | Next.js **15.5.27** production build and TypeScript passed; **250 tests passed** |
| SDK | Build/runtime/types and package-consumer checks passed; **42 tests passed** |
| Browser regression | **311 passed** across all **31 browser files**, zero failures/cancellations/skips; separate runs contain **85** independent controller cases and **226** remaining cases |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, with no exceptions or dependency changes |

The browser selection covers real built-page layout, local Task payload/model/undo, five-language
controls, reviewed dangerous actions, events, classroom enrollment and adjacent runtime safety.
Controller fixtures use actual hooks with synthetic models/transports; production-page fixtures use
the built 0.6.0 frontend, real Monaco and controlled Engine responses. Neither authenticates against
a deployed Engine or runs real kernel programs. The owned loopback-only frontend was stopped.

The full frontend gate retains **28 non-blocking `MODULE_TYPELESS_PACKAGE_JSON` warnings**;
the remaining browser run reports **4** of the same class. Module policy and dependencies were
not changed to hide them. Database/PG connection settings,
Node injection and native release-tool overrides were absent from the full gate. Opt-in ignored
cases are not counted as passes.

## Release identity and preservation

Canonical Engine, root Cargo/npm locks, frontend/SDK packages, OpenAPI info, both root READMEs and
runner-agent documents advance to **0.6.0**. API compatibility remains frozen at **0.3.0**. Public
operations, SDK surfaces, dependencies and Task contracts are unchanged. Completed Unreleased
entries move into the dated changelog section; existing tags and historical reports remain intact.

The release preservation check protects **2,437 files**, normalizing only intentional version
strings in package/lock/OpenAPI metadata. Protected source, dependency entries and prior execution
records match the completed development state. Current documentation gains source-inclusion notes
without changing earlier dates, counts or capability scores.

This report precedes the version commit. Committed-candidate and annotated-tag preflight bind
publication to the exact release tree; tag publication alone does not create a GitHub Release or
establish remote CI, offline-package or deployed-LAN acceptance.

## Runtime boundary

The existing teaching runtime and authentication remain unchanged. The platform library has no
mounted host, public Session issuer, automatic migration or generic browser Task saving. Five valid
storage markers do not establish complete business-schema compatibility, authority or content health.
Historical UI, SQL and release records retain their original dates and scope.

There is no fresh opt-in PostgreSQL/Source-TLS run, RustSec audit, privileged live-kernel acceptance,
offline Docker archive extraction/smoke, disk-full/fsync/power-loss test or performance benchmark in
this release turn. Earlier real marker/catalog and Session SQL runs remain implementation evidence
at their original source version; they are not relabeled as fresh 0.6.0 execution.

Raw release logs remain outside the repository at `/tmp/cyanrex-release-0.6.0-j8uquWkZ`.
