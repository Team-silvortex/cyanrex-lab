# 0.6.4 source version validation

Date: **2026-10-26**. Previous release: **v0.6.3**. This patch source release includes retained
private text publication, the default-off `--session-ui` Session/Task page, metadata-only Draft
saving, the post-0.6.3 dependency security fixes and bilingual documentation reorganization.
Code, assessment and execution remain optional Task capabilities.

## Fresh complete local quality gate

`scripts/quality-gate.sh --no-npm-install` completed successfully on **0.6.4**, reusing unchanged
installed locked dependencies. Both Rust debug profiles were zero and build parallelism was two;
release-build profiles were unchanged. Earlier 0.6.3 implementation runs were not substituted.

| Selection | Actual result |
|---|---|
| Common checks | **636 passed**, including file/version/course synchronization, generated contracts, compatibility and management-tool fixtures |
| Rust defaults and documentation | **1,451 passed**, **1,030 opt-in ignored**, 144 successful result groups; no failures or Rust warnings |
| Frontend | Next.js **15.5.28** production build and TypeScript passed; **298 tests passed** |
| SDK | Build, runtime, type and package-consumer checks passed; **42 tests passed** |
| Production npm audits | Frontend and SDK each reported **zero vulnerabilities**, with no exceptions or dependency upgrades |

Ignored cases are not passes. Frontend tests emitted 30 non-blocking `MODULE_TYPELESS_PACKAGE_JSON`
warnings; module policy was not changed to conceal them. No dependency versions were resolved during
this release increment: Cargo/npm root metadata and OpenAPI info changed, not their dependency graph.

## Fresh browser and database selections

The three actual Chrome suites each passed 12 cases, **36** in total, with zero failures or skips.
They use production compiled assets/CSP and **mock HTTP APIs**, covering Session actions, private
Task reading, metadata saving, five languages, 360px touch controls and unknown/conflict/denial states.
Synthetic page lifecycle and controlled late-fetch cases are not actual BFCache-navigation acceptance.
These mock suites do not prove server authorization or persisted Task saving.

Separately, **49 unique exact PostgreSQL selectors passed** on 0.6.4, without compiler warnings,
test panics or failures. Each was selected with `--locked`, `--ignored --list` and `--ignored --exact`.

| Boundary | Exact passes |
|---|---:|
| Real plain-browser Session / private Task reading / metadata saving | 1 / 1 / 1 |
| Retained text connector / retained HTTP / publication workspace and hosts | 8 / 6 / 6 |
| Task manifest replacement | 10 |
| Original plain / native-TLS host | 8 / 8 |

The real browser cases use actual authentication/storage, not injected Cookies or fulfilled API
responses. Independent storage checks confirm one metadata revision/event, unchanged Artifact files,
no stale overwrite and no revoked-Session write. Confirmed metadata uses the previously verified
unchanged text copy; it is not a fresh content read or a complete browser text-publication workflow.
The other selected cases cover original bindings, narrow grants, current authority, private bytes,
deferred effects, cancellation and host compatibility within their explicit fixture scope.

The PostgreSQL 18 fixture listened only on its owned Unix socket, with administrator-only trust and
ordinary-role SCRAM. Final catalog checks retained only the administrator, public schema and default
databases; other sessions, prepared transactions and advisory locks were zero. The server stopped
cleanly, and only its validated 56 MiB generated data was removed. Logs and the 49-selector inventory
remain in the external evidence directory. No existing application database or service was used.

## Release scope and omissions

Engine, Cargo/npm root locks, frontend/SDK metadata, OpenAPI info and current README/Runner examples
advance together. The public API compatibility baseline remains **0.3.0**, and no storage schema is
upgraded or installed. The dedicated platform operations remain outside the public teaching SDK.

The plain/native-TLS loopback hosts explicitly compose eight API routes/seven independent response
lanes. Optional browser hosting adds seven fixed assets without teaching AppState/authentication
fallback. The private page reads verified text and saves only an existing manual Draft's title,
display filenames and language hints, preserving Artifact pins, order, bytes and item count.
Unknown writes never authorize automatic retry or rollback; confirmed text publication and Task
replacement remain separate commands. The pure content-save helper remains unmounted.

Raw-text browser editing/publication, normal Next.js draft/SDK saving, automatic recovery, sharing,
Run/status/catalogue mounting and protected-LAN deployment are not completed by this patch.
Local real browser-TLS acceptance remains blocked by the existing legacy user NSS database.
User/system trust, HOME and certificate verification are not altered to bypass that limitation.
Remote CI, release packaging, extracted-package/kernel smoke and installed-service upgrades require
their own results. Publishing a tag does not create a GitHub Release or deploy services.
The live RustSec audit, benchmarks, power-loss/disk-full tests and remaining opt-in SQL/kernel suites
were not rerun by this local release validation.

Raw release evidence is retained outside the repository at `/tmp/cyanrex-release-0.6.4-FdyyZQQz`.
