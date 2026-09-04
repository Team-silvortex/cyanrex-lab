# Project Status

Snapshot date: **2026-09-04**
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
| Release traceability | Accepted unsigned candidates | Changelog/version sync, annotated `v0.2.9`/`v0.3.1` targets, immutable-Tag validation, checksum-bound source/archive metadata, per-image Docker content IDs, exact-image installation, and a safe streaming whole-bundle verifier that cross-binds strictly validated live Aya evidence before 30-day workflow retention; `0.3.0` is an API baseline only and signed publication remains manual |
| Module catalog | Operational, state-only | Versioned v1 manifests are discovered and validated at startup; lifecycle is in memory and never executes directory code |
| JavaScript SDK | Operational internal package | Typed ESM client with 56 generated non-Agent operationId calls, a 77-member additive namespace baseline and deprecation policy, explicit `/openapi` and `/operations` exports, browser/Node sessions, cancellation, downloads, typed errors, and package-consumer smoke coverage |
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
- File-length, version/changelog/course-copy sync, OpenAPI generation/route/access/model/compatibility checks with 28
  contract, compatibility, and schema/operation-generator regressions, Runner Agent tooling, distribution tooling, and both Compose profile
  configuration checks.
- Production dependency audits: zero npm vulnerabilities and zero RustSec findings.

This local snapshot did not start the privileged Engine or run the destructive disposable-host offline
installation smoke. The annotated-Tag candidate workflow now enables the packaged live Aya
attach/ring-buffer-event/exact-detach check and retains a candidate-bound report; its result is
environment-level evidence and is not claimed by the local checks listed above. The packaged verifier
can recheck v1/v2 schema, release metadata binding, event identity, and cleanup without kernel access;
the repository verifier additionally checks the complete downloaded artifact without extracting it.

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
2. Define registry publication, support ownership, and 1.0 readiness criteria before making the SDK
   a stable independently consumed package.
3. Define a real isolation and ownership model before adding remote eBPF execution or Engine replicas.
4. Sign and publish accepted candidate artifacts after choosing a release trust/key ownership model.
5. Collect the first annotated-Tag live-kernel evidence, then decide whether release acceptance needs a
   dedicated self-hosted kernel-version matrix beyond the GitHub-hosted privileged Docker environment.

`engine/Cargo.toml` is the canonical release version. `scripts/check-version-sync.sh` prevents the
frontend, SDK, OpenAPI document, lockfiles, and release-facing documentation from drifting again.
