# Project Status

Snapshot date: **2026-10-03**. Source version: **0.5.0**.

Cyanrex is becoming a domain-neutral collaboration platform, with eBPF teaching retained as its first
domain rather than the definition of all work. The transition has produced a durable collaboration
preparation layer and a local task-payload editor. It has **not** yet produced a server-backed generic
task workflow or replaced the existing teaching runtime. This page describes the whole project now;
dated implementation and verification details are preserved in [Development History](development-history.md).

## Release and implementation boundaries

| Scope | What it contains | What that status does not mean |
|---|---|---|
| Source releases through 0.4.8 | Existing teaching product, collaboration identity preparation through C1-K, and task/domain separation C2-A through C2-F | A source release does not install, migrate or validate a running deployment |
| Source release 0.4.9 | Session task inputs, private human Reviews, catalogue task admission and Draft input replacement C2-G through C2-J; local task-payload and language editing UI; project-wide documentation refresh | Source inclusion does not expose public platform APIs, connect browser drafts to storage, or update an installed deployment |
| 0.5.0 · C2-K preparation | Independent Task content metadata and pure supplied-snapshot/byte verification | No metadata persistence, atomic editing, public command, browser save or Session authority in this pure layer |
| 0.5.0 · C2-L preparation | Separate schema 3 content storage with atomic Task/manifest/outbox edits | Trusted owner interface only; no Session content adapter, Artifact byte verification, browser connection or schema 2 migration in this storage layer |
| 0.5.0 · C2-M preparation | Dedicated Session content adapter with old/new text-byte verification and atomic edits | Internal composition only; does not itself mount HTTP, save browser drafts, switch live authentication or migrate storage |
| 0.5.0 · C2-N preparation | Explicit standalone Task-content HTTP router over C2-M, strict Origin/cookie/JSON admission and private errors | Not mounted in the normal app; no Session issuer, Artifact publication, browser connection, live OpenAPI/SDK exposure or migration |
| Architecture targets | Shared collaboration, generic Run orchestration, AI delegation, reliable business-event delivery, isolated workers and ecosystem integration | A proposed type, diagram or milestone is not an implemented user workflow |

Product version, API compatibility baseline and individual storage schema versions are independent.
The source version is 0.5.0 and the public API compatibility baseline remains 0.3.0. The current Task
store requires schema 2 in a fresh dedicated namespace and rejects schema 1 without migration. Neither
the normal startup path nor this source release upgrades a database.
The content store included in 0.5.0 separately requires schema 3; the schema 2 handle and existing Session
adapters reject it. Both formats require their own explicitly installed fresh namespace.

## Whole project capability map

The [capability tensor](capability-maturity.md) adds architecture/function/implementation coordinates,
four independent maturity scores, evidence references and missing edges. These source-review judgments
do not supersede the dated test records below or turn prepared components into a connected product.

“Existing workflow” means the current application implements the path, not that a particular installed
instance or newly built release artifact has been accepted. “Preparation layer” means tested Rust
contracts/services that are not connected to the live application's authentication and storage.

| Module | Available implementation | Remaining boundary |
|---|---|---|
| Application shell and task editing | Existing four-language navigation and action confirmations; local task draft with optional text payload and 14 language profiles, included in 0.4.9 | No server save, browser persistence, autosave, binary attachment or generic task list |
| Live identity and classroom access | Password/TOTP, cookie Sessions, CSRF checks, teacher management, student guards and link-based invitation onboarding | Still the existing instance authority; new Workspace ownership is not a deployment grant |
| Collaboration identity and policy | Stable identities, audited membership/deployment policy, durable Sessions, lifecycle commands, explicit provisioning and read-only reconciliation | No live AuthService cutover, reviewed data migration or complete recovery flow |
| Domain definitions | Versioned TaskCatalog; the five teaching labs use the explicit eBPF pack; a text-only fixture exercises a non-teaching provider | Catalogue metadata admission is not typed input validation or policy execution |
| Task instances | Private manual/catalogue tasks, fixed definitions, revision-fenced states, atomic task/outbox records; Draft input replacement included in 0.4.9 | No mounted public task API, cross-user assignment, dependencies or acceptance policy |
| Artifact content | Private immutable revision files, exact digest-bound references, transactional metadata/events and Session-authorized publication/reads | No UI metadata mapping, public content workflow, retention/garbage collection or cross-user sharing |
| Task content metadata | C2-K contracts, C2-L storage, C2-M Session/text composition and C2-N explicitly constructed HTTP adapter | HTTP adapter remains unmounted; no secure login/installation workflow, public publication, browser saving or migration |
| Review | Immutable revision history; Session-authorized private human opinions on exact Artifact targets/evidence included in 0.4.9 | No cross-user reviewer grants, generic Task approval, automatic rule execution or AI review |
| Teaching and eBPF workbench | Five labs, attempts/progress, teacher feedback, historical resume, Clang assistance, bpftool and supported Aya tracepoint execution | Remains a separate compatible workflow; old attempts have not been migrated into generic Tasks/Runs/Reviews |
| Execution and resources | Local leases/quotas/timeouts; signed remote Agent registration, jobs, probes and optional compile-only diagnostics | Local kernel remains shared; no durable generic Run service, remote eBPF loading or VM lifecycle |
| Events and persistence | Owner-scoped telemetry, bounded history/reconnect recovery and service-specific persistence; preparation stores commit their own outbox records | No generic durable business-event dispatcher, cursor replay or multi-Engine coordination |
| Extensions and integrations | State-only module manifests, structured teacher commands, generated OpenAPI and internal JavaScript SDK | No executable plugin runtime, AI Agent planning/delegation or implemented Viento/Lese/Nuis integration |
| Operations and distribution | Linux/WSL2/Docker paths, explicit SSH management of preinstalled packages, offline tooling and artifact verification | No automatic bare-host bootstrap, deployment cutover or current-candidate LAN/kernel/artifact acceptance claim |

The [system architecture](architecture.md) explains ownership and trust boundaries. The
[current platform network](platform-network.md) distinguishes connected workflows from unfinished links;
the older functional/testing maps retain their original teaching-era evidence.

## Task content is not a code requirement

A Task describes work. Its optional payload may contain notes, code or configuration; editing one
text item does not turn that Task into a Run. The new `/tasks/new` page owns the draft, and `/editor`
opens the same container. The original `/ebpf` workbench remains separate.

The local draft supports up to 32 text items, at most 256 KiB each, with bounded whole-draft JSON
import/export. Filename, language and text belong to the parent draft. Confirmations and revision
checks prevent obsolete imports or callbacks from replacing newer work. Drafts live only in page
memory; a download is a local export, not a server save. See [Task payload editing](editor.md).

JS/TS provide browser-local semantic assistance; JSON/HTML/CSS use their configured local providers.
Rust, Python, C/C++ and the other basic profiles provide highlighting/snippets, not external language
servers. There is no rust-analyzer, Pyright, clangd, project filesystem or LSP transport. Language
feedback is neither execution nor assessment, and shared browser workers are not a security sandbox.

The server has a different identity contract: exact owned Artifact revisions, current Session
authorization and a Task revision check. [Draft replacement](session-task-revisions.md) validates old
and new content around the task/outbox write while preserving prior content and Reviews. Artifact
publication remains a separate operation. Filenames/languages, title changes, import semantics and
browser save/conflict handling still need an explicit public contract; local IDs cannot supply one.
The [C2-K manifest](task-content-manifest.md), included in 0.5.0, defines metadata and pure consistency
checks. [C2-L](task-content-store.md) adds independent trusted storage and atomic edits, without
changing existing schema 2 records or Session commands. [C2-M](session-task-content.md) now composes
current Session and exact Artifact-text checks internally. The authorized browser workflow remains absent.
[C2-N](task-content-http.md) adds a standalone HTTP boundary but does not mount it in the existing
application, issue credentials, publish Artifacts or connect the browser.

## Security and compatibility retained during transition

- Existing self-hosted teaching remains teacher-authoritative, including deployment management for
  the personal-use teacher. The preparation layer separates membership from explicit instance grants;
  it does not grant a new workspace owner access to other people's private content or other instances.
- Session-authorized resource commands derive ownership on the server and recheck account incarnation,
  audited active membership and current Session before confirmed commit. Direct owner-supplied stores
  are trusted internal primitives, not authorization APIs.
- New durable stores reject unavailable or incompatible storage rather than silently adopting the
  legacy runtime's memory/file fallbacks. Existing teaching storage has not been switched or migrated.
- A timeout, cancellation or lost commit acknowledgement is not proof of rollback. Failed content
  publication/replacement does not authorize deleting immutable files or blindly retrying a command.
- The privileged local Engine is for trusted self-hosted use, not public multi-tenancy. Shared-kernel
  quotas are not student isolation; a registered compiler Agent is not an AI participant or a remote
  kernel sandbox. In-memory jobs, attachments and module state have no multi-replica coordination.
- Task state, execution success and Review judgment remain separate. Generic Task acceptance, shared
  review authority and policy execution must not be inferred from private records or frozen metadata.

## Verification recorded so far

### 0.5.0 source release checks

On 2026-10-03, the full local gate passed **452 default Rust tests, 114 common script checks,
165 frontend tests and 16 SDK tests**. The default Rust run ignored 452 opt-in cases. Production build,
full TypeScript, package checks, formatting, API/SDK compatibility, version metadata and document
checks passed. Dependency resolutions did not change; installed dependencies were reused with
`--no-npm-install`. Public API compatibility remains based on 0.3.0.

Seven separate browser suites passed all **72 cases**: task payload 16, filename import 1, task
navigation 4, multi-language 18, editor intelligence 25, auth Session 3 and layout 5. Forty-seven use
the production Next.js pages; the 25 editor-intelligence cases use the component fixture. Engine
responses are mocked throughout, not server saving. The temporary frontend and browsers were stopped.

Fresh npm audits reported one low-severity DOMPurify finding, no moderate/high/critical findings in
the frontend, and none in the SDK. Rust advisories, live kernel/LAN, distribution artifacts, deployment,
migration and remote CI were not revalidated. The 212 isolated PostgreSQL cases passed in the immediately
preceding C2-N run recorded below; they were not rerun for this metadata-only version step. No live
authentication or public task-content installation was enabled by the release.

### Historical implementation records

The dated implementation records below preserve their original 0.4.9 working-tree version, counts
and then-unreleased status. Their changes are included in 0.5.0; that source inclusion does not rerun
those checks, establish a 0.5.0 release gate or certify deployment.

### Unreleased C2-N explicit task-content HTTP

On 2026-10-03, the standalone HTTP target first failed to compile on its missing API. Five subsequent
behavior regressions first failed and then passed: always-ready empty frames starving the body deadline,
GET media-type admission, wildcard origin configuration, the empty task path, and missing method
advertisement. The router remains explicitly constructed and unmounted; no live route, credential
issuer, OpenAPI/SDK contract or product version changed.

The backend-only gate passed **452 default Rust cases and 114 common script checks**, with 452 opt-in
Rust cases ignored. The new target contributes 14 default cases; the selected SQL cases below run
separately. Formatting, API/SDK compatibility, synthetic management/distribution tools, source limits,
version metadata, course copies and tensor/view checks passed.

**212 exact PostgreSQL cases passed** on a new PostgreSQL 16.15 cluster with a private 0700 Unix socket
and no TCP listener: 11 new HTTP-to-C2-M cases and all 201 preceding resource/content cases. The new
cases cover four endpoint successes, ordered Unicode text and empty payloads, private ownership,
forged references, invalid/corrupted content, lifecycle rejection, authorization expiry after observed
storage locks, deferred commit/outbox faults, post-write file loss and competing edits. A source-table
lock proves transport rejections return before entering Session commands. The initial schema-2 case
failed in fixture setup before HTTP; its setup was corrected and all eleven cases rerun successfully.
The exact runner and no-live-wiring guards are connected to CI; remote CI was not run.

All runs remove inherited deployment/database settings and use only disposable storage and synthetic
content. The exact temporary server was stopped; its 151 MiB database and 2.6 MiB generated fixtures/cache
(including the failed setup's synthetic residue) were removed, retaining diagnostic logs. In-process
requests do not establish listener, proxy/TLS or browser-to-server acceptance.
Frontend/browser, SDK runtime, independent identity/lifecycle SQL, dependency advisories, kernel,
LAN, deployment and migration were not rerun. W13 scores **3 / 1 / 3 / 1** only for this standalone
adapter. The tensor has 57 implementation coordinates, 73 edges and 16 paths; browser e70 and
installation e73 remain missing. Version remains 0.4.9, with no commit or push.

### Unreleased C2-M Session task content

On 2026-10-03, the new target first failed to compile on the missing Session-content API. After
implementation, the backend-only gate passed **438 default Rust cases and 112 common script checks**,
with 441 opt-in Rust cases ignored. Formatting, API/SDK compatibility, synthetic tooling, source
limits, version metadata, course copies and tensor/view checks passed. The two new default cases
check trusted routing and unavailable-source behavior; no public API or product version changed.

**201 exact PostgreSQL cases passed** on a new PostgreSQL 16.15 cluster with a private 0700 Unix socket
and no TCP listener: 25 new Session-content cases plus the preceding 176 resource/storage cases.
The new cases cover ordered text, empty tasks, metadata-only edits/removal, exact ownership, text
bounds, schema compatibility, revoked/expired Sessions, account reincarnation, competing edits,
namespace substitution, post-write SQL/file tampering, commit failure and cancellation. Three
namespace cases were also run separately during review; they are not counted twice. The runner checks
exact names before executing and is wired into CI, but remote CI was not run.

An initial restricted-sandbox default test stopped at the private file-root fixture with
`BlobUnavailable`, before invoking the adapter. The scoped elevated run passed; the failure was not
an adapter assertion. Deployment/database environment settings were cleared for all execution.
No existing database or runtime data was used. Test schemas were removed, the exact temporary server
was stopped, and its 148 MiB generated data plus empty fixture/socket directories were deleted;
diagnostic logs were retained.

Frontend/browser, SDK runtime, independent identity/lifecycle SQL, dependency advisories, kernel,
LAN, deployment, migration and browser-to-server acceptance were not rerun. W12's **3 / 1 / 3 / 1**
scores apply only to this internal source-owned Session/text transaction. The tensor now has
56 implementation coordinates, 70 edges and 15 paths; public/browser e70 remains missing. Version
remains 0.4.9, with no commit, push or deployment.

### Unreleased C2-L atomic content storage

On 2026-10-03, the new target first failed to compile without the separate store API. Six pure helper
cases then produced 2 passes/4 failures against a replacement stub before all six passed. The completed
backend-only gate passed **436 default Rust cases and 111 common script checks**, with 416 opt-in Rust
cases ignored. Formatting, API/SDK compatibility, synthetic tooling, source limits, version metadata,
course copies and tensor/view checks passed. This adds eight default cases and one CI-selection check
to the preceding C2-K baseline; no product version or public API changed.

Separately, **176 exact PostgreSQL cases passed** on a fresh PostgreSQL 16.15 cluster with a private
0700 Unix socket and no TCP listener: 21 new content-store cases plus all 155 cases from the nine
preceding Task/Artifact/Review and Session resource runners. This is real transaction, conflict and
fault-injection evidence, not content Session/browser acceptance. The new runner is wired into CI and
checks every exact test name before execution; remote CI was not run in this turn.

The first database attempt failed during URL parsing (`EmptyHost`) before any database assertion.
Adding a nonempty URI authority while retaining the private socket query fixed the test configuration;
the exact runner then passed all 21. Deployment/database environment settings were cleared. All test
namespaces were removed, the specific temporary server was stopped, and its generated data/fixture
directories were deleted; test logs were retained. No existing database or runtime data was used.

Frontend/browser, SDK runtime, separate identity/lifecycle SQL suites, advisory scans, kernel, LAN,
deployment, migration and browser-to-server tests were not rerun. W11 scores `3 / 1 / 3 / 1` for trusted
schema 3 storage only; current-Session/Artifact-byte composition remains missing. The tensor now has
55 coordinates, 66 edges and 14 paths. Version remains 0.4.9, with no commit, push or deployment.

### Unreleased C2-K content contract

On 2026-10-03, metadata tests first rejected the absent model and caught a Unicode filename bound;
the byte-verifier stub failed 10 of 14 cases before implementation. Independent review then found
Serde accepted positional record arrays and an object-shaped `kind`; a failing regression reproduced
the issue before map-only decoding was added without altering existing reference types.

The backend-only quality gate passed **428 default Rust cases and 110 common script checks**, with
395 opt-in Rust cases ignored. The 26 new cases comprise 12 metadata/JSON and 14 supplied-byte cases.
Formatting, public API/SDK compatibility, synthetic tooling, version, file-length, document links and
generated course checks passed. Inherited deployment/database settings were removed; only test
fixtures used loopback access. No real PostgreSQL, frontend/browser, SDK runtime, security advisory,
kernel, LAN, deployment or migration tests were rerun. The tensor now records 54 coordinates, 62 edges
and 13 representative paths; W10 is `3 / 1 / 2 / 1`, while the browser save bridge remains missing.
The source version remains 0.4.9; this working-tree slice is not committed, pushed or deployed.

### Capability inventory checks after 0.4.9

The initial 2026-10-03 capability review recorded 53 implementation coordinates, 59 directed edges and 12
representative paths. All 110 common script tests passed, including 18 new tensor/reading-view checks.
Source/test/evidence paths, bilingual tables, local document links, file limits, version metadata and
course copies were checked. This was documentation/tooling validation, not a new product, database,
browser, kernel or deployment run; maturity scores retain their separately dated evidence.

### Unreleased bug hunt after 0.4.9

On 2026-10-03, regression tests reproduced two frontend defects before the fixes: same-page fragment
navigation discarded local task drafts or interrupted pending logout, and filename truncation split
a supplementary Unicode character and rejected a valid text import. The fixes retain pathname/query
identity checks, login return fragments, filename limits and content validation.

After the fixes, the frontend-only quality gate passed 165 frontend and 92 common script tests,
production build/TypeScript, and API, version and document checks. Seven production-browser suites
passed all 72 cases: task payload 16, filename import 1, task navigation 4, multi-language 18,
editor intelligence 25, auth Session 3 and workspace layout 5. These use mocked Engine responses;
the temporary frontend and browsers were stopped afterwards. npm audit still reports one low-severity
DOMPurify finding and no moderate/high/critical findings.

Backend task-input and authorization boundaries received code review, not a fresh runtime test.
Rust, PostgreSQL, SDK runtime, deployment, kernel and LAN acceptance were not rerun. This working-tree
check leaves the source version at 0.4.9; these fixes are not yet committed, pushed or deployed.

### 0.4.9 source release checks

On 2026-10-03, the full local quality gate passed: 402 default Rust tests (395 opt-in cases ignored),
92 common script tests, 164 frontend tests and 16 SDK tests, with production build, TypeScript,
package, API compatibility, version and document checks. Installed dependencies matched the unchanged
dependency locks; the gate used `--no-npm-install`. Separate production-browser fixtures passed all
59 cases: 16 task-payload, 18 multi-language and 25 editor-intelligence cases. They mock Engine responses,
not server saving. The temporary frontend and browsers were stopped after testing.

Six initial Rust loopback tests were blocked by sandbox permissions; all six and then the full gate
passed with loopback testing permitted and deployment configuration removed. Fresh npm audits reported
one low-severity frontend DOMPurify finding, no moderate/high/critical findings, and none for the SDK.
This release check did not rerun opt-in PostgreSQL suites, Rust advisory audit, deployment, kernel or
LAN acceptance; remote CI and tag-built artifacts require their own results.

### Earlier implementation checks

The records below are separate dated runs, not a combined whole-project acceptance run. Their counts
are preserved from the corresponding implementation stages, not relabeled as a new 0.4.9 release check.
Counts include the scope indicated, not all ignored or optional suites.

| Recorded run on 2026-10-03 | Passed checks | Limits |
|---|---|---|
| 0.4.8 source release baseline | 381 default Rust, 88 script, 125 frontend and 16 SDK tests; production build/type/package checks; 114 selected PostgreSQL cases | 324 Rust cases were default-ignored; remote CI, deployment and current artifact/kernel acceptance were separate |
| Local task-payload frontend slice | 164 frontend tests, 91 common script checks, production build/full TypeScript; 35 browser cases | Temporary frontend and synthetic identity/network responses; did not rerun backend, SDK runtime or live dependency audits |
| C2-J backend slice | 402 default Rust tests, 92 common script checks; 155 PostgreSQL cases comprising 17 new and 138 preceding resource cases | 395 Rust cases were default-ignored; separate auth-source/collaboration suites, frontend/browser tests and deployment acceptance were not rerun |

The release baseline's frontend dependency audit reported one low-severity DOMPurify advisory and no
moderate/high/critical findings; the SDK audit reported none. That is a dated result, not a fresh
security audit. The [testing guide](testing-guide.md) describes which suites establish which boundary;
[acceptance](acceptance.md) keeps real integration, kernel, LAN and distribution evidence separate.
Historical performance results likewise retain their original sources and do not establish current
platform capacity or classroom isolation.

## Next Decision Points

The immediate mainline is to complete one private, non-teaching task-content workflow before adding
generic execution or AI. The following order is proposed work, not an enabled feature:

1. Define whole-draft import and bounded publication/manifest mapping onto the internal C2-M commands.
   Decide empty-title behavior explicitly; local IDs and revisions stay non-authoritative.
2. Build reviewed Session issuance and an explicit authenticated installation/mounting path for the
   C2-N router, then public content publication. Preserve its Origin/request limits and conflict/outcome
   semantics; do not silently switch live authentication or migrate existing storage.
3. Connect explicit browser save, reload and conflict handling. Preserve unsaved work and distinguish
   content publication from Task reference replacement; verify uncertain results before retrying.
4. Verify browser-to-server persistence across restart, authorization rejection and concurrent edits.
   Only then call the private task workflow connected; it still does not establish cross-user collaboration.
5. Add separately reviewed sharing/reviewer grants and domain evidence/acceptance policies, then prove
   a non-teaching version-specific collaboration workflow without teacher/student or Run requirements.

## Longer term platform roadmap

| Track | Existing foundation | Gate before claiming completion |
|---|---|---|
| Identity and migration | C0 contracts and C1 preparation through provisioning/reconciliation | Actual deployment inventory, restored backups, reviewed migration, single write authority and live route/CSRF cutover |
| Artifact Task Review | C2-A–F first included in 0.4.8; C2-G–J and local UI in 0.4.9; C2-K–N content preparation in 0.5.0 | Saved user workflow, sharing/acceptance policy and compatible legacy projections; C2 is not complete |
| Reliable execution and events | Local Runner and compile-only Agent protocol; transactional resource outboxes | Durable Runs/jobs/leases, outcome reconciliation, business-event delivery/replay and restart/fault evidence |
| Runtime isolation | Explicit shared-kernel boundary and desktop/LAN VM design | Full execution/resource ownership, cleanup/reset evidence and recovery before remote eBPF or Engine replicas |
| Agents and ecosystem | Typed core references, internal SDK and proposed integration contracts | Bounded Agent delegation, tools/budgets, independent Review and real partner integration evidence |
| Distribution and support | Source/version/API checks and candidate verification tooling | Accepted candidate artifacts, signing/key ownership, publishing/support policy and current kernel/LAN evidence |

The [next architecture proposal](../zh-CN/next-architecture.md) retains its original 0.4.3 source
inventory and target milestones; it is not a claim that those milestones are delivered. In particular,
offline attempt conversion, a reliable compile-only Run, multi-user document review and Viento
integration remain outstanding. No calendar date or new version is implied by this roadmap.

`engine/Cargo.toml` remains the canonical product version. Version checks protect release-facing
metadata; they do not merge working-tree features into a release or certify a deployed instance.
