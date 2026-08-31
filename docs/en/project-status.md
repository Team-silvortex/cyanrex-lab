# Project Status

Snapshot date: **2026-08-31**
Current release line: **0.3.1**

This page is the capability-level progress baseline for Cyanrex Lab. It records what is usable now,
what remains intentionally limited, and which decisions should drive the next development cycle.
The detailed trust boundaries and data flows remain in the [system architecture](architecture.md).

## Capability Matrix

| Area | State | Current scope |
|---|---|---|
| Identity and authorization | Operational | Argon2 passwords, TOTP, cookie sessions, CSRF origin checks, and admin/teacher/student route guards |
| eBPF workbench | Operational | Monaco editing, local Clang diagnostics/completion, bpftool execution, Aya tracepoint execution, attachments, source probes, and kernel event streaming |
| Learning workflow | Operational | Five assessed labs, persisted attempts, student progress, teacher overview, and bounded source review |
| Events and persistence | Operational | User-scoped WebSocket/event center, retention policies, PostgreSQL storage, and documented memory/file fallbacks |
| Local Runner | Operational | Replaceable driver boundary, global/per-user leases, timeout handling, and explicit `shared_kernel` reporting |
| Runner Agent | Operational for remote checks | Signed registration, heartbeat, leases, cancellation, probes, and isolated compile-only diagnostics; remote eBPF loading is not enabled |
| Deployment and distribution | Operational | Docker, WSL2, native Linux, hardened optional compiler Agent, and offline package/install tooling |
| Release traceability | Baseline established | Changelog/version sync, annotated `v0.2.9`/`v0.3.1` targets, and preflight plus automatic validation for future tags; `0.3.0` is an API baseline only and publication remains manual |
| Module catalog | Operational, state-only | Versioned v1 manifests are discovered and validated at startup; lifecycle is in memory and never executes directory code |
| JavaScript SDK | Operational internal package | Typed ESM client with OpenAPI-generated wire models and all 56 non-Agent operationId calls, stable task namespaces, explicit `/openapi` and `/operations` exports, browser/Node sessions, cancellation, downloads, typed errors, and package-consumer smoke coverage |
| API contract | Operational internal contract | Generated OpenAPI 3.1 served at `/openapi.json`; route/access/SDK/model drift and breaking changes against the frozen `0.3.0` baseline fail the quality gate |
| Terminal page | Operational for admins | Permission-aware List/Start/Stop module commands, structured results/history, and a safe handoff to the eBPF experiment workspace; it is not a shell |

## Verified Baseline

The following checks passed on the snapshot date:

- Rust formatting and the locked Engine suite: 105 tests, plus the normally ignored real loopback
  Runner Agent compile integration run explicitly and passing.
- Next.js production build: 17 statically generated routes with TypeScript validation.
- Frontend regressions: 14 tests covering permissions, Terminal commands, teacher review, performance
  hotspot logic, security headers, and macOS metadata cleanup; the SDK has 9 transport/operation
  regressions, a compile-time operation fixture, plus 3 package-manifest/import smoke checks.
- File-length, version/changelog sync, OpenAPI generation/route/access/model/compatibility checks with 18
  contract, compatibility, and schema/operation-generator regressions, Runner Agent tooling, distribution tooling, and both Compose profile
  configuration checks.
- Production dependency audits: zero npm vulnerabilities and zero RustSec findings.

This snapshot did not start the privileged Engine to perform a live kernel attach/stream, and it did
not run the destructive disposable-host offline installation smoke. Those remain environment-level
acceptance checks in CI or a dedicated Linux host.

## Intentional Boundaries

- The privileged Engine is for trusted self-hosted teaching environments, not public multi-tenancy.
- Local execution shares one Linux kernel; quotas are resource controls, not student isolation.
- Engine state is single-process. Agent registration, remote jobs, attachments, and module lifecycle
  do not yet have a multi-replica coordination model.
- Aya currently covers the supported tracepoint path; bpftool remains the broad compatibility path.
- Remote Agents compile and diagnose only. `/ebpf/run` remains local.

## Next Decision Points

1. Define a signed, isolated executable-module adapter and durable ownership model before extending
   the state-only module catalog into a process or library plugin runtime.
2. Define the public stability and deprecation cadence for the additive generated operationId layer
   and hand-designed namespaces before publishing a stable independently consumed SDK package.
3. Define a real isolation and ownership model before adding remote eBPF execution or Engine replicas.
4. Automate signed release publication and attach checksums/provenance to each tagged distribution.
5. Promote live kernel attach/stream and extracted-distribution smoke checks to a documented release
   acceptance environment.

`engine/Cargo.toml` is the canonical release version. `scripts/check-version-sync.sh` prevents the
frontend, SDK, OpenAPI document, lockfiles, and release-facing documentation from drifting again.
