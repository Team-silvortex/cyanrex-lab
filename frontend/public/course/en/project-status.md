# Project Status

Snapshot date: **2026-09-24**
Current release line: **0.4.0**

This page is the capability-level progress baseline for Cyanrex Lab. It records what is usable now,
what remains intentionally limited, and which decisions should drive the next development cycle.
The detailed trust boundaries and data flows remain in the [system architecture](architecture.md).

## Capability Matrix

| Area | State | Current scope |
|---|---|---|
| Identity and authorization | Operational | Argon2 passwords, TOTP, cookie sessions, CSRF origin checks, teacher teaching/deployment authority and student guards; legacy admin compatibility |
| eBPF workbench | Operational | Monaco editing, local Clang diagnostics/completion, bpftool execution, Aya tracepoint execution, attachments, source probes, and kernel event streaming |
| Learning workflow | Operational | Five assessed labs, persisted attempts, student progress, teacher overview, bounded source review, student-visible teacher feedback, and confirmed resume from a previous submission |
| Events and persistence | Operational | User-scoped event center and live queues, shared lazy event JSON, FIFO retention, bounded filtered reads, explicit WebSocket lag closure and recent-history recovery, PostgreSQL storage, and documented memory/file fallbacks |
| Local Runner | Operational | Replaceable driver boundary, global/per-user leases, timeout handling, and explicit `shared_kernel` reporting |
| Runner Agent | Operational for remote checks | Signed registration, heartbeat, leases, cancellation, probes, and isolated compile-only diagnostics; remote eBPF loading is not enabled |
| Deployment and distribution | Operational | Docker, WSL2, native Linux, hardened optional compiler Agent, and offline package/install tooling |
| Release traceability | `0.4.0` synchronized source metadata; artifact acceptance remains separate | Changelog/version sync, annotated-tag preflight, checksum-bound source/archive metadata, per-image Docker content IDs, exact-image installation, and native Rust evidence/candidate verification with safe non-overwriting extraction; `0.3.0` is an API baseline only; publishing a source tag does not establish artifact acceptance |
| Module catalog | Operational, state-only | Versioned v1 manifests are discovered and validated at startup; lifecycle is in memory and never executes directory code |
| JavaScript SDK | Operational internal package | Typed ESM client with 63 generated non-Agent operationId calls, a 77-member additive namespace baseline and deprecation policy, explicit `/openapi` and `/operations` exports, browser/Node sessions, cancellation, downloads, typed errors, and package-consumer smoke coverage |
| API contract | Operational internal contract | Generated OpenAPI 3.1 served at `/openapi.json`; route/access/SDK/model drift and breaking changes against the frozen `0.3.0` baseline fail the quality gate |
| Terminal page | Operational for teachers | Permission-aware List/Start/Stop module commands, structured results/history, and a safe handoff to the eBPF experiment workspace; it is not a shell |

## Event workflow and build footprint (included in 0.4.0)

- [Pass 07](functional-network-bug-hunt-07.md) fixes event filtering, export/deletion admission,
  navigation cancellation, read acknowledgements and unread polling, with explicit four-locale errors.
- [Pass 08](functional-network-bug-hunt-08.md) fixes SQL/JSON persistence and serializes publications,
  deletion, replacement and retention through bounded queues, owner admission and transactions.
  PostgreSQL fault, ordering and cancellation regressions now run explicitly in CI.
- Development/test builds retain line-level backtraces with smaller debug information and no incremental
  cache; generated TypeScript build metadata is no longer tracked. Release build settings are unchanged.
- Original maps and bug-hunt evidence retain their recorded 0.3.8/0.3.9 baselines, not 0.4.0 artifact
  acceptance. Teacher authority, API compatibility and running deployments remain unchanged; durable
  event replay, cross-process coordination and current-version LAN/kernel/distribution acceptance remain separate.

## Functional-network bug fixes (included in 0.3.9)

- The [functional-network map](functional-network.md) enumerates 12 modules, 40 workflows and 69 links.
  Six linked bug-hunt passes cover learning reads/private commits, Runner Agent capacity/leases/body
  limits, compiler diagnostics, editor/header ownership, run/detach races and breakpoint/event recovery.
- The original map, source fingerprints and six reports retain their 0.3.8 pre-release inputs. Version
  and repaired-source drift against that frozen inventory is expected; it is not a refreshed 0.3.9 snapshot.
- Teacher authority, owner-scoped access, explicit confirmations and API compatibility remain unchanged.
  Local regressions do not establish current-version LAN/TLS, SSH deployment, live-kernel or offline
  artifact acceptance, and this source release does not modify a running deployment.

## Authentication and persistence fixes (included in 0.3.8)

- All nine findings from the [module/boundary network](testing-network.md) are fixed: four auth,
  three local-script consistency, and two destructive event-filter cases. Login/session creation and
  account deletion require confirmed transactional state; local scripts commit atomically before cache
  publication; event deletion removes matches, rejects unsafe filters, and preserves SQL-only rows.
- Registration/TOTP encoding, observed session revocation, normalized account mutations and four-locale
  logout error/retry handling are covered. Auth, event and script SQL boundaries now run explicitly in CI.
- Pre-bump verification on 2026-09-09 passed 290 distinct Rust cases, 35 mocked-Engine browser cases,
  44 frontend units, 55 tooling cases and 16 SDK/package cases; two performance-only tests were not run.
  [Original reports](acceptance.md) retain their 0.3.7 inputs/hashes, not 0.3.8 artifact acceptance claims.
- Real LAN/TLS onboarding, current-source kernel acceptance, actual SSH deployment and asynchronous
  event queue/restart/failover validation remain separate. No running deployment is changed by this patch.

## SSH and classroom entry (2026-09-09, included in 0.3.7)

- Native SSH plan/apply manages pre-installed offline packages, with strict host verification,
  target-bound confirmation, exact version/control-file checks and no retries or volume deletion.
- Opt-in `/join` and minimal discovery metadata support teacher-approved student enrollment through
  name-bound, single-use 10-minute invitations. Password/TOTP, confirmed teacher origins, CSRF,
  independent protocol/capability checks, revocation and no-store handling are retained.
- This is link-based discovery, not mDNS or network scanning. Package upload, bare-host installation,
  certificate/ingress setup, classroom member removal and VM isolation remain pending.
- OpenAPI/SDK now cover 68 Engine operations and 63 non-Agent generated/convenience operations.
  Frontend builds accept an explicit Engine origin; old images require rebuilding for LAN use.
  See [Classroom Connection](classroom-connection.md) for configuration and acceptance boundaries.
- Verification: security-enabled full gate plus final backend regressions passed (233 Rust tests,
  5 default-ignored integrations/benchmarks; 41 frontend regressions; 51 common checks; 13 SDK tests,
  type checks and 3 package checks; 18 production routes). All 27 optional Chromium cases passed.
  The first RustSec fetch failed and was not counted; subsequent RustSec/npm audits reported no
  vulnerabilities. Compose configuration used synthetic inputs. No real SSH host, deployment,
  network exposure, credentials or VM was changed; current SSH/browser coverage is synthetic.

## Teacher authority (2026-09-09, earlier increment included in 0.3.7)

- The seeded personal-use account and legacy administrator allowlist now return `teacher`. Teachers
  manage modules/headers, settings, Runner Agents and Terminal without a separate admin session;
  students remain excluded from management. Review teacher allowlists before upgrading: all entries
  now confer full deployment authority. Credentials and historical records are not rewritten.
- Public registration cannot claim reserved teacher/legacy-admin names or choose elevated roles.
  The deployment owner cannot delete itself. TOTP, CSRF, owner-bound data and confirmations remain.
- Docker, offline Compose and native/WSL pass both role allowlists. Four-locale navigation identifies
  teacher/student authority, and OpenAPI retains legacy category identifiers with truthful role lists.
- The security-enabled quality gate passed: 218 Rust tests (5 default-ignored integrations/benchmarks),
  38 frontend regressions, 49 common checks, SDK checks and 17 production routes. All 24 optional
  Chromium cases passed, including teacher management with confirmations, student navigation and
  four-locale narrow layouts. Browser Engine responses are synthetic; no deployment was changed.
- RustSec and production npm audits passed; the first RustSec fetch failed on a network error and
  was not counted as a result. Both Compose configurations were validated with synthetic inputs only.
- This is role/ownership consolidation, not a VM Runner, remote kernel loading, classroom enrollment,
  browser role editor or deployment restart. Teacher-owned control remains the LAN architecture target.
  See [Teacher Guide](teacher-guide.md) and [Classroom Isolation](classroom-isolation.md).

## Verified 0.3.6 Baseline

The following checks cover the confirmation, layout, submission-resume and cold-load increments now
included in 0.3.6 and the changes recorded in 0.3.5 below. Browser, real-integration, kernel and benchmark
evidence was collected before the version bump; its original source and binary fingerprints remain
unchanged and do not represent acceptance of a 0.3.6 release artifact.

- Rust formatting and the locked Engine suite: 213 tests passed; 5 tests remain ignored in the default
  gate (external PostgreSQL and Runner Agent integrations, plus three explicitly manual benchmarks).
- Next.js production build: 17 statically generated routes with TypeScript validation.
- Frontend regressions: 37 tests covering permissions, Terminal commands, teacher review/feedback, submission
  resume, safe action scopes, event recovery, performance hotspot logic, security headers, and macOS metadata cleanup; the SDK has 12 transport/operation
  regressions, a compile-time operation fixture, plus 3 package-manifest/import smoke checks.
- Optional Chromium UI regressions: all 22 passed against the updated production build. The 12 safety
  cases also passed in development Strict Mode before the dependency update. Four-locale 320 px bounds
  passed automated checks, and the final Chinese dialog screenshot was visually checked.
- File-length, version/changelog/course-copy sync, OpenAPI generation/route/access/model/compatibility checks
  with 47 common tooling regressions, plus Runner Agent and distribution tooling checks.
- Production npm dependency audits and the RustSec audit passed, with no reported vulnerabilities.
  Next.js 15.5.24 and sharp 0.35.4 (bundled libheif 1.23.2) replace the earlier vulnerable frontend lock.
  Existing running deployments require a rebuild; see the [security guide](security.md).

The portable quality gate did not start a privileged Engine or run the destructive disposable-host offline
installation smoke. Separate source-level kernel acceptance ran inside an explicitly approved disposable
VM, as recorded below; it does not establish release-artifact acceptance. The annotated-Tag candidate workflow enables the packaged live Aya
attach/ring-buffer-event/exact-detach check and retains a candidate-bound report; its result is
environment-level evidence and is not claimed by the local checks listed above. The packaged verifier
can recheck v1/v2 schema, release metadata binding, event identity, and cleanup without kernel access;
the repository verifier additionally checks the complete downloaded artifact and can manually extract
only verified regular files without invoking `tar` or replacing an existing output.

## Disposable-VM kernel acceptance (2026-09-09, included in 0.3.6)

- Created an explicitly approved, resource-bounded QEMU/KVM guest from a signature/checksum-verified
  Ubuntu image. Package setup was followed by a shutdown/restart into restricted user networking;
  a dedicated canary confirmed guest-to-host access was blocked while loopback SSH remained usable.
- The pre-bump native dev snapshot passed real Aya attach → matched ring-buffer event → exact detach
  on Ubuntu 24.04.5 / Linux 6.8.0-139. No bpffs pins remained; kernel program/link ID inventories were
  unchanged before/after, and the Rust evidence verifier passed in the guest and on the host.
- The VM and test processes are stopped; existing VMs and deployments were not changed. Retained
  source/binary hashes and `candidate: null` evidence are described in [Acceptance checklist](acceptance.md).
  This does not ship a VM Runner, prove multi-student isolation, or accept an offline package/Tag/LAN flow.

## First real integration acceptance (2026-09-09, included in 0.3.6)

- Explicitly ran both external integrations skipped by the default gate: one passed against disposable
  PostgreSQL 16.14 and one passed using real loopback HTTP, signed Runner Agent requests and Clang 18.
- PostgreSQL coverage now includes owner-bound historical submission reads, current teacher feedback,
  missing/foreign-owner records and read failures without local fallback, alongside legacy migration,
  concurrent revision conflicts and persistence. This is service-level SQL acceptance, not a full browser flow.
- Rust CI now provisions its own loopback-only PostgreSQL service and requires the exact ignored test
  to exist before running it. A common regression checks this wiring. The configuration was checked
  locally; the recorded PostgreSQL result is local acceptance, not a remote CI result.
- The temporary database was removed; existing containers, data and kernel attachments were left alone.
  This does not establish VM isolation, LAN security, packaged installation or live-kernel acceptance.
  Evidence, reproduction and the remaining gates are in [Acceptance checklist](acceptance.md).

## Consequential-action confirmations (2026-09-09, included in 0.3.6)

- Kernel runs, detach, script/header/event deletion, draft replacement, remote compiler selection and
  consequential account/admin controls share a target/impact review with Cancel focused by default.
  Bulk cleanup requires an exact phrase. Duplicate clicks cannot resubmit a pending action; failures
  require inspecting state and a fresh confirmation instead of an automatic retry. Read-only work stays direct.
- Single detach requires an exact path and never falls back to all. Event deletion freezes its absolute
  cutoff and explains that all matching records are affected, not just 200 visible rows. Header batches
  stop at the first failure and report completed items without pretending to roll back earlier deletions.
- Entering a lab preserves the draft; **Load lab template** is explicit and separate from running.
  Navigation discards unconfirmed actions and late local file imports. Source transfer to a remote
  diagnostic Agent also needs consent. Already-dispatched Engine mutations are not undone by navigation.
- Three pure regressions cover exact detach targeting and deletion scopes; twelve optional Chromium
  checks (`npm --prefix frontend run test:safety-browser`) cover keyboard focus, cancellation, duplicates,
  failures, target binding, draft/navigation races, four locales and 320 px dialogs. Development Strict
  Mode replay also preserves confirmation/focus. Tests use synthetic Engine responses, not real account,
  database, kernel or LAN changes. Existing server authorization and runtime boundaries are unchanged.

## Task-focused interface (2026-09-08, included in 0.3.6)

- Desktop navigation stays available while scrolling; compact screens use an expandable menu with
  Escape dismissal, active-page semantics and a skip link, retaining the same role-filtered routes.
- The editor groups import/save/run in a sticky action bar, places diagnostics and results directly
  below source, and puts runtime settings and attachment cleanup alongside on wide screens. Section
  shortcuts work on smaller screens; saved scripts, metadata, headers and breakpoint details expand
  without remounting Monaco. The same 1440 × 900 synthetic case moves source from about 623 to 315 px
  below the page top. No runtime, session or API contract changes are introduced by the layout work.
- Learning progress precedes reference material, with direct history/resource links. Classroom summaries
  are compact, roster rows become labelled narrow-screen cards, and selecting a student focuses the review.
- Five optional Chromium layout checks (`npm --prefix frontend run test:layout-browser`) cover source
  position, sticky actions, 320–1440 px widths, four locales, menu/section navigation, review focus/draft
  retention, and import/save/reload without execution. Existing resume, feedback and event browser checks
  remain available. Browser Engine calls are synthetic; these are UI checks, not kernel/LAN acceptance.

## Resume a historical submission (2026-09-08, included in 0.3.6)

- **Learn → My lab history → Continue from this attempt** opens a source/current-feedback preview in
  the editor. Explicit confirmation restores that submission's code and lab/template context; keeping
  the draft, failed loads and cancelled/late responses do not replace it. Templates cannot overwrite
  resume drafts, and visiting a target again requires a fresh decision. Nothing runs/attaches/detaches
  automatically; editing and manually running creates a new attempt while preserving the old record.
- Added owner-bound `GET /learning/attempt?attempt_id=...` with no-store responses and explicit storage
  errors, plus typed `learning.attempt()` and generated `getLearningAttempt` SDK calls. Staff still use
  the separate authorized review path for another student's source; the resume endpoint has no username
  override. SQL/file formats and existing API contracts stay compatible.
- Added 2 Rust permission/error regressions, 5 frontend request/validation regressions, 1 SDK transport
  regression and typed/package checks; the full security-enabled quality gate passed. Four optional Chromium
  checks passed, exercising confirmation, real Monaco
  edits, manual mocked runs, retry/cancellation, same-page target races and desktop/mobile layout, alongside
  the existing teacher feedback smoke. Only synthetic Engine responses are used in browser tests; this
  does not claim real PostgreSQL, kernel or LAN execution acceptance. Usage is in the
  [student guide](student-guide.md), with teacher and four-locale UI updates.

## Cancellation-safe learning initialization (2026-09-08, included in 0.3.6)

- First local reads validate UTF-8 and decode JSON on an admitted blocking worker, constructing shared
  records directly. Initialization and commits share one lock; a queued/running load completes after its
  caller is cancelled, and concurrent readers reuse the successful snapshot. Only `NotFound` initializes
  an empty store; other I/O/decode failures remain retryable without partial publication.
- Six Rust regressions cover cancellation, concurrent first reads/writes, invalid paths and differential
  legacy decoding, including invalid UTF-8 in ignored fields. They also passed 20 consecutive focused
  rounds. One JS regression validates five-round paired ordering; the full security-enabled gate passed.
- The final 40-process comparison against the 0.3.5 library uses the same new read-only example and
  identical synthetic files created outside measured processes. At 50,000 rows / 4,184-byte source,
  median maximum 2 ms timer lateness fell 218.641 → 2.037 ms and peak RSS 461.7 → 448.9 MiB, but first
  read latency rose 405.9 → 485.5 ms (+19.6%). The 10k and smaller-source 50k first reads also regressed;
  this improves cancellation safety and executor responsiveness, not uniform initialization speed.
- Measurements are cold-process/store with warm OS page cache, not cold disk, HTTP, PostgreSQL or LAN
  acceptance. Input and decoded records still coexist in memory without a new size limit; whole-file writes,
  crash recovery, fsync and cross-process coordination remain unchanged. Behavior and reproduction are in
  [Learning Record Storage](learning-storage.md); evidence is in
  `reports/benchmarks/2026-09-08-learning-cold-load-final`. The incompatible byte-slice prototype's separate
  40-process dataset is retained without overwriting its hashes or results.

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
