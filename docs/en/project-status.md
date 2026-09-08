# Project Status

Snapshot date: **2026-09-08**
Current release line: **0.3.5**

This page is the capability-level progress baseline for Cyanrex Lab. It records what is usable now,
what remains intentionally limited, and which decisions should drive the next development cycle.
The detailed trust boundaries and data flows remain in the [system architecture](architecture.md).

## Capability Matrix

| Area | State | Current scope |
|---|---|---|
| Identity and authorization | Operational | Argon2 passwords, TOTP, cookie sessions, CSRF origin checks, and admin/teacher/student route guards |
| eBPF workbench | Operational | Monaco editing, local Clang diagnostics/completion, bpftool execution, Aya tracepoint execution, attachments, source probes, and kernel event streaming |
| Learning workflow | Operational | Five assessed labs, persisted attempts, student progress, teacher overview, bounded source review, and student-visible teacher feedback |
| Events and persistence | Operational | User-scoped event center and live queues, shared lazy event JSON, FIFO retention, bounded filtered reads, explicit WebSocket lag closure and recent-history recovery, PostgreSQL storage, and documented memory/file fallbacks |
| Local Runner | Operational | Replaceable driver boundary, global/per-user leases, timeout handling, and explicit `shared_kernel` reporting |
| Runner Agent | Operational for remote checks | Signed registration, heartbeat, leases, cancellation, probes, and isolated compile-only diagnostics; remote eBPF loading is not enabled |
| Deployment and distribution | Operational | Docker, WSL2, native Linux, hardened optional compiler Agent, and offline package/install tooling |
| Release traceability | `0.3.5` version metadata prepared; artifact acceptance/publication pending | Changelog/version sync, annotated-tag preflight, checksum-bound source/archive metadata, per-image Docker content IDs, exact-image installation, and native Rust evidence/candidate verification with safe non-overwriting extraction; `0.3.0` is an API baseline only, and this snapshot does not claim an accepted `0.3.5` artifact or published tag |
| Module catalog | Operational, state-only | Versioned v1 manifests are discovered and validated at startup; lifecycle is in memory and never executes directory code |
| JavaScript SDK | Operational internal package | Typed ESM client with 57 generated non-Agent operationId calls, a 77-member additive namespace baseline and deprecation policy, explicit `/openapi` and `/operations` exports, browser/Node sessions, cancellation, downloads, typed errors, and package-consumer smoke coverage |
| API contract | Operational internal contract | Generated OpenAPI 3.1 served at `/openapi.json`; route/access/SDK/model drift and breaking changes against the frozen `0.3.0` baseline fail the quality gate |
| Terminal page | Operational for admins | Permission-aware List/Start/Stop module commands, structured results/history, and a safe handoff to the eBPF experiment workspace; it is not a shell |

## Verified Baseline

The following working-tree checks passed on the snapshot date, including the increments recorded in 0.3.5 below:

- Rust formatting and the locked Engine suite: 205 tests passed; 5 tests remain ignored in the default
  gate (external PostgreSQL and Runner Agent integrations, plus three explicitly manual benchmarks).
- Next.js production build: 17 statically generated routes with TypeScript validation.
- Frontend regressions: 29 tests covering permissions, Terminal commands, teacher review/feedback, event
  recovery, performance hotspot logic, security headers, and macOS metadata cleanup; the SDK has 11 transport/operation
  regressions, a compile-time operation fixture, plus 3 package-manifest/import smoke checks.
- File-length, version/changelog/course-copy sync, OpenAPI generation/route/access/model/compatibility checks
  with 45 common tooling regressions, plus Runner Agent and distribution tooling checks.
- Production npm dependency audits and the RustSec audit passed, with no reported vulnerabilities.

This local snapshot did not start the privileged Engine or run the destructive disposable-host offline
installation smoke. The annotated-Tag candidate workflow now enables the packaged live Aya
attach/ring-buffer-event/exact-detach check and retains a candidate-bound report; its result is
environment-level evidence and is not claimed by the local checks listed above. The packaged verifier
can recheck v1/v2 schema, release metadata binding, event identity, and cleanup without kernel access;
the repository verifier additionally checks the complete downloaded artifact and can manually extract
only verified regular files without invoking `tar` or replacing an existing output.

## Streaming local learning commits (2026-09-08, included in 0.3.5)

- Local writes stream byte-compatible pretty JSON through a 64 KiB buffer on a blocking worker,
  explicitly flush before rename, then publish memory. No full encoded-file buffer is allocated;
  the complete file is still rewritten and source snapshots/SQL/HTTP/SDK contracts remain unchanged.
- Admission is acquired before dispatch; one queued/running worker per store retains the write lock
  after request cancellation through final publication. This fixes disk/memory divergence and stale
  subsequent overwrites. Cancellation before handoff remains harmless; after handoff a save may finish
  without a response, so reload before retrying. No append idempotency or crash/power-loss guarantee is added.
- Added 9 Rust regressions covering cancellation before admission, blocking-pool queuing, cancellation
  after rename for append/feedback, failed detached commits, streaming, exact bytes, interrupted/partial
  writes and flush failures. The full security-enabled quality gate passed.
- A fresh 72-run paired disk matrix compares with the preceding snapshot optimization, not the clean
  release. At 50,000 rows / 1,112-byte source, write mean was 144.6 → 111.3 ms and p95 152.5 → 122.1 ms.
  At 10,000 rows, whole-process write peak RSS fell 63.2 → 39.2 MiB. Small-write mean/p50 and some unchanged
  read controls regressed; large-source disk tails varied widely, so this is not a uniform speedup claim.
- Source-bound raw measurements are in `reports/benchmarks/2026-09-08-learning-streaming-writes`, with
  a separate 72-run tmpfs control. See [Learning Record Storage](learning-storage.md) for cancellation,
  retry and shutdown behavior. Fsync, recovery logs, incremental persistence and cross-process coordination
  remain future work. Benchmark source/version fingerprints remain unchanged by the patch bump and are
  not release-artifact acceptance evidence.

## LearningStore snapshots and queries (2026-09-08, included in 0.3.5)

- Local readers share immutable snapshots. Recent review selects bounded references before copying
  response records; progress and teacher overview aggregate in one pass without cloning submitted source.
  Appends copy the pointer index; feedback additionally copies only the edited record, preserving old readers.
- Added 12 Rust regressions and 2 benchmark-ordering regressions; the full security-enabled gate passed.
  Plain pretty JSON, legacy records, SQL queries, authorization, revision conflicts and response schemas
  remain compatible. Write/rename failure, concurrent updates and reload behavior have regression coverage.
- In a corrected-order 72-run disk comparison, 50,000 records with 1,112-byte source reduced recent-20
  p95 from 1.744 to 0.220 ms and teacher overview from 41.907 to 4.446 ms; write-process peak RSS fell
  from 296.3 to 180.1 MiB. These are warm local-service measurements, not HTTP or classroom capacity.
- Disk writes are not uniformly faster: 50,000 records with 4,184-byte source increased mean write
  latency by 28%. A separate 72-run tmpfs control improved writes but does not establish durable-disk
  performance. The initial 72-run ordering-defect dataset is retained separately, not merged or replaced.
- This measurement stage still used a complete encoded-file buffer and cancellable rename/publication;
  the follow-up above replaces those mechanisms without rewriting these historical measurements.
  Whole-file replacement, unbounded resident records, pointer-index copying and old reader snapshots
  remain, without new fsync, recovery logs or cross-process coordination.
- See [Learning Record Storage](learning-storage.md) and
  `reports/benchmarks/2026-09-08-learning-snapshots-final` for evidence and remaining limits.

## Owner-scoped event distribution (2026-09-08, included in 0.3.5)

- `/ws/events` uses the authenticated owner's bounded live queue. Another user's burst cannot evict
  this owner's pending events; own-overload closure, authentication/Origin checks and raw JSON remain
  compatible. The global EventBus service subscription API remains available.
- Same-owner connections share immutable events and one lazy JSON encoding. Last-connection drop or
  cancellation removes the queue, with generation-safe cleanup during simultaneous reconnections.
- Added 10 regressions and passed the full security-enabled quality gate. A final 30-run release-mode
  experiment (24 current-path comparisons and 6 no-subscriber binary controls) found 24–64% higher
  one-owner and 40–56% higher eight-owner service/frame-construction throughput at equal matching
  output. Nine separate real-loopback checks preserved delivery, explicit overload and five-second
  stalled-peer cleanup. The regressing uncached prototype's evidence is retained, not overwritten.
- These are local synthetic results, not LAN/kernel or release acceptance. Queue capacity is per active
  owner; aggregate memory and cached JSON costs grow with active owners/payloads. Shared locks,
  persistence and CPU remain shared; global admission limits and durable replay are not implemented.
- Details and reproducible commands are in [Event Stream Recovery](event-stream.md); measurements are
  under `reports/benchmarks/2026-09-08-owner-fanout-final` and `2026-09-08-owner-event-stream`.

## Event performance and recovery (2026-09-08, included in 0.3.4)

- Replaced full-history head shifts with FIFO deques and selected matching references before copying
  limited snapshots. Added isolated release benchmarks; their original source/version fingerprints remain
  unchanged by the patch bump and are not release-artifact acceptance evidence.
- WebSocket lag now closes explicitly, blocked sends time out, and both browser consumers reconnect with
  bounded snapshots, cancellation and visible possible-gap notices. Recovery is limited to retained history.
- Cookie-authenticated WebSocket handshakes also enforce the Origin/Referer policy; native clients must
  supply an allowed source. See [Event Stream Recovery](event-stream.md) for compatibility and limits.
- Separately verified nine real loopback pressure runs and a production-page Chromium smoke with mock
  HTTP/WebSocket fixtures. Those 0.3.4 measurements used global broadcast; the 0.3.5 increment above
  adds owner-scoped distribution. Durable cursor-based replay remains future work.
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
