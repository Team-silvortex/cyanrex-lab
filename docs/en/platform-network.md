# Current platform feature and module network

Base source review date: **2026-10-03**. Current scope: **0.5.1 source**, including prepared-authentication
hardening, the 0.5.0 navigation/filename fixes and C2-K/L/M/N content preparation, alongside the preceding
collaboration commands and local task payload editor. The review retains its
0.4.9 base commit and dated evidence; source inclusion is not a new release acceptance report or a
claim about an installed deployment.

For architecture × capability × implementation × maturity, use the new
[capability tensor and score rubric](capability-maturity.md) and its
[machine inventory](../platform-capability-tensor.json): 57 implementation coordinates, 73 directed
edges and 16 representative paths. This page remains the route/module topology view; scores and
evidence are maintained separately, with no aggregate completion percentage.

Cyanrex is moving from an eBPF teaching application toward a task-oriented collaboration platform.
Three areas coexist: the connected teaching runtime, the separately prepared generic backend, and a
local-only task editor. Code is one optional kind of task payload; neither a task nor the platform
requires code. The important unfinished work is connecting these areas without transferring the
legacy teacher role or browser-supplied identities into generic permissions.

## Status vocabulary

| Status | Meaning in this map |
|---|---|
| Connected runtime | Included in the current Engine or frontend composition; deployment-specific prerequisites still apply. |
| Prepared backend | Implemented internal service or explicit local operator tool, with tests, but not connected to public routes or normal startup. |
| Local only | A browser feature whose content is not saved to the Engine or executed. |
| Planned boundary | A required connection or capability that the current source does not implement. |

The composition anchors are [`state.rs`](../../engine/src/state.rs),
[`application.rs`](../../engine/src/application.rs),
[`SidebarLayout.tsx`](../../frontend/src/components/SidebarLayout.tsx) and the
[`pages` directory](../../frontend/pages). New Task, Artifact, Review and durable-source services are
not fields in the current `AppState`, and no public generic Task/Artifact/Review API is registered.

## Browser entry points

There are **18 page routes**, excluding Next.js `_app`. A page being present is not a grant of access:
the sidebar handles navigation and session visibility, while Engine guards authorize server actions.

| Route | Current responsibility | Connection |
|---|---|---|
| `/` | Redirect to the dashboard | Connected runtime |
| `/dashboard` | Workspace navigation and overview | Connected runtime |
| `/login` | Password and TOTP login, safe return path | Legacy authentication |
| `/register` | Public account registration where configured | Legacy authentication |
| `/otp-setup` | Password-authorized TOTP enrollment | Legacy authentication |
| `/join` | Inspect teacher discovery and compatibility, confirm origin, consume an invitation | Legacy classroom entry |
| `/account` | Password change and account deletion with confirmations | Legacy authentication |
| `/ebpf` | Source, templates, saved scripts, checks, completion, runtime and attempt resume | Connected teaching and Linux execution |
| `/learn` | Lab progress, attempts and learning resources | Learning API and document links |
| `/learn/[...slug]` | Render the synchronized English or Chinese course document | Static course content, optional lab entry |
| `/teaching` | Teacher student overview, attempt inspection and feedback | Legacy teaching authorization and storage |
| `/modules` | Teacher header catalog, download, selection and deletion | Header service; not executable plugin installation |
| `/events` | History, filters, streaming, unread state, export and deletion | Owner-scoped event API and WebSocket |
| `/helper` | Environment and Runner readiness | Read-only Engine reports |
| `/settings` | Event/compiler settings, metrics, Agent and classroom management panels | Existing settings, Runner and classroom APIs |
| `/terminal` | Structured management commands | Typed dispatcher, not an arbitrary shell |
| `/tasks/new` | Task title and optional text payloads, local import/export and editing | Local only; session-gated shell |
| `/editor` | Compatibility entry to the same task draft container | Local only; not a second editor data model |

The local draft owns accepted content and local revision numbers. The controlled editor owns Monaco
models and local language tools, not server task identity or execution authority. Fourteen language
profiles include semantic JavaScript/TypeScript tools, structured JSON/CSS/HTML tools, basic language
support and plain text. This does not provide a general external LSP server, a filesystem project,
Rust Analyzer, Pyright or clangd. See [the editor guide](editor.md).

## Connected runtime modules

| Module | Main source | Functional chain and limit |
|---|---|---|
| Runtime composition and configuration | [`config.rs`](../../engine/src/config.rs), [`state.rs`](../../engine/src/state.rs), [`application.rs`](../../engine/src/application.rs) | Environment configuration → service composition → guarded HTTP/WebSocket routes. It does not initialize the prepared collaboration stores. |
| Authentication and teacher authority | [`auth_service.rs`](../../engine/src/services/auth_service.rs), [`auth.rs`](../../engine/src/routes/auth.rs) | Accounts → password/TOTP → HTTP-only session → CSRF/role guards. Personal use seeds a teacher who also manages deployment; this is the existing authority model. |
| Classroom connection | [`classroom.rs`](../../engine/src/services/classroom.rs), [`classroom route`](../../engine/src/routes/classroom.rs) | Teacher descriptor/link → independent protocol/capability checks → student-name-bound invitation → normal account/session flow. Discovery does not authenticate the teacher or authorize execution. |
| Saved scripts | [`script_store.rs`](../../engine/src/services/script_store.rs), [`scripts route`](../../engine/src/routes/scripts.rs) | Authenticated owner → save/list/delete → PostgreSQL or configured local persistence → explicit editor load. A saved script is not yet a generic Artifact. |
| Modules, headers and commands | [`module_manager.rs`](../../engine/src/services/module_manager.rs), [`c_header_module.rs`](../../engine/src/services/c_header_module.rs), [`command_dispatcher.rs`](../../engine/src/services/command_dispatcher.rs) | Manifest discovery → inventory/control state; verified header bytes → selected metadata → compiler; typed terminal command → approved module action/navigation. Module start/stop is not dynamic code loading. |
| eBPF tooling and runtime | [`ebpf route`](../../engine/src/routes/ebpf.rs), [`ebpf_loader`](../../engine/src/services/ebpf_loader), [`runner_manager`](../../engine/src/services/runner_manager.rs) | Source/headers → compiler diagnostics or bounded Runner operation → Linux loader/attachment → exact detach and observations. Kernel work is privileged and is not a hostile-student sandbox. |
| Runner Agent | [`runner_agent_client.rs`](../../engine/src/services/runner_agent_client.rs), [`runner_job_queue.rs`](../../engine/src/services/runner_job_queue.rs), [`Agent binary`](../../engine/src/bin/cyanrex-runner-agent.rs) | Signed Agent registration/heartbeat → leased job → probe or compile report. Remote compile checks do not introduce remote kernel execution; advertised isolation is not proof of a VM/container security boundary. |
| Teaching and learning | [`learning_catalog.rs`](../../engine/src/services/learning_catalog.rs), [`learning_store`](../../engine/src/services/learning_store), [`learning route`](../../engine/src/routes/learning.rs) | Lab → run observation → assessment → stored attempt/source snapshot → teacher feedback → owner-bound resume. Resume loads historical source and does not rerun it. |
| Event delivery and settings | [`event_bus.rs`](../../engine/src/services/event_bus.rs), [`events route`](../../engine/src/routes/events.rs), [`settings route`](../../engine/src/routes/settings.rs) | Runtime publication → retained owner history/persistence → WebSocket delivery or resync → browser view. Export/delete/settings have separate validation and confirmed-write boundaries. This EventBus is not the generic stores' transactional outbox. |
| Distribution and deployment | [`cyanrex-release`](../../engine/src/bin/cyanrex-release), [`docker`](../../docker), [`scripts`](../../scripts/README.md) | Verify package/provenance → review SSH target and installed package → explicit apply/acceptance. Native Rust tools lead; compatibility Python/shell tools remain. No automatic migration of generic stores is implied. |
| Public integration contract | [`openapi.json`](../../engine/openapi/openapi.json), [`sdk-js`](../../sdk-js/README.md) | Registered routes → OpenAPI → generated TypeScript/operations → SDK consumers. The signed Agent protocol is outside the browser SDK; prepared generic commands are outside both public contracts. |

The existing teaching facade is already connected to the built-in
[`EbpfTeachingPack`](../../engine/src/domain_packs/ebpf_teaching/mod.rs) through
[`TaskCatalog`](../../engine/src/services/task_catalog.rs). This extracts versioned definitions and
typed rule evidence without changing the existing teaching HTTP/storage path. A domain pack is trusted
built-in code, not a downloaded plugin, an execution scheduler or an authorization provider.

## Prepared collaboration backend

The shared model distinguishes Authority, Principal, Workspace, membership, deployment permission,
Task, immutable Artifact revision and Review. Their identifiers are scoped references, not usernames,
paths, editor IDs or client role claims. See [collaboration concepts](collaboration-foundation.md).

| Module | Main source | Implemented connection and remaining limit |
|---|---|---|
| Contract and legacy projection | [`models/collaboration`](../../engine/src/models/collaboration), [`legacy_workspace.rs`](../../engine/src/services/legacy_workspace.rs) | Domain-neutral values and a pure preview of selected legacy permissions; not migration or runtime authorization. |
| Durable identity and policy | [`collaboration_identity_store`](../../engine/src/services/collaboration_identity_store) | Explicit namespaces → bindings, active membership, deployment grants and audited revisions. No implicit teacher-role upgrade or ambient database adoption. |
| Durable authentication and commands | [`durable_source`](../../engine/src/services/auth_service/durable_source) | Account incarnation/session → audited identity/policy/lifecycle commands in one transaction. Source schema 2, included in 0.5.1, consumes OTP counters with login/rotation, fresh-install only; rejects schema 1 without migration. Public login/account endpoints still use legacy `AuthService`. |
| Bootstrap and reconciliation | [`cyanrex-provision`](../../engine/src/bin/cyanrex-provision), [`reconciliation`](../../engine/src/services/collaboration_identity_store/reconciliation) | Explicit target-bound empty-namespace bootstrap and private enrollment delivery; bounded read-only graph inspection. Not a startup hook, repair tool or existing-data importer. |
| Task instance store | [`task_store`](../../engine/src/services/task_store) | Definition snapshot + exact inputs → revision-fenced lifecycle + atomic outbox. Current fresh-install storage schema is 2; schema 1 is rejected without migration. |
| Artifact revision store | [`artifact_store`](../../engine/src/services/artifact_store) | Private immutable files + metadata/outbox → exact scoped revision/digest read. Publication is separate from Task changes; a failed database write can leave a private file. |
| Review store | [`review_store`](../../engine/src/services/review_store) | Exact target/evidence revisions → immutable judgment/comment history. Human and rule judgments are distinct; neither automatically accepts a Task. |
| Session-authorized private work | [`task_commands`](../../engine/src/services/auth_service/durable_source/task_commands), [`private_work`](../../engine/src/services/auth_service/durable_source/private_work) | Fresh Session + current account/membership → own Tasks, Artifacts and human Reviews with namespace and post-write checks. A teacher cannot use these adapters to read another owner's private work. |
| Catalogue admission and Draft replacement | [catalogue commands](session-catalog-tasks.md), [replacement commands](session-task-revisions.md) | Exact server-admitted definition metadata; Draft replacement verifies old and new inputs and increments one Task revision. It does not run the domain provider, create evidence or accept work. |
| Content metadata (0.5.0 C2-K) | [manifest contract](task-content-manifest.md), [`task_content.rs`](../../engine/src/services/task_content.rs) | Supplied Task/Artifact snapshots → metadata, owner and byte consistency; no storage read/write or current-Session authorization. |
| Content storage (0.5.0 C2-L) | [separate schema 3 store](task-content-store.md), [`content.rs`](../../engine/src/services/task_store/content.rs) | Trusted owner + metadata → atomic Task/manifest/outbox and one revision; no Session/Artifact-byte adapter, public API or schema 2 migration. |
| Authorized content (0.5.0 C2-M) | [Session content adapter](session-task-content.md), [`content.rs`](../../engine/src/services/auth_service/durable_source/task_commands/content.rs) | Current Session → old/new exact Artifact text → atomic schema 3 edit → final authorization and commit; no live/browser connection. |
| HTTP content boundary (0.5.0 C2-N) | [explicit router](task-content-http.md), [`platform_http`](../../engine/src/platform_http/mod.rs) | Bounded HTTP → C2-M → private versioned response; standalone only, no main-app mounting, Session issuer, public publication or browser wiring. |

## Cross-module paths and unconnected boundaries

| Path | Current state |
|---|---|
| Browser → legacy Session → teaching/script/event operation | Connected; authorization and ownership are enforced by the Engine, not sidebar visibility. |
| Source → local Runner → observation → teaching pack → attempt → teacher feedback | Connected teaching path; an assessment result is not a generic durable Review. |
| Browser remote check → owner-bound job → signed Agent → compile report | Connected optional compiler path; no local-execution fallback is granted by Agent failure. |
| Local TaskDraft → selected payload → controlled editor → local export | Connected entirely in the browser; login/unread reads are shell behavior, not payload saving. |
| Durable Session → own Artifact revision → own Task exact inputs → Draft replacement | Implemented preparatory backend path; public API and browser adapter are absent. |
| Durable Session → exact owned Artifact target/evidence → human Review history | Implemented preparatory backend path; cross-user review authority and acceptance remain absent. |
| Legacy accounts/scripts/attempts → durable identity/Artifacts/Tasks/Reviews | Planned migration, writer fencing and cutover; no implicit dual write or adoption. |
| Browser task save → Artifact publication → Task create/replace → conflict recovery | W10–W13 provide contracts, storage, Session/text checks and an unmounted HTTP adapter; explicit login/installation, public publication and browser mapping/conflict/outcome handling remain missing. |
| Generic Task → execution plan → typed evidence → rule Review → acceptance | Planned composition. Catalogue metadata and in-process assessment alone do not establish this workflow. |
| Teacher-managed isolated student runtime, managed VM lifecycle, external LSP process | Planned; existing Linux execution and local language workers do not establish these isolation boundaries. |

In particular, changing a Task to `Cancelled` does not cancel a Runner job, and Task input replacement
does not rewrite old Artifacts or Reviews. Such effects need explicit adapters and separately defined
authorization and failure semantics.

## Keeping this map current

Update this page, the [capability tensor](capability-maturity.md) and their Chinese counterparts when adding a page,
composition field, public route, provider, persistent store or cross-module adapter. Record whether a
connection is public, internal or local-only and name its failure boundary. Verify its tests through
the [current testing guide](testing-guide.md), not a historical passing total.

The [0.3.8 functional network](functional-network.md) and its machine-readable inventory are preserved
historical source snapshots; the [0.3.7 test network](testing-network.md) is a dated test report with
separate fix evidence. Their counts and source fingerprints must not be refreshed to suggest that they
cover this source release. Use [project status](project-status.md) for dated verification evidence and
[architecture](architecture.md) for the governing design.
