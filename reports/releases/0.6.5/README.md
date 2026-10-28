# 0.6.5 source version validation

Release date: **2026-10-28**. Full local validation: **2026-10-26**. Previous release: **v0.6.4**.
This patch source release adds Task-bound
private text publication, a strict independent HTTP adapter and default-off loopback/native-TLS
host composition. It also repairs a reproduced metadata-browser test observation gap. Code and
execution remain optional Task payload capabilities, not the core of the collaboration platform.

## Fresh complete local quality gate

`scripts/quality-gate.sh --no-npm-install` completed successfully on **0.6.5**, reusing unchanged
installed locked dependencies. Both Rust debug profiles were zero and build parallelism was two;
release-build profiles were unchanged. Earlier implementation or 0.6.4 results were not substituted.

| Selection | Actual result |
|---|---|
| Common checks | **650 passed**, including file/version/course synchronization, generated contracts, compatibility and management-tool fixtures |
| Rust defaults and documentation | **1,483 passed**, **1,049 opt-in ignored**, 148 successful result groups; no failures or Rust warnings |
| Frontend | Next.js **15.5.28** production build and TypeScript passed; **298 tests passed** |
| SDK | Build, runtime, type and package-consumer checks passed; **42 tests passed** |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, with no exceptions or dependency upgrades |

Ignored cases are not passes. Frontend tests emitted 30 non-blocking `MODULE_TYPELESS_PACKAGE_JSON`
warnings; module policy was not changed to conceal them. Cargo/npm lock changes update only this
project's root version metadata. The dependency graph, public schemas and compatibility baseline
remain unchanged. Management-tool fixtures are not real package or live-kernel acceptance.

On **2026-10-28**, the final common/format gate again passed all **650** checks, and both production
npm audits again reported zero vulnerabilities. A preceding restricted-context common run failed
three test files; its log is retained rather than rewritten. All three passed a separate **22**-test
focused run and the complete native common gate without changing source or test assertions.

## Fresh browser and database selections

Three actual Chrome suites each passed 12 cases, **36** in total, with zero failures, skips or
retries. They use production assets/CSP and **mock HTTP APIs**, covering Session actions, private
Task reading, metadata saving, five languages, 360px touch controls and unknown/conflict/denial
states. Controlled late-fetch and synthetic lifecycle cases are not actual BFCache-navigation
acceptance. These suites do not prove SQL authorization or browser text-body publication.

Separately, **68 unique exact selectors passed** on 0.6.5: **65 database/host cases and three real
plain-browser cases**, without failures, Rust warnings or retries. Each target was listed using
`--locked --ignored --list`; selectors were run using `--locked --ignored --exact`, with explicit
inventory, selection and pass counts reconciled.

| Boundary | Exact passes |
|---|---:|
| Task-bound text connector / independent HTTP / opt-in publication hosts | 7 / 6 / 6 |
| Existing retained text connector / retained HTTP / publication hosts | 8 / 6 / 6 |
| Task manifest replacement | 10 |
| Original plain / native-TLS host | 8 / 8 |
| Real plain-browser Session / private Task reading / metadata saving | 1 / 1 / 1 |

The real browser cases use actual authentication/storage, not injected Cookies or fulfilled API
responses. The retained HTTP count is its explicit six-case runner, not every pooled HTTP case.
These selections exercise original storage bindings, Task baseline drift, current authority,
private bytes, deferred effects, cancellation and compatibility within their fixture scope.
The metadata fixture binds each explicit GET/PUT response before dispatch and drains its JSON
observation; reproducing that gap does not establish the earlier remote CI failure's unique cause.

The PostgreSQL 18 fixture listened only on its owned Unix socket, with administrator-only trust
and ordinary-role SCRAM. Final catalog checks retained only the initial administrator/default
databases, with no other client connections, backend locks or waiting locks. The server stopped
cleanly and its exited PID was checked before removing only the validated generated data directory,
reclaiming **58,089,472 bytes (55.40 MiB)**. Logs, inventory and cleanup evidence remain outside the
repository. No existing application database or service was used.

## Release scope and omissions

Engine, Cargo/npm root locks, frontend/SDK metadata, OpenAPI info and current README/Runner examples
advance together. Public API compatibility remains **0.3.0**. No storage schema or grants are
automatically installed; dedicated platform operations remain outside the public teaching SDK.

Task-bound publication pins the complete existing owned manual Draft and current Session authority
before new bytes, then confirms the new Artifact and unchanged Task before commit. It does not
change Task metadata, attach the Artifact, verify old payload bytes or provide whole-draft atomicity.
Unknown outcomes retain uncertainty; they do not authorize retry, cleanup or recovery.

The explicit `--task-text-publication` serve option composes nine API routes/eight response lanes
over one original storage binding. Default CLI hosts retain eight routes/seven lanes; older
constructors retain seven/six. The two text capabilities each have four actual-worker permits,
independent of response admission and the unchanged global 16-connection limit. The optional
Session UI still has seven fixed assets and saves only existing Draft display metadata, not bodies.

Raw-text browser editing/publication, normal Next.js/SDK saving, automatic recovery, sharing,
Run/status/catalogue mounting and protected-LAN deployment are not completed by this patch.
Local real browser-TLS acceptance remains blocked by the existing legacy user NSS database;
native private-CA TLS host cases passed separately. User/system trust, HOME and certificate
verification were not altered to bypass that limitation.

Remote CI, release packaging, extracted-package/kernel smoke and installed-service upgrades require
their own results. Publishing the source tag does not create a GitHub Release or deploy services.
The live RustSec audit, benchmarks, power-loss/disk-full tests and remaining opt-in SQL/kernel
suites were not rerun by this local release validation. Historical reports were not rewritten.

Raw release evidence is retained at `/tmp/cyanrex-release-0.6.5-ROwlvI`, including
`quality-gate-full.log`, the three `mock-*.log` files and `sql-6Psi69/verification-summary.json`,
`selector-inventory.json`, `database-baseline.json`, `cleanup.json` and per-runner logs.
