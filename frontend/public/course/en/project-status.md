# Project Status

Snapshot date: **2026-10-03**. Source version: **0.4.9**.

Cyanrex is becoming a domain-neutral collaboration platform, with eBPF teaching retained as its first
domain rather than the definition of all work. The transition has produced a durable collaboration
preparation layer and a local task-payload editor. It has **not** yet produced a server-backed generic
task workflow or replaced the existing teaching runtime. This page describes the whole project now;
dated implementation and verification details are preserved in [Development History](development-history.md).

## Release and implementation boundaries

| Scope | What it contains | What that status does not mean |
|---|---|---|
| Source releases through 0.4.8 | Existing teaching product, collaboration identity preparation through C1-K, and task/domain separation C2-A through C2-F | A source release does not install, migrate or validate a running deployment |
| New in source release 0.4.9 | Session task inputs, private human Reviews, catalogue task admission and Draft input replacement C2-G through C2-J; local task-payload and language editing UI; project-wide documentation refresh | Source inclusion does not expose public platform APIs, connect browser drafts to storage, or update an installed deployment |
| Architecture targets | Shared collaboration, generic Run orchestration, AI delegation, reliable business-event delivery, isolated workers and ecosystem integration | A proposed type, diagram or milestone is not an implemented user workflow |

Product version, API compatibility baseline and individual storage schema versions are independent.
The source version is 0.4.9 and the public API compatibility baseline remains 0.3.0. The current Task
store requires schema 2 in a fresh dedicated namespace and rejects schema 1 without migration. Neither
the normal startup path nor this source release upgrades a database.

## Whole project capability map

“Existing workflow” means the current application implements the path, not that a particular installed
instance or newly built release artifact has been accepted. “Preparation layer” means tested Rust
contracts/services that are not connected to the live application's authentication and storage.

| Module | Available implementation | Remaining boundary |
|---|---|---|
| Application shell and task editing | Existing four-language navigation and action confirmations; local task draft with optional text payload and 14 language profiles, included in 0.4.9 | No server save, browser persistence, autosave, binary attachment or generic task list |
| Live identity and classroom access | Password/TOTP, cookie Sessions, CSRF checks, teacher management, student guards and link-based invitation onboarding | Still the existing instance authority; new Workspace ownership is not a deployment grant |
| Collaboration identity and policy | Stable identities, audited membership/deployment policy, durable Sessions, lifecycle commands, explicit provisioning and read-only reconciliation | No live AuthService cutover, reviewed data migration or complete recovery flow |
| Domain definitions | Versioned TaskCatalog; the five teaching labs use the explicit eBPF pack; a text-only fixture exercises a non-teaching provider | Catalogue metadata admission is not typed input validation or policy execution |
| Task instances | Private manual/catalogue tasks, fixed definitions, revision-fenced states, atomic task/outbox records; Draft input replacement included in 0.4.9 | No public task command adapter, cross-user assignment, dependencies or acceptance policy |
| Artifact content | Private immutable revision files, exact digest-bound references, transactional metadata/events and Session-authorized publication/reads | No UI metadata mapping, public content workflow, retention/garbage collection or cross-user sharing |
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

1. Define server payload metadata and exact Artifact references, including empty tasks, supported
   filename/language fields, task title changes and whole-draft import. Keep local IDs non-authoritative.
2. Design reviewed public command adapters with current Session/CSRF checks, request limits, explicit
   conflicts and outcome semantics. Decide the authenticated installation path before exposing the
   preparation layer; do not silently switch live authentication or migrate existing storage.
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
| Artifact Task Review | C2-A–F first included in 0.4.8; C2-G–J and local UI included in 0.4.9 | Saved user workflow, sharing/acceptance policy and compatible legacy projections; C2 is not complete |
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
