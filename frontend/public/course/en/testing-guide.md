# Current project testing guide

Reviewed against **source release 0.5.0 on 2026-10-03**, including task payload, Session commands and
the standalone content HTTP adapter. This guide explains what to run for each module and boundary, what each layer
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
| Frontend build and unit tests | Frontend quality gate and [`frontend/package.json`](../../frontend/package.json) | Production compilation, TypeScript and state/request/permission/editor regressions | Real browser rendering or real Engine connectivity |
| Browser regressions | Explicit `test:*-browser` scripts | Real Next pages, interaction, Monaco models/workers and fixture-defined request behavior | Real service authorization or end-to-end database/kernel flow when Engine responses are mocked |
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
| Teaching pack and generic catalogue | `task_catalog_tdd`, `teaching_task_adapter_tdd`, `collaboration_contract_tdd` | Typed evidence and exact definition/policy identity; catalogue admission does not execute rules or accept Tasks |
| Events and settings | `module_boundaries_tdd`, EventBus tests, explicit event SQL cases; event/settings unit and browser suites | Publication ordering → durable history → resync, export/delete filter safety, confirmed settings and read failures |
| Generic identity and authority | `collaboration_*_tdd`, `legacy_workspace_projection_tdd` | Audited binding/member/grant revisions, absent-key races, last manager, role versus deployment authority |
| Durable source and lifecycle | `durable_auth_source_tdd`, `durable_collaboration_tdd`, deletion/password/bootstrap/reconciliation targets | Account incarnation → exact current Session → identity/policy on one transaction, namespace replacement, post-write facts |
| Operator provisioning | `provision_cli_tdd`, `provision_postgres_tdd` | Read-only plan, target-bound apply, private enrollment delivery, lost acknowledgement and cancellation; no live configuration adoption |
| Generic Task, Artifact and Review | Explicit storage runners below; their default Rust tests | Exact revisions/digests, Task/outbox atomicity, immutable file boundaries, private ownership, Review history, current Session checks |
| Local task payload and editor | `test:editor-languages`, `test:task-payload-browser`, `test:multi-language-editor-browser` | Optional payloads, accepted content/revisions, import/export, stale edits, model disposal and forbidden network writes |
| Internal Task content metadata | Rust `task_content_contract_tdd`, `task_content_binding_tdd` | Strict object shape, scalar/item limits, exact ordered pins, supplied owner/bytes/digest consistency; pure default tests, not storage or Session authorization |
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
Legacy auth/events/scripts/learning and generic identity/source/lifecycle/provisioning cases have
separate explicit lists in CI. The runner inventory is guarded by
[`postgresCi.test.mjs`](../../scripts/tests/postgresCi.test.mjs).

For example, with the disposable URL already set:

```bash
bash scripts/test-session-task-revisions.sh
```

Record each executed suite, not just an aggregate. Stop the exact temporary PostgreSQL instance and
remove only its validated fixture directory after the run. A failed or cancelled command does not
prove transaction rollback; inspect the test's assertions and retained evidence. Likewise, a failed
Artifact publication or Task replacement is not permission to delete retained content automatically.

## Browser and transport suites

Use a **production frontend build** with a fixture-only `NEXT_PUBLIC_ENGINE_URL`. At browser-test time,
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
