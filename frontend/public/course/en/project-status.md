# Project Status

Snapshot date: **2026-08-31**
Current release line: **0.2.9**

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
| Module runtime | Partial | Module lifecycle is in memory and `modules/` contains examples; directories are not dynamically discovered |
| JavaScript SDK | Operational internal package | Typed ESM client with OpenAPI-generated public wire models, explicit `/openapi` type export, browser/Node session handling, cancellation, downloads, typed errors, and package-consumer smoke coverage |
| API contract | Operational internal contract | Generated OpenAPI 3.1 document served at `/openapi.json`, with exact Engine route/access, expected SDK coverage, and generated-model checks in the quality gate |
| Terminal page | Operational for admins | Permission-aware List/Start/Stop module commands, structured results/history, and a safe handoff to the eBPF experiment workspace; it is not a shell |

## Verified Baseline

The following checks passed on the snapshot date:

- Rust formatting and the locked Engine suite: 100 tests, plus the normally ignored real loopback
  Runner Agent compile integration run explicitly and passing.
- Next.js production build: 17 statically generated routes with TypeScript validation.
- Frontend regressions: 14 tests covering permissions, Terminal commands, teacher review, performance
  hotspot logic, security headers, and macOS metadata cleanup; the SDK has 6 transport regressions
  plus 3 package-manifest/import smoke checks.
- File-length, version-sync, OpenAPI generation/route/access/model-drift checks with 7 contract and
  type-generator regressions, Runner Agent tooling, distribution tooling, and both Compose profile
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

1. Decide whether `modules/` should become a versioned dynamic runtime or remain documented examples.
2. Decide whether to generate client operations from OpenAPI or retain the hand-designed namespaces,
   and define compatibility policy before publishing a stable independently consumed package.
3. Define a real isolation and ownership model before adding remote eBPF execution or Engine replicas.
4. Add release tags and a changelog so the synchronized semantic version is tied to an auditable release.
5. Promote live kernel attach/stream and extracted-distribution smoke checks to a documented release
   acceptance environment.

`engine/Cargo.toml` is the canonical release version. `scripts/check-version-sync.sh` prevents the
frontend, SDK, OpenAPI document, lockfiles, and release-facing documentation from drifting again.
