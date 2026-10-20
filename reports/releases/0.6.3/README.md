# 0.6.3 source version validation

Date: **2026-10-20**. Previous release: **v0.6.2**. This patch source release adds an explicitly
configured low-privilege loopback platform host, native TLS, bounded existing-account Session
issuance and private Task create/list/reopen/manifest replacement. It also improves page-local
deployment diagnostics, source-launcher targeting, bounded package health observations and
read-only installed-package control verification.

## Fresh complete quality gate

The native `scripts/quality-gate.sh --no-npm-install` completed successfully on **0.6.3**.
Installed locked dependencies were reused. Both Rust debug profiles were set to zero and build
parallelism to two for compact release validation; release-build profiles were not changed.

| Selection | Actual result |
|---|---|
| Common checks | **536 passed**; file/version/course synchronization, generated contracts, compatibility and management tools |
| Rust defaults and documentation | **1,402 passed**, **1,006 opt-in ignored**, **137 successful result groups**, zero failures or Rust warnings |
| Frontend | Next.js **15.5.27** production build and TypeScript passed; **262 tests passed** |
| SDK | Build, runtime, type and package-consumer checks passed; **42 tests passed** |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, without exceptions or dependency upgrades |

Ignored Rust selections are not counted as passes. The frontend gate retains **29 non-blocking
`MODULE_TYPELESS_PACKAGE_JSON` warnings**; module policy was not changed to hide them. Its production
audit succeeded after one transient registry connection retry. Database/PG settings, Node injection
and native release-tool overrides were absent from this full gate.

The legacy Task structural regression now follows the factored original-Session helpers, including
the final exact Session comparison, fresh database expiry and immediate finish-to-COMMIT ordering.
This test update changes no runtime authorization policy.

## Fresh browser verification

Google Chrome/Playwright ran **35 actual passing cases**, with zero failures, skips or cancellations:
25 isolated Runner administration component cases, eight Helper cases and two adjacent runtime/
learning localization cases. Helper and adjacent checks used the final **0.6.3** Next production build;
Runner checks exercised the current source component in its existing browser fixture.

Six desktop 1440px/mobile 390px screenshots cover initial, successful and failed Helper checks.
Inspected failures show the actual localized error, restored action button and cleared previous
reports; geometry checks found no horizontal overflow and page-error lists were empty. All backend
identity/diagnostic requests were intercepted synthetic responses, not live teaching or platform
authentication, server Task saving, privileged operations or deployment acceptance.

The owned production fixture stopped cleanly and its exact loopback port **36317** subsequently
refused connections. Screenshots, geometry, browser logs and the closed-port/build-identity proof
remain under `browser/` in the external evidence directory.

## Fresh PostgreSQL verification

All **208 unique exact cases passed** on source **0.6.3**, across eighteen explicitly selected
runners, with zero failures, duplicate selections or Rust warnings. These results were rerun after
the version increment rather than reused from the implementation's earlier 0.6.2 test runs.

| Boundary | Exact passes |
|---|---:|
| Task replacement / creation / index / reopening | 10 / 8 / 8 / 8 |
| Task content store / Session content | 21 / 33 |
| Artifact store / Session Artifact | 17 / 18 |
| Original-storage opener / retained login / login HTTP / Session router | 7 / 15 / 7 / 7 |
| Plain loopback host / native TLS host | 8 / 8 |
| Shared login password / OTP freshness / OTP consumption / Session stock | 7 / 7 / 11 / 8 |

The replacement cohort includes actual competing requests blocked by an observed original Artifact
fence, same-revision conflicts, current-account recreation, suspension, complete old/new byte checks
and hit-proven immediate/deferred faults. Task writes preserve original file names, device/inode,
permissions, link counts and bytes. The login cohort includes three actual password-work gap cases.

A private PostgreSQL **18.6** fixture listened only on its owned Unix socket, with administrator-only
local trust and SCRAM for ordinary roles. Final audit found zero extra databases, roles, base-database
schemas, client backends or HBA errors. The server stopped cleanly; its validated data/socket
directories were removed, reclaiming about **110.8 MiB**, with logs retained. Application databases
and configured services were not used. The machine-readable selection/cleanup summary is retained
in `sql/validation-summary.json` under the external evidence directory below.

## Release identity and boundaries

Engine, Cargo/npm root locks, frontend/SDK package metadata, OpenAPI info and current README/Runner
references advance to **0.6.3**. API compatibility remains **0.3.0**. Four existing locked transport
libraries become explicit direct dependencies: Hyper 1.11.0, hyper-util 0.1.20, rustls 0.23.45 and
tokio-rustls 0.26.4. No additional package versions are resolved. Public teaching operations and SDK
surfaces remain unchanged; the dedicated platform HTTP operations are not added to SDK/OpenAPI.

The two platform hosts are explicit Linux loopback modes, not a teaching AppState replacement or
LAN deployment. Seven concrete routes share one original retained storage binding with six separate
response-lifetime admission lanes. Task content is optional; code, an editor, an assessment and a Run
are not prerequisites. Creation writes an empty manual Draft. Replacement changes a Draft title or
existing exact owned text references with an expected revision, without publishing new Artifact bytes.

Current audited membership, account generation and exact Session checks remain inside each command
transaction. Deferred effects precede complete old/new content and expected Task-pair checks; original
root/storage checks precede final fresh authority and confirmed commit. Cancellation, timeout or
lost delivery is not rollback proof or permission for retries, adoption, compensation or file deletion.
Filesystem/database checks are not an atomic transaction, continuous authorization or same-UID isolation.

Browser Task saving, new text/binary publication, catalogue/status/Run mounting, SDK integration,
automatic installation/migration, recovery and protected-LAN acceptance remain separate. Helper and
Runner browser checks use intercepted synthetic endpoints, not production authentication or privileged
kernel execution. Existing application data, configured services and historical execution records remain intact.

Tag publication triggers immutable metadata validation and the separate offline distribution workflow.
Remote CI, Docker extraction/smoke, privileged live-kernel checks, RustSec, power-loss/disk-full tests
and performance benchmarks are not established by local source validation. This workflow does not
create a GitHub Release, publish npm packages or deploy services.

Raw release evidence is retained outside the repository at `/tmp/cyanrex-release-0.6.3-x12e6hzJ`.
