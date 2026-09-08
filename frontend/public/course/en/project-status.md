# Project Status

Snapshot date: **2026-09-08**
Current release line: **0.3.4**

This page is the capability-level progress baseline for Cyanrex Lab. It records what is usable now,
what remains intentionally limited, and which decisions should drive the next development cycle.
The detailed trust boundaries and data flows remain in the [system architecture](architecture.md).

## Capability Matrix

| Area | State | Current scope |
|---|---|---|
| Identity and authorization | Operational | Argon2 passwords, TOTP, cookie sessions, CSRF origin checks, and admin/teacher/student route guards |
| eBPF workbench | Operational | Monaco editing, local Clang diagnostics/completion, bpftool execution, Aya tracepoint execution, attachments, source probes, and kernel event streaming |
| Learning workflow | Operational | Five assessed labs, persisted attempts, student progress, teacher overview, bounded source review, and student-visible teacher feedback |
| Events and persistence | Operational | User-scoped event center, FIFO retention, bounded filtered reads, explicit WebSocket lag closure and recent-history recovery, PostgreSQL storage, and documented memory/file fallbacks |
| Local Runner | Operational | Replaceable driver boundary, global/per-user leases, timeout handling, and explicit `shared_kernel` reporting |
| Runner Agent | Operational for remote checks | Signed registration, heartbeat, leases, cancellation, probes, and isolated compile-only diagnostics; remote eBPF loading is not enabled |
| Deployment and distribution | Operational | Docker, WSL2, native Linux, hardened optional compiler Agent, and offline package/install tooling |
| Release traceability | `0.3.4` version metadata prepared; artifact acceptance/publication pending | Changelog/version sync, annotated-tag preflight, checksum-bound source/archive metadata, per-image Docker content IDs, exact-image installation, and native Rust evidence/candidate verification with safe non-overwriting extraction; `0.3.0` is an API baseline only, and this snapshot does not claim an accepted `0.3.4` artifact or published tag |
| Module catalog | Operational, state-only | Versioned v1 manifests are discovered and validated at startup; lifecycle is in memory and never executes directory code |
| JavaScript SDK | Operational internal package | Typed ESM client with 57 generated non-Agent operationId calls, a 77-member additive namespace baseline and deprecation policy, explicit `/openapi` and `/operations` exports, browser/Node sessions, cancellation, downloads, typed errors, and package-consumer smoke coverage |
| API contract | Operational internal contract | Generated OpenAPI 3.1 served at `/openapi.json`; route/access/SDK/model drift and breaking changes against the frozen `0.3.0` baseline fail the quality gate |
| Terminal page | Operational for admins | Permission-aware List/Start/Stop module commands, structured results/history, and a safe handoff to the eBPF experiment workspace; it is not a shell |

## Verified Baseline

The following checks passed on the snapshot date:

- Rust formatting and the locked Engine suite: 174 tests passed; 4 tests remain ignored in the default
  gate (external PostgreSQL and Runner Agent integrations, plus two explicitly manual benchmarks).
- Next.js production build: 17 statically generated routes with TypeScript validation.
- Frontend regressions: 29 tests covering permissions, Terminal commands, teacher review/feedback, event
  recovery, performance hotspot logic, security headers, and macOS metadata cleanup; the SDK has 11 transport/operation
  regressions, a compile-time operation fixture, plus 3 package-manifest/import smoke checks.
- File-length, version/changelog/course-copy sync, OpenAPI generation/route/access/model/compatibility checks
  with 43 common tooling regressions, plus Runner Agent and distribution tooling checks.
- Production npm dependency audits and the RustSec audit passed, with no reported vulnerabilities.

This local snapshot did not start the privileged Engine or run the destructive disposable-host offline
installation smoke. The annotated-Tag candidate workflow now enables the packaged live Aya
attach/ring-buffer-event/exact-detach check and retains a candidate-bound report; its result is
environment-level evidence and is not claimed by the local checks listed above. The packaged verifier
can recheck v1/v2 schema, release metadata binding, event identity, and cleanup without kernel access;
the repository verifier additionally checks the complete downloaded artifact and can manually extract
only verified regular files without invoking `tar` or replacing an existing output.

## Event performance and recovery (2026-09-08, included in 0.3.4)

- Replaced full-history head shifts with FIFO deques and selected matching references before copying
  limited snapshots. Added isolated release benchmarks; their original source/version fingerprints remain
  unchanged by the patch bump and are not release-artifact acceptance evidence.
- WebSocket lag now closes explicitly, blocked sends time out, and both browser consumers reconnect with
  bounded snapshots, cancellation and visible possible-gap notices. Recovery is limited to retained history.
- Cookie-authenticated WebSocket handshakes also enforce the Origin/Referer policy; native clients must
  supply an allowed source. See [Event Stream Recovery](event-stream.md) for compatibility and limits.
- Separately verified nine real loopback pressure runs and a production-page Chromium smoke with mock
  HTTP/WebSocket fixtures. Global broadcast fan-out and durable cursor-based replay remain future work.
- Fixed Docker build input coverage and bounded retry handling for transient npm audit failures.

## Teaching mainline update (2026-09-05, included in 0.3.3)

- Teachers/admins can save the current comment on an existing student submission; students read it in
  **Learn → My lab history**. Multiline plain text, reviewer/time, and a 2000-character limit are supported;
  automated acceptance and the submitted source remain unchanged.
- Revision checks reject stale writes with `409`. The page retains the draft and lets the teacher load
  the latest comment before explicitly resubmitting. Failed local writes do not publish unsaved changes,
  and failed PostgreSQL writes do not write a local fallback copy.
- Added 8 backend route, 2 frontend request-builder, and 2 SDK regressions. Separately verified legacy-table
  migration, concurrent updates, and failure without fallback against a disposable UTF-8 PostgreSQL database,
  plus Chromium interactions using a mocked Engine API.
- This increment completes the teaching feedback loop without changing privileged kernel execution
  or remote Agent trust boundaries.

## LAN/desktop Runner preparation (2026-09-06, included in 0.3.3)

- Recorded the [Linux desktop/LAN isolation target](classroom-isolation.md): teaching control separate
  from exclusive student VMs, with desktop virtualization hosts supported by the design.
- Runner execution validates the lease owner. Attachment inventory, detach, and cleanup reports now
  use the selected driver, with operation deadlines and no local fallback on backend failure.
- Added ownership, lifecycle, timeout, authentication/CSRF, and local-adapter regression coverage.
- VM provisioning/loading, persistent environment ownership, full runtime/event delegation, and LAN
  ingress hardening remain pending. This increment does not make the local Engine a student sandbox.

## Compiler Runner increment (2026-09-06, included in 0.3.3)

- Check/completion routes now pass session ownership, source, selected headers, and cursor data to the
  selected Runner driver, preserving diagnostics and optional cache-status reporting without local fallback.
- Compiler capacity is per-manager (2 checks, 3 completions); operation deadlines and cancellation-safe
  metrics/permits prevent stuck requests from retaining capacity or inflating in-flight counts.
- The local adapter isolates cache keys by owner and cleans private source workspaces on return/cancellation.
  Added 4 route, 2 cache-scope, 2 workspace, and 1 required-owner regressions.
- Attach verification, event streaming, environment/settings, remote header delivery, and VM lifecycle
  remain pending; the actual runtime is still local and shared-kernel.

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
3. Implement the desktop/LAN isolation target's full runtime boundary, durable environment ownership,
   and VM recovery before enabling remote eBPF execution or Engine replicas.
4. Sign and publish accepted candidate artifacts after choosing a release trust/key ownership model.
5. Collect the first annotated-Tag live-kernel evidence, then decide whether release acceptance needs a
   dedicated self-hosted kernel-version matrix beyond the GitHub-hosted privileged Docker environment.

`engine/Cargo.toml` is the canonical release version. `scripts/check-version-sync.sh` prevents the
frontend, SDK, OpenAPI document, lockfiles, and release-facing documentation from drifting again.
