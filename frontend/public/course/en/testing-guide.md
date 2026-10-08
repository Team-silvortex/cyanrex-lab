# Current project testing guide

Reviewed against **0.5.4 on 2026-10-08**, including C2-T–W intent, dispatch and inspection boundaries,
the Next.js 15.5.27 floor, and earlier AI adapters, draft preparation, sanitizer hardening,
authentication guards and the standalone content HTTP adapter. This guide explains what to run for each module and boundary, what each layer
proves, and how to avoid using live data. It is a test plan and source inventory, not a new test run.
Use the [platform network](platform-network.md) to identify connected and unconnected product paths.
The [capability tensor](capability-maturity.md) links each implementation slice to test sources and
dated evidence. Its common-script checks validate structure, references and bilingual score-table
consistency; they do not execute the referenced product tests or establish deployment maturity.

## Test layers

| Layer | Maintained entry | What a pass establishes | Not established |
|---|---|---|---|
| Common source and tooling checks | [`quality-gate.sh`](../../scripts/quality-gate.sh), [`scripts/tests`](../../scripts/tests) | File/version/course consistency, public API/SDK contract consistency, script regressions and synthetic tool fixtures | Deployed workflows, real SSH/Docker/kernel acceptance |
| Default Rust tests | `cargo test --manifest-path engine/Cargo.toml --locked` | Models, services, in-process routes, default integration fixtures and compile-time contracts | Explicitly ignored PostgreSQL, live Agent transport and performance cases |
| Explicit PostgreSQL tests | Named CI cases and storage runners below | Real SQL transactions, namespace/schema rules, locks, races, cancellation and injected failures in disposable data | Browser integration, deployed data migration or live authorization cutover |
| Frontend build and unit tests | Frontend quality gate and [`frontend/package.json`](../../frontend/package.json) | Production compilation, TypeScript, state/request/permission/editor regressions and fail-closed asset-generation checks | Real browser rendering or real Engine connectivity |
| Browser regressions | Explicit `test:*-browser` scripts | Real Next pages or local generated-asset fixtures, interaction, Monaco models/workers and fixture-defined request behavior | Real service authorization or end-to-end database/kernel flow when Engine responses are mocked |
| SDK and contract tests | `quality-gate.sh --sdk-only`, [`sdk-js/package.json`](../../sdk-js/package.json) | Generated models/operations, compatibility, runtime requests, type checks and packed consumer imports | APIs absent from the public contract, including prepared generic commands |
| Tool and release fixtures | `test-runner-agent-tools.sh`, `test-distribution-tools.sh`, `test-live-kernel-smoke.sh`, `test-release-candidate.sh` | Secret handling, target/metadata/archive validation, cleanup and error reporting under mocks | Actual installation, remote SSH changes, Agent deployment or kernel execution |
| Explicit live acceptance | `runner-agent-smoke.sh`, `live-kernel-smoke.sh`, `distribution-install-smoke.sh`, native `cyanrex-release` | Only the real environment, candidate and operations recorded by that run | Other kernels, machines, versions or isolation guarantees |
| Benchmarks | `bench-mainline.mjs`, `bench-event-stream.mjs`, `bench-event-history-reads.mjs` | The measured workload/profile/source snapshot | Correctness acceptance or an unmeasured whole-system throughput claim |

Every quality-gate mode runs the common preflight and tool fixtures first. Default mode then runs
backend, frontend and SDK checks. `--security` adds the Rust advisory audit; frontend/SDK checks also
run their production npm audits. `--format-only` is therefore not merely a formatter invocation.
The gate does **not** automatically run all ignored cases, browser suites or real privileged smoke tests.
The [CI workflow](../../.github/workflows/ci.yml) adds named PostgreSQL cases and real loopback Agent
compilation; consult its explicit lists rather than assuming `cargo test` covers them.

## Module and boundary selection

| Changed area | Primary tests | Adjacent boundary to include |
|---|---|---|
| Legacy authentication and roles | `routes_tdd`, `auth_service` unit/explicit SQL tests; `authSession`, `sidebarPermissions` frontend tests | Account lifecycle, CSRF, teacher authority, classroom enrollment cancellation, UI logout failure |
| Classroom discovery and invitations | `classroom_tdd`, `classroomConnection` tooling/unit/browser suites | Version/protocol/capability rejection, confirmed origin, one-time invitation → legacy account creation |
| Scripts and learning persistence | `module_boundaries_tdd`, `learning_store` tests | Owner isolation, corrupt/failed JSON writes, real script SQL, feedback revision races, historical resume without rerun |
| Modules, headers and terminal | `routes_tdd` module/command cases; header/dispatcher service tests; `terminalCommand` frontend tests | Teacher-only mutation, checksum-invalid downloads, selected metadata → compiler, no arbitrary shell dispatch |
| eBPF editor and execution | Compiler/loader/Runner tests; `compilerCheck`, `semanticCompletion`, runtime/breakpoint browser suites | Check versus run, compiler source freshness, cancellation/lease ownership, exact detach; live kernel acceptance remains separate |
| Runner Agent | Authenticator, registry, job queue/executor/client tests; `runner_agent_client_tdd`; inventory/admin browser tests | Signature/replay → lease/result, timeout/cancel, owner-bound remote check, no execution fallback |
| AI Agent profiles and SDK helpers | `ai_agent_settings_tdd`, private-store cancellation units, `test:ai-agents`, explicit `test:ai-agents-browser`, SDK Agent runtime/type/package tests | Teacher/CSRF → revision-fenced private metadata → exact reviewed UI acknowledgement; explicit tool selection, per-call approval, hostile calls and no automatic retry. Synthetic formats are not real provider acceptance |
| Teaching pack and generic catalogue | `task_catalog_tdd`, `teaching_task_adapter_tdd`, `collaboration_contract_tdd` | Typed evidence and exact definition/policy identity; catalogue admission does not execute rules or accept Tasks |
| Events and settings | `module_boundaries_tdd`, EventBus tests, explicit event SQL cases; event/settings unit and browser suites | Publication ordering → durable history → resync, export/delete filter safety, confirmed settings and read failures |
| Generic identity and authority | `collaboration_*_tdd`, `legacy_workspace_projection_tdd` | Audited binding/member/grant revisions, absent-key races, last manager, role versus deployment authority |
| Registry timestamps | `test-registry-timestamps.sh` selects ten cases from four existing targets | Nullable retirement validity, mandatory identity/policy audit times, finite limits, old history/replay, append readback, reconciliation errors and Session-command rejection; no new age/order policy |
| Durable source and lifecycle | `durable_auth_source_tdd`, `durable_session_boundary_tdd`, `durable_collaboration_tdd`, deletion/password/bootstrap/reconciliation targets | Source-only namespace/table pins → account incarnation → exact current Session → identity/policy on one transaction, namespace replacement, post-write facts |
| Prepared password work | Default `auth_service::durable_source::password_work` unit tests and source-wiring guard | Dispatched/queued capacity, cancellation/timeout lifetime, panic recovery and real hashing; rerun source, password-change and bootstrap SQL for all entry points |
| Missing-account password work | `test-durable-login-password.sh`, fixed-PHC unit and source/CI guards | Real shared-gate admission into a held executor, released SQL resources, cancellation, concurrent same-name registration and rejection even when the synthetic password matches; not a timing benchmark or complete enumeration defense |
| Explicit expired Session cleanup | `test-durable-session-cleanup.sh`: nine exact library SQL cases, default closed-pool test and source/CI guards | 128-row batches, active-session preservation, independent writers, lock-fresh cutoff, bounded text/time decoding, orphan/corrupt candidates and hit-proven deletion/commit/cancellation faults; no automatic or live-data cleanup |
| Prepared password profile | `test-durable-password-profile.sh`, default `password_profile`/`password_work` units and source/CI guards | Six exact SQL cases for unsupported records, writer compatibility, existing Sessions and rollback; hostile numeric costs are pure-preflight tests, never expensive test computations |
| Prepared OTP freshness | `test-durable-otp-freshness.sh` selects seven `--lib` SQL cases; default `otp::tests` and source/CI guards | Private clocks advance after observed SQL waits or snapshot release with the password executor held, not a proven queue-dispatch instant; rollback, success and freshness versus composed consumption. No machine clock changes or public clock override |
| OTP consumption policy | Default `otp_consumption::tests` and `durableOtpConsumption` source guards | Pure counter selection, real collisions, credential binding, exact final counter and input/time limits; pure units alone do not prove persistence/atomicity. The policy is now used by prepared login/rotation |
| Atomic OTP consumption | `test-durable-otp-consumption.sh`: 11 exact library SQL cases | Independent sources, reopen/logout, login/rotation competition, account incarnations, version/shape rejection, suppression/tampering/deferred failure/cancellation and real collisions |
| OTP source format | `test-durable-otp-schema.sh`: four exact SQL cases | Genuine schema 1 rejected unchanged, malformed schema-2 column/default/constraints refused, fresh registration/bootstrap at -1 and hit-proven initialization faults; no migration test or claim |
| Operator provisioning | `provision_cli_tdd`, `provision_postgres_tdd` | Read-only plan, target-bound apply, private enrollment delivery, lost acknowledgement and cancellation; no live configuration adoption |
| Generic Task, Artifact and Review | Explicit storage runners below; their default Rust tests | Exact revisions/digests, Task/outbox atomicity, immutable file boundaries, private ownership, Review history, current Session checks |
| Local task payload and editor | `test:editor-languages`, `test:task-payload-browser`, `test:multi-language-editor-browser` | Optional payloads, accepted content/revisions, import/export, stale edits, model disposal and forbidden network writes |
| DOMPurify and generated Monaco assets | `frontendDependencySecurity` common guards, default `test:tooling`, explicit `test:dompurify-browser` | Locked copies and overrides → exact vendor replacement → new chunk references → the sanitizer actually used by Monaco; dependency audit and deployed behavior remain separate |
| Internal Task content metadata | Rust `task_content_contract_tdd`, `task_content_binding_tdd` | Strict object shape, scalar/item limits, exact ordered pins, supplied owner/bytes/digest consistency; pure default tests, not storage or Session authorization |
| Draft import and publication plan | Rust `task_draft_publication_tdd`, `taskDraftPublication` common guards, frontend `taskDraft` shared fixtures | Strict bounded JSON, blank-title rejection, local identity removal, exact supplied-content mapping and canonical-export compatibility; no publication, current-Session check or browser saving |
| Session draft publication steps | Rust `session_task_draft_publication_tdd`, `scripts/test-session-task-draft-publication.sh`, `sessionTaskDraftPublication` common guards | Fixed allocation and Session binding, one write per advance, empty/multi-item success, confirmed prefix versus unconfirmed step, cancellation, revoked authority and final real-byte checks; no durable recovery or browser saving |
| Unconfirmed draft target observation | Rust `session_task_draft_observation_tdd`, `scripts/test-session-task-draft-observation.sh` | Original Session/state admission, exact read-only comparisons, typed errors, unchanged attempt, current Task bytes versus historical prefix and missing-Task scope; no rollback proof or resumption |
| Draft metadata checkpoints | Default Rust `draft_publication::tests::checkpoint`, `session_task_draft_checkpoint_tdd`, `sessionTaskDraftCheckpoint` common guards | Strict bounded metadata codec, ordered allocations/lengths, state/count relations and round trips; imported progress is data, not authorization, provenance, persistence or a restored attempt |
| Explicit checkpoint target inspection | Default `draft_publication::tests::inspection`, `session_task_draft_inspection_tdd`, `scripts/test-session-task-draft-inspection.sh`, `sessionTaskDraftInspection` common guards | Explicit scope/current Session, one existing reader, exact metadata/text checks, typed faults, missing/empty Task boundaries and unchanged data; no original identity/body provenance, journal or restoration |
| Immutable Session draft intent | `session_task_draft_intent_tdd`, `scripts/test-session-task-draft-intent.sh`, `sessionTaskDraftIntent` common guards | Real Ready/zero-progress admission, canonical bounded metadata, duplicate conflicts, exact owner/account generation, current source/registry/journal pins and failure outcomes; no resource dispatch, restored attempt or write-ahead execution |
| Journaled Session draft dispatch | `session_task_draft_dispatch_tdd`, `scripts/test-session-task-draft-dispatch.sh`, `sessionTaskDraftDispatch` common guards | Explicit schema 2, consumed pristine attempt, acknowledged Unknown reservation, exact owner/account/nonce gates, resource/marker atomicity and complete prefix/pin checks; no restart/retry or public/browser acceptance |
| Session draft journal inspection | `session_task_draft_journal_inspection_tdd`, `scripts/test-session-task-draft-journal-inspection.sh`, `sessionTaskDraftJournalInspection` common guards | Schema 2 only, current exact owner/account, complete committed prefix plus optional Unknown, final Task count, pins/fresh authority and typed failures; no resource/file read or recovery |
| Journal step resource observation | `session_task_draft_journal_observation_tdd`, `scripts/test-session-task-draft-journal-observation.sh`, `sessionTaskDraftJournalObservation` common guards | Ordinal admission, same-transaction exact resource reads, original full journal/nonces/first pins after the read, final fresh authority and body-free result; no business writes, receipt or recovery |
| Separate content storage | Rust `task_content_store_tdd`, `scripts/test-task-content-storage.sh` | Schema 2/3 isolation, complete metadata/outbox atomicity, revision conflicts and storage faults; not a Session content or browser adapter |
| Session task content | Rust `session_task_content_tdd`, `scripts/test-session-task-content.sh` | Current authorization, old/new exact text, metadata-only edits, removal, namespace pins and post-write checks on one transaction; no public/browser acceptance |
| UI navigation and safety | `test:ui-permissions`, layout/action/account/runtime/browser suites | Target-bound confirmations, duplicate actions, route changes, keyboard access, keeping unsaved drafts |
| API, SDK, packaging and deployment | Common contract tests, SDK gate, release/SSH Rust targets and tool fixture runners | Engine route → OpenAPI → generated SDK; archive provenance → exact reviewed SSH target; fixtures versus real installation |

Names such as `authSession` identify the matching files in `frontend/tests`; exact npm script names
are listed in `frontend/package.json`. Use `--list` before an exact Rust filter: a mistyped filter can
otherwise report success with zero tests. The explicit storage runners enforce this check themselves.

## Safe default workflow

Run from a checkout intended for testing. Remove inherited deployment configuration before starting:
in particular `DATABASE_URL`, `CYANREX_*`, PostgreSQL connection variables and dotenv preload settings.
Then set only the fixture-specific values required by the chosen suite. Merely unsetting
`CYANREX_TEST_DATABASE_URL` does not protect a process whose normal services inherited `DATABASE_URL`.
Never reuse the deployment data directory, private Artifact root, credentials or Agent token.

After preparing that isolated environment, select the smallest relevant gate and its boundary tests:

```bash
./scripts/quality-gate.sh --backend-only
./scripts/quality-gate.sh --frontend-only --no-npm-install
./scripts/quality-gate.sh --sdk-only --no-npm-install
```

`--no-npm-install` is appropriate only when dependencies already match their lockfiles. Normal frontend
builds synchronize course copies from `docs/`; do not edit `frontend/public/course` as the source.
For documentation-only changes, check lengths, version synchronization, course synchronization and
links; do not describe those checks as a fresh runtime regression run.

The frontend gate also runs `test:private-request` for the shared transport lifecycle. Feature tests
retain their response/acknowledgement policies; explicit component-browser fixtures check Settings,
Metrics, Runner, Events and Runtime state separately. A cancelled browser wait is not server rollback.

## Explicit database suites

Use a newly provisioned disposable PostgreSQL instance with a dedicated test database and private
temporary files. A private Unix socket or restricted loopback listener is suitable; never point these
suites at a deployed database. Fixtures deliberately create/drop schemas, replace records, introduce
triggers and corrupt synthetic content. Some need schema/object privileges unavailable to application
roles. Use the isolation procedure in [acceptance](acceptance.md) and the current CI fixture as examples,
not as permission to reuse old local database credentials or paths.

Set `CYANREX_TEST_DATABASE_URL` explicitly to that disposable database. Keep `DATABASE_URL` unset.
Some legacy SQL suites also require `CYANREX_DB_FALLBACK=true`; this is a fixture setting, not an
authorization to accept fallback as successful persistence. Provisioning SQL fixtures additionally use
`CYANREX_TEST_DATABASE_PASSWORD`. Follow each exact suite's fixture and CI environment.

| Runner | Cases in this source snapshot | Main boundary |
|---|---:|---|
| `scripts/test-task-storage.sh` | 15 | Trusted Task storage, lifecycle revisions and outbox |
| `scripts/test-task-content-storage.sh` | 21 | Separate schema 3 manual Task/manifest persistence, atomic edits, legacy rejection and fault/concurrency checks |
| `scripts/test-artifact-storage.sh` | 17 | Exact immutable content, private file faults and pinned inputs |
| `scripts/test-review-storage.sh` | 17 | Review history, human/rule separation and non-teaching integration |
| `scripts/test-session-task-storage.sh` | 17 | Current Session → private manual Task |
| `scripts/test-session-artifact-storage.sh` | 18 | Current Session → private publication and exact read |
| `scripts/test-session-task-inputs.sh` | 20 | Task → exact owned Artifact inputs and post-write verification |
| `scripts/test-session-review-storage.sh` | 18 | Session → private human Review with exact target/evidence |
| `scripts/test-session-catalog-tasks.sh` | 16 | Exact admitted definition metadata and 0–32 inputs |
| `scripts/test-session-task-revisions.sh` | 17 | Draft replacement, old/new input union, schema 2 compatibility and competing saves |
| `scripts/test-session-task-content.sh` | 25 | Schema 3 current-Session content, exact text, old/new rechecks, namespace substitution, lifecycle and concurrent faults |
| `scripts/test-platform-task-content-http.sh` | 11 | Standalone HTTP → C2-M, private ownership, Session expiry during lock waits, exact text, commit/outbox faults and competing edits |

These twelve runners total **212 selected cases** (155 preceding resource cases, 21 content-store, 25 Session-content and 11 standalone HTTP cases),
not all PostgreSQL coverage in the repository.

0.5.3 C2-P adds a separate `scripts/test-session-task-draft-publication.sh` runner for explicit
12 create-only workflow cases, not included in the twelve-runner total. It selects exact cases from
`session_task_draft_publication_tdd` against disposable storage; default preparation/state tests do
not substitute for those SQL runs. Dated execution results remain in [project status](project-status.md).

0.5.3 C2-Q has a separate `scripts/test-session-task-draft-observation.sh` exact runner and
`session_task_draft_observation_tdd` target. Its 12 read-only cases are separate from C2-P’s 12 creation
cases: observations must preserve attempt state and distinguish current visibility from past commit
confirmation. A missing Task does not require inspecting the Artifact namespace; read errors must not
be converted into absence. These cases are outside the twelve-runner total above.

0.5.3 C2-R checkpoint tests are default pure-contract/library checks, not another PostgreSQL runner.
They must reject malformed/oversized documents and inconsistent targets or progress while keeping
parsed data separate from live attempts. Round trips do not establish crash durability or permit
calling C2-Q after restart; storage and recovery require separate future tests.

0.5.3 C2-S uses `scripts/test-session-task-draft-inspection.sh` for the independent inspection
entry point. It supplies current Session authority rather than recreating C2-Q's original attempt.
Select its exact database cases separately from the C2-R codec and C2-Q observation suites; metadata
agreement must never be counted as historical confirmation, original-body comparison or safe retry.

0.5.4 C2-T uses `scripts/test-session-task-draft-intent.sh` to select registration/read cases
from `session_task_draft_intent_tdd` on an explicitly installed disposable journal. Keep its fresh
schema/installation-ID, exact account-generation, duplicate/conflict and transaction-fault checks
separate from C2-P writes. Source/registry/journal validation does not test original Task/Artifact
incarnations. A confirmed record is not a dispatched resource or proof of durable-before-dispatch;
database COMMIT settings do not substitute for host recovery or backup-replay tests. Dated execution
results belong in [project status](project-status.md), not the earlier runner totals above.

0.5.4 C2-U uses `scripts/test-session-task-draft-dispatch.sh` with
`session_task_draft_dispatch_tdd`. Check explicit schema 2 admission without upgrading schema 1,
confirmed A-before-B ordering, authorization changes, exact nonces/prefixes, marker triggers before
business validation and final read-only journal checks. Resource SQL and its marker must share B's
commit; an uncertain acknowledgement must not become rollback proof or retry permission. Existing
C2-T registration/read cases remain separate and do not establish this dispatch behavior.

0.5.4 C2-V uses `scripts/test-session-task-draft-journal-inspection.sh` with
`session_task_draft_journal_inspection_tdd` for ten selected SQL cases; two default tests and three
common guards are separate. Dated execution results are recorded in [project status](project-status.md). Check legal
prefixes, final Task counting even for an empty payload, exact owner/account visibility, schema/pin
faults, wait expiry and cancellation without resource or file access. Missing records do not prove
rollback, and recorded progress cannot restore a wrapper or permit retry. C2-T/U evidence stays separate.

0.5.4 C2-W selects twelve SQL cases through `scripts/test-session-task-draft-journal-observation.sh`
and `session_task_draft_journal_observation_tdd`, with two default cases and three common guards.
Dated execution is recorded in [project status](project-status.md). Check pure ordinal refusal, missing
intent versus unrecorded step versus invisible resource, old exact Artifact revisions, edited/current
Task contents, typed blob failures and the original complete journal/pin recheck after resource access.
Body-free output still reads files; waiting/cancellation must not grant receipts, mutation or retry.
External-writer blocking does not test mutation inside the read transaction; first-snapshot reuse is source-guarded.
C2-V remains resource-free, and its prior execution does not verify this separate read path.

Legacy auth/events/scripts/learning and generic identity/source/lifecycle/provisioning cases have
separate explicit lists in CI. The runner inventory is guarded by
[`postgresCi.test.mjs`](../../scripts/tests/postgresCi.test.mjs).

The separate `scripts/test-prepared-resource-sql.sh` runner selects two additional library cases for
the shared Task/Artifact/Review mechanics. It checks ordinary-table rejection and decode order, plus
read/write transaction settings and LOCAL timeout cleanup after rollback/drop on one pooled connection.
Its exact inventory and CI inclusion are checked by `architectureBoundaries.test.mjs`; these two cases
are not included in the twelve-runner total above and do not replace store or Session coverage.

As of the 2026-10-07 expiry-decoding follow-up, `scripts/test-durable-session-boundary.sh` selects
**17 source-only Session cases**, the original 14 plus three timestamp cases:
ambiguous/temporary namespaces, filtered or incompatible relations, post-write redirects/replacements,
metadata changes, login's two-transaction identity, logout/expiry lock ordering, and invalid/boundary
Session expiry through validation, login writeback and the password-change guard. It needs no
collaboration registry and is guarded by `durableSessionBoundaryCi.test.mjs`. It is separate from the
212 resource cases and the original 14 `durable_auth_source_tdd` SQL cases. These inventory counts are
not claims that every suite was rerun; see dated project status.

`scripts/test-durable-reconciliation.sh` now selects **17 cases**: the original twelve lifecycle checks,
two Session-expiry cases and three subsequent registry-timestamp cases from 2026-10-07. Session checks
retrieve only an SQL expiry boolean; registry cases cover precise error stages, finite audit times and
denied Session-authorized binding. The runner checks exact names before execution. These are inventory
counts, not new passing-run evidence; historical C1-K results are unchanged.

The 2026-10-07 registry follow-up adds **10 distinct SQL cases**: two identity-store, two identity-audit,
three policy-audit and three reconciliation cases. The corresponding full inventories are **17, 18, 19
and 17** cases. `scripts/test-registry-timestamps.sh` is a focused exact selector for those ten, not ten
additional cases beyond those suites; CI includes them through the maintained full-suite selections.
Range guards protect the three stored registry columns, not arbitrary timestamps or wall-clock ordering.

For example, with the disposable URL already set:

```bash
bash scripts/test-session-task-revisions.sh
```

Record each executed suite, not just an aggregate. Stop the exact temporary PostgreSQL instance and
remove only its validated fixture directory after the run. A failed or cancelled command does not
prove transaction rollback; inspect the test's assertions and retained evidence. Likewise, a failed
Artifact publication or Task replacement is not permission to delete retained content automatically.

## Browser and transport suites

For Next-backed page suites, use a **production frontend build** with a fixture-only `NEXT_PUBLIC_ENGINE_URL`. At browser-test time,
set matching `CYANREX_UI_ENGINE_URL` and the temporary frontend's `CYANREX_UI_BASE_URL`; configure
`CYANREX_PLAYWRIGHT_MODULE` and `CYANREX_CHROMIUM_PATH` if not available at their defaults. Inspect each
suite's helper before running: environment values and fixtures are not a connection to a real backend.
Production mode matters because some WebSocket fixtures would interfere with development HMR.

For the local payload/editor boundary:

```bash
npm --prefix frontend run test:task-payload-browser
npm --prefix frontend run test:multi-language-editor-browser
npm --prefix frontend run test:editor-intelligence-browser
```

The payload helper uses real pages and Monaco, permits only mocked identity/unread reads, and rejects
payload network writes and WebSockets. This is useful proof of local-only behavior, not a server-save
test. Other browser suites mock Engine routes for auth, classroom, learning, events, settings, Runner
administration and safety scenarios. No currently implemented browser suite can prove the absent
generic Task HTTP save workflow.

### Generated Monaco sanitizer fixture in 0.5.3

The DOMPurify follow-up adds default `test:tooling` units for the asset-generation boundary and
extends the common offline dependency guards to the DOMPurify 3.4.16 floor, alongside sharp and
source-map-js. Units check supported versions, original chunk hash and syntax shape, isolated official
ESM wrapping, unchanged bytes outside the vendor block, deterministic new chunk URLs and rewritten
references. Unknown input fails closed; the tests do not authorize patching `node_modules` or upgrading
Monaco silently. Live production npm audit remains separate, with its existing threshold.
Tooling units also inject preparation and patch-write failures to check private staging outside
`public` and preservation of existing assets before publication; they do not claim atomic publication.
The common Docker-input guard also requires dependency-stage generated assets and excludes the host's
Monaco asset directory from the build context; this source check is not a Docker image build.

With matching installed dependencies and Playwright/Chromium available, regenerate the local assets
before selecting the browser fixture from the repository root:

```bash
node frontend/scripts/sync-monaco-assets.mjs
npm --prefix frontend run test:dompurify-browser
```

This explicit suite contains ten cases. Nine instrument the actual generated AMD chunk to inspect the
same DOMPurify instance used by Monaco, not merely the root npm package. They cover the patched
version, safe formatting and executable-markup removal, before/after-element and after-attribute
removal hooks including kept custom elements, rejected rawtext roots, healthy root identity, and
real Monaco hover rendering. Test instrumentation is not shipped in generated production assets.

The tenth case loads the unmodified AMD loader and editor entry, new hashed chunk and language-registration
graph. Requests to the synthetic `monaco-fixture.invalid` origin are intercepted and fulfilled only from
local generated files; unexpected origins and paths are rejected. The first nine cases reject every
network request. No case makes external network contact or starts a Next or Engine service.
The suite accepts the same optional Playwright/Chromium path settings above but needs
no production frontend build or Engine URL. These are component/browser regressions, not proof of
application exploitability, a deployed sanitizer version or end-to-end acceptance. Browser execution
is opt-in; the default frontend gate runs the pure tooling units, not these ten browser cases.
The suite inventory is not a passing result; dated outcomes remain in [project status](project-status.md).

Real transport checks are separate. The ignored `runner_agent_client_tdd` case starts a loopback HTTP
server and uses Linux Clang with the BPF target; it compiles without attaching a program. The event
stream pressure harness checks real paced, overloaded and stalled sockets. Neither proves a real
LAN browser/teacher/student workflow, TLS deployment or VM isolation.

## Live and release acceptance

`test-live-kernel-smoke.sh` uses mocks; **`live-kernel-smoke.sh` is different and performs real privileged
work**. Run the latter only on an explicitly designated disposable Linux stack after reviewing its
target, credentials, empty attachment requirement and cleanup. It binds an observed kernel event to
its program, detaches the exact pin and rejects residue. Candidate evidence must match the package
metadata, revision and image content it claims to validate.

Likewise, `test-distribution-tools.sh` is not an installation, while `distribution-install-smoke.sh`
can start a real stack. SSH fixture tests do not authorize a remote apply. See
[offline deployment and release acceptance](../../docker/README-DEPLOY.md), [acceptance](acceptance.md) and
[classroom connection](classroom-connection.md) before those operations. Keep real privileged runs,
mock tool checks, dependency advisory scans and benchmarks as separate evidence categories.

## Evidence and maintenance

The unreleased **C2-L storage run** passed 436 default Rust cases (416 ignored), 111 common script
checks and, separately, 176 exact PostgreSQL cases: 21 new plus the preceding 155 resource cases.
The database was a fresh private-socket PostgreSQL 16 instance, not a deployed service. No content
Session/browser adapter or migration was exercised; see the dated cleanup and scope in project status.

The unreleased **C2-K pure-contract run** passed 428 default Rust cases (395 ignored) and 110 common
script checks, including 26 new metadata/snapshot cases. It did not rerun PostgreSQL or browser tests.

The earlier **C2-J backend development run** passed 402 default Rust cases, 92 common script
cases and the preceding 155 resource PostgreSQL cases. The earlier **task payload/editor run** passed
164 default frontend tests and 35 browser cases. These were separate scoped runs recorded on
2026-10-03; they must not be added into a single fresh end-to-end acceptance claim. The backend run
did not rerun every durable-source or lifecycle database suite, and browser mocks do not prove live
server saving. See [project status](project-status.md) for exact qualifications and previous phases.

For every new run, retain date, source revision/worktree state, selected tests, exact environment,
passed/failed/ignored counts, fixture boundaries and cleanup status. Never count a repeated case as
new coverage. Preserve [the historical test network](testing-network.md) and its reports rather than
rewriting old failures, counts or source fingerprints. Update this guide when a runner, boundary or
test environment changes, and keep its [Chinese version](../zh-CN/testing-guide.md) aligned.
