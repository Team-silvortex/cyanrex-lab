# Platform capability tensor and maturity

Review date: **2026-10-04**. Source version: **0.5.1**, based on the prior 0.5.0 commit
`a9523ec052f701e447d2e59fcd38d3a8d8186740`, plus the source-only Session namespace/table guards included in 0.5.1
and prepared password-work admission, hash-profile validation, OTP freshness and schema-2 atomic
counter consumption. Consumption commits with login/rotation; schema 1 is refused without migration.
C2-K through C2-N remain prepared contracts, storage, Session content and an unmounted HTTP adapter.
Historical evidence dates/scopes remain unchanged; the Session follow-up adds its own record, not
a public issuer or current deployment acceptance. This inventory is not a release gate.

The central finding is that **local task editing, the existing teaching runtime and the prepared
collaboration backend remain separate paths**. They do not yet form a generic saved-task or shared
review workflow. Capabilities therefore have multidimensional coordinates, while connections are
recorded independently instead of inferred from the existence of adjacent modules.

The [machine inventory](../platform-capability-tensor.json) is the maintained source for coordinates,
scores, implementation/test references, evidence and gaps; this page is its reading view. The
[platform network](platform-network.md) retains route/module topology, [project status](project-status.md)
retains dated execution records, and the [testing guide](testing-guide.md) explains test selection.

## Four dimensions and sparse semantics

Read `T[A, F, I, M] = score`: A is architecture, F capability, I implementation slice, and M maturity
criterion. Each JSON cell stores one `[A, F, I]` coordinate and a D/C/V/O score vector: a grouped sparse
coordinate representation of four scalar entries. An implementation slice can reference several
cooperating files; it is not a file count, microservice or deployment unit. One capability can have
multiple coordinates: D01's internal domain catalogue and D02's connected teaching facade have
different connection scores and must not be collapsed into one completed capability.

This review records **8 architecture areas, 56 capabilities, 57 implementation coordinates, 73 directed
edges and 16 representative paths**. The 57 coordinates comprise **17 live, 5 local, 23 prepared and
12 planned** slices. These are inventory counts, not coverage denominators. This is not an enumeration
of every endpoint, internal function, failure permutation or potential capability; sparse density is
not product completion.

- **Absent coordinate:** unmodeled or inapplicable, not an implicit zero.
- **`null`:** modeled but unassessed, not unimplemented; all current 57 coordinates are assessed.
- **Explicit zero:** below level one for that criterion; the gap explains why.
- **Planned slice:** may have a type contract D=1 while capability connection/evidence remain C=0, V=0.
- **Independent edges:** code and tests at both ends do not establish a connection. A `missing` edge
  cannot inherit endpoint scores and become connected.

## Maturity rubric

Scores are **reviewer-assigned ordinal judgments grounded in inspectable evidence**, not measured
percentages, security levels or SLAs. Do not sum, average or normalize them into completion rates.
A path containing a missing edge remains blocked regardless of its node averages. Evidence dates
and scope stay separate from the scores.

| Criterion | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| D Implementation | Design only or absent | Explicit types/contracts | Functional implementation | Boundary safeguards and regression cases implemented |
| C Connection | Required connection absent | Explicit internal composition only | Wired into current application, local workflow or operator tool | Exact current deployment cross-component acceptance |
| V Verification | No capability execution evidence | Test source inspected only | Dated unit, mock, browser or tool-fixture execution | Scoped real database or service integration record |
| O Operations | No capability operating procedure | Limits and operating prerequisites documented | Explicit bounded operating/reconciliation controls and regressions | Exact current deployment recovery/rollback acceptance |

For example, `3 / 1 / 3 / 1` describes guarded backend code with historical SQL evidence but internal-only
composition; it does **not** mean the UI can use it. `3 / 2 / 2 / 1` can describe local editing: real Monaco
and file-download tests still do not prove real Engine saving. No current coordinate has C=3 or O=3.
Old kernel, LAN, SSH and package evidence has not been relabeled as current deployment acceptance;
V=3 does not refresh an evidence date either.

| Architecture coordinate | Scope |
|---|---|
| `ui` | Browser and local content |
| `authority` | Live instance authority |
| `identity` | Collaboration identity |
| `work` | Task Artifact Review |
| `domain` | Domains and teaching |
| `execution` | Execution and resources |
| `delivery` | Events and integration |
| `operations` | Operations and distribution |

## Capability and implementation coordinates

Each row is scored only for its named scope. Source links locate implementations; evidence links locate
existing records. Full test paths are in `implementations.tests` in the JSON. A test file's existence
is not a claim that this review ran it. U01/U06 include fixes collected in 0.5.0; W10–W13 are that
release's pure contract, separate storage, current-Session content composition and unmounted HTTP
preparation. Release inclusion does not change their connection scores or provide browser saving.

<!-- capability-tensor:start -->
| Coordinate | Capability | Connection | D / C / V / O | Implementation sources | Evidence | Next gap |
| --- | --- | --- | --- | --- | --- | --- |
| U01 · ui | Session shell and navigation | Live | 3 / 2 / 2 / 1 | [SidebarLayout.tsx](../../frontend/src/components/SidebarLayout.tsx) | [bughunt-local](../../docs/en/project-status.md) | Client visibility is not Engine authorization; fragment fixes are included in 0.5.0. |
| U02 · ui | Local Task draft ownership | Local | 3 / 2 / 2 / 1 | [taskDraft.ts](../../frontend/src/features/tasks/taskDraft.ts)<br>[TaskDraftWorkspace.tsx](../../frontend/src/features/tasks/TaskDraftWorkspace.tsx) | [bughunt-local](../../docs/en/project-status.md) | No server save, browser persistence, task list or binary payload. |
| U03 · ui | Whole draft import and export | Local | 3 / 2 / 2 / 1 | [TaskDraftWorkspace.tsx](../../frontend/src/features/tasks/TaskDraftWorkspace.tsx)<br>[taskDraft.ts](../../frontend/src/features/tasks/taskDraft.ts) | [bughunt-local](../../docs/en/project-status.md) | Download is local export, not server commit or guaranteed disk persistence. |
| U04 · ui | Controlled payload editor | Local | 3 / 2 / 2 / 1 | [EditorWorkspace.tsx](../../frontend/src/features/editor/EditorWorkspace.tsx)<br>[useEditorSession.ts](../../frontend/src/features/editor/useEditorSession.ts) | [bughunt-local](../../docs/en/project-status.md) | Single selected model; switching does not retain undo or create a filesystem project. |
| U05 · ui | Local language assistance | Local | 3 / 2 / 2 / 1 | [languages.ts](../../frontend/src/features/editor/languages.ts)<br>[languageServices.ts](../../frontend/src/features/editor/languageServices.ts) | [bughunt-local](../../docs/en/project-status.md) | 14 profiles are not 14 LSP servers; shared JS/TS workers are not a security sandbox. |
| U06 · ui | Bounded text import and download | Local | 3 / 2 / 2 / 1 | [document.ts](../../frontend/src/features/editor/document.ts)<br>[EditorWorkspace.tsx](../../frontend/src/features/editor/EditorWorkspace.tsx) | [bughunt-local](../../docs/en/project-status.md) | Text only; Unicode truncation fix is included in 0.5.0; filenames confer no filesystem access. |
| U07 · ui | Target-bound confirmations | Live | 3 / 2 / 2 / 1 | [useConfirmedAction.tsx](../../frontend/src/components/useConfirmedAction.tsx)<br>[useDraftWarning.ts](../../frontend/src/features/editor/useDraftWarning.ts) | [release-049](../../docs/en/project-status.md)<br>[bughunt-local](../../docs/en/project-status.md) | Confirmation is not authorization or rollback; history/programmatic navigation recovery is incomplete. |
| A01 · authority | Password TOTP and teacher authority | Live | 3 / 2 / 2 / 1 | [auth_service.rs](../../engine/src/services/auth_service.rs)<br>[auth.rs](../../engine/src/routes/auth.rs)<br>[application.rs](../../engine/src/application.rs) | [release-049](../../docs/en/project-status.md)<br>[bughunt-local](../../docs/en/project-status.md) | Legacy instance authority, not new Workspace grants; no durable-source cutover. |
| A02 · authority | Teacher discovery and invitation join | Live | 3 / 2 / 2 / 1 | [classroom.rs](../../engine/src/services/classroom.rs)<br>[join.tsx](../../frontend/pages/join.tsx)<br>[connection.js](../../frontend/src/features/classroom/connection.js) | [release-049](../../docs/en/project-status.md) | Discovery is not authentication; no multicast discovery or current LAN/TLS acceptance. |
| I01 · identity | Scoped collaboration contracts | Prepared | 2 / 1 / 2 / 1 | [mod.rs](../../engine/src/models/collaboration/mod.rs)<br>[legacy_workspace.rs](../../engine/src/services/legacy_workspace.rs) | [release-049](../../docs/en/project-status.md) | Scoped types and implemented pure legacy projections do not migrate, authenticate or authorize. |
| I02 · identity | Identity binding and retirement | Prepared | 3 / 1 / 3 / 1 | [identity_command.rs](../../engine/src/services/collaboration_identity_store/identity_command.rs)<br>[identity_audit.rs](../../engine/src/services/collaboration_identity_store/identity_audit.rs) | [c1-identity](../../docs/en/collaboration-identity-lifecycle.md) | Historical SQL evidence; production adoption, writer fencing and recovery are absent. |
| I03 · identity | Membership grants and audit | Prepared | 3 / 1 / 3 / 1 | [access_policy.rs](../../engine/src/services/collaboration_identity_store/access_policy.rs)<br>[policy_command.rs](../../engine/src/services/collaboration_identity_store/policy_command.rs) | [c1-policy](../../docs/en/collaboration-policy-audit.md) | Membership is not deployment permission or private-content sharing. |
| I04 · identity | Durable authentication and Sessions | Prepared | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/auth_service/durable_source/mod.rs)<br>[sessions.rs](../../engine/src/services/auth_service/durable_source/sessions.rs)<br>[session_source.rs](../../engine/src/services/auth_service/durable_source/session_source.rs)<br>[namespace.rs](../../engine/src/services/auth_service/durable_source/namespace.rs)<br>[source_relations.rs](../../engine/src/services/auth_service/durable_source/source_relations.rs)<br>[password_work.rs](../../engine/src/services/auth_service/durable_source/password_work.rs)<br>[password_profile.rs](../../engine/src/services/auth_service/durable_source/password_profile.rs)<br>[otp.rs](../../engine/src/services/auth_service/durable_source/otp.rs)<br>[otp_consumption.rs](../../engine/src/services/auth_service/durable_source/otp_consumption.rs)<br>[otp_storage.rs](../../engine/src/services/auth_service/durable_source/otp_storage.rs)<br>[schema.rs](../../engine/src/services/auth_service/durable_source/schema.rs)<br>[accounts.rs](../../engine/src/services/auth_service/durable_source/accounts.rs)<br>[0015_durable_otp_consumption.sql](../../engine/migrations/0015_durable_otp_consumption.sql) | [c1-auth](../../docs/en/collaboration-auth-source.md)<br>[session-boundary](../../docs/en/collaboration-auth-source.md)<br>[password-work](../../docs/en/collaboration-auth-source.md)<br>[password-profile](../../docs/en/collaboration-auth-source.md)<br>[otp-freshness](../../docs/en/collaboration-auth-source.md)<br>[otp-consumption-policy](../../docs/en/collaboration-auth-source.md)<br>[otp-consumption](../../docs/en/collaboration-auth-source.md) | Schema-2 OTP consumption, source pins, worker counts, PHC costs and pre-commit freshness are guarded. Timing policy, ingress/auth composition, recovery/migration and live cutover remain pending. |
| I05 · identity | Session-authorized policy commands | Prepared | 3 / 1 / 3 / 1 | [session_commands.rs](../../engine/src/services/auth_service/durable_source/session_commands.rs)<br>[session_adapter.rs](../../engine/src/services/collaboration_identity_store/session_adapter.rs) | [c1-session](../../docs/en/collaboration-session-commands.md) | Internal transaction-owned authorization only; no public management workflow. |
| I06 · identity | Account deletion and password rotation | Prepared | 3 / 1 / 3 / 1 | [deletion.rs](../../engine/src/services/auth_service/durable_source/deletion.rs)<br>[credentials.rs](../../engine/src/services/auth_service/durable_source/credentials.rs)<br>[password_work.rs](../../engine/src/services/auth_service/durable_source/password_work.rs)<br>[password_profile.rs](../../engine/src/services/auth_service/durable_source/password_profile.rs)<br>[otp.rs](../../engine/src/services/auth_service/durable_source/otp.rs)<br>[otp_consumption.rs](../../engine/src/services/auth_service/durable_source/otp_consumption.rs) | [c1-password](../../docs/en/collaboration-password-change.md)<br>[password-work](../../docs/en/collaboration-auth-source.md)<br>[password-profile](../../docs/en/collaboration-auth-source.md)<br>[otp-freshness](../../docs/en/collaboration-auth-source.md)<br>[otp-consumption-policy](../../docs/en/collaboration-auth-source.md)<br>[otp-consumption](../../docs/en/collaboration-auth-source.md) | No live account migration or recovery; cancelled waiting does not prove rollback. |
| O01 · operations | Explicit authority provisioning | Prepared | 3 / 2 / 3 / 2 | [main.rs](../../engine/src/bin/cyanrex-provision/main.rs)<br>[bootstrap.rs](../../engine/src/services/auth_service/durable_source/bootstrap.rs)<br>[password_work.rs](../../engine/src/services/auth_service/durable_source/password_work.rs)<br>[password_profile.rs](../../engine/src/services/auth_service/durable_source/password_profile.rs) | [c1-provision](../../docs/en/collaboration-provisioning.md)<br>[password-work](../../docs/en/collaboration-auth-source.md)<br>[password-profile](../../docs/en/collaboration-auth-source.md)<br>[otp-consumption](../../docs/en/collaboration-auth-source.md) | Local explicit empty-namespace tool, not startup bootstrap or old-data import. |
| O02 · operations | Read-only authority reconciliation | Prepared | 3 / 2 / 3 / 2 | [mod.rs](../../engine/src/services/auth_service/durable_source/reconciliation/mod.rs)<br>[mod.rs](../../engine/src/services/collaboration_identity_store/reconciliation/mod.rs) | [c1-reconcile](../../docs/en/collaboration-reconciliation.md) | A bounded read-only snapshot, not repair, permission or secret-delivery proof. |
| W01 · work | Revision-fenced Task storage | Prepared | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/task_store/mod.rs)<br>[replace.rs](../../engine/src/services/task_store/replace.rs) | [c2-database](../../docs/en/project-status.md) | Schema 2 fresh namespace only; no schema 1 migration, assignment or acceptance. |
| W02 · work | Immutable Artifact revisions | Prepared | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/artifact_store/mod.rs)<br>[blob.rs](../../engine/src/services/artifact_store/blob.rs) | [c2-database](../../docs/en/project-status.md) | Files and SQL are not one atomic write; no metadata mapping, sharing or garbage collection. |
| W03 · work | Immutable Review history | Prepared | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/review_store/mod.rs)<br>[records.rs](../../engine/src/services/review_store/records.rs) | [c2-database](../../docs/en/project-status.md) | Trusted direct caller; a stored rule judgment does not prove rule execution or Task acceptance. |
| W04 · work | Private manual Task commands | Prepared | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/auth_service/durable_source/task_commands/mod.rs)<br>[mod.rs](../../engine/src/services/auth_service/durable_source/private_work/mod.rs) | [c2-database](../../docs/en/project-status.md) | Internal own-task adapter; no public API, payload metadata or Task title edit. |
| W05 · work | Session-authorized content publication | Prepared | 3 / 1 / 3 / 1 | [artifact_commands.rs](../../engine/src/services/auth_service/durable_source/artifact_commands.rs) | [c2-database](../../docs/en/project-status.md) | No browser upload adapter; uncertain database outcome does not authorize file deletion. |
| W06 · work | Exact owned Task inputs | Prepared | 3 / 1 / 3 / 1 | [inputs.rs](../../engine/src/services/auth_service/durable_source/task_commands/inputs.rs) | [c2-database](../../docs/en/project-status.md) | Pins owned revisions with byte checks, not domain evidence or a Run. |
| W07 · work | Private human Review commands | Prepared | 3 / 1 / 3 / 1 | [review_commands.rs](../../engine/src/services/auth_service/durable_source/review_commands.rs) | [c2-database](../../docs/en/project-status.md) | Own content only; no cross-user grant, AI review or rule-authorized adapter. |
| W08 · work | Exact catalogue Task admission | Prepared | 3 / 1 / 3 / 1 | [catalog.rs](../../engine/src/services/auth_service/durable_source/task_commands/inputs/catalog.rs) | [c2-database](../../docs/en/project-status.md) | Admits definition metadata only; does not retain or execute a provider. |
| W09 · work | Draft input replacement | Prepared | 3 / 1 / 3 / 1 | [replacement.rs](../../engine/src/services/auth_service/durable_source/task_commands/inputs/replacement.rs) | [c2-database](../../docs/en/project-status.md) | No UI conflict recovery; publication is separate and old content/Reviews stay unchanged. |
| W10 · work | Task content manifest and snapshot checks | Prepared | 3 / 1 / 2 / 1 | [content.rs](../../engine/src/models/collaboration/content.rs)<br>[task_content.rs](../../engine/src/services/task_content.rs) | [content-contract](../../docs/en/task-content-manifest.md) | Pure supplied-snapshot checks only. Separate W11 persists metadata, but this validator establishes neither provenance nor authorization. |
| W11 · work | Atomic Task content storage | Prepared | 3 / 1 / 3 / 1 | [content.rs](../../engine/src/services/task_store/content.rs)<br>[content_commands.rs](../../engine/src/services/task_store/content_commands.rs)<br>[content_records.rs](../../engine/src/services/task_store/content_records.rs)<br>[schema.rs](../../engine/src/services/task_store/schema.rs)<br>[0014_collaboration_task_content.sql](../../engine/migrations/0014_collaboration_task_content.sql) | [content-storage](../../docs/en/task-content-store.md) | Trusted schema-3 primitive only; W12 adds separate authorization/text composition. No catalogue admission, public saving or migration. |
| W12 · work | Session-authorized Task content | Prepared | 3 / 1 / 3 / 1 | [content.rs](../../engine/src/services/auth_service/durable_source/task_commands/content.rs)<br>[mod.rs](../../engine/src/services/auth_service/durable_source/private_work/mod.rs)<br>[task_content.rs](../../engine/src/services/task_content.rs) | [session-content](../../docs/en/session-task-content.md) | Internal source-owned authorization/text checks; W13 is a separate unmounted HTTP adapter. No browser save, catalogue admission, migration or live-auth cutover. |
| W13 · work | Explicit Task content HTTP adapter | Prepared | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/platform_http/mod.rs)<br>[security.rs](../../engine/src/platform_http/security.rs)<br>[handlers.rs](../../engine/src/platform_http/handlers.rs)<br>[contract.rs](../../engine/src/platform_http/contract.rs)<br>[errors.rs](../../engine/src/platform_http/errors.rs) | [content-http](../../docs/en/task-content-http.md) | Explicit standalone router only, not mounted in the current app. No Session issuer, public Artifact publication, browser mapping/save, live OpenAPI/SDK or migration. |
| D01 · domain | Versioned domain catalogue | Prepared | 3 / 1 / 2 / 1 | [task_catalog.rs](../../engine/src/services/task_catalog.rs) | [release-049](../../docs/en/project-status.md) | Trusted static providers; non-teaching provider remains a fixture, not a product workflow. |
| D02 · domain | Versioned domain catalogue | Live | 3 / 2 / 2 / 1 | [learning_catalog.rs](../../engine/src/services/learning_catalog.rs)<br>[mod.rs](../../engine/src/domain_packs/ebpf_teaching/mod.rs) | [release-049](../../docs/en/project-status.md) | Only the teaching facade is wired; its assessment is not a generic persisted Review. |
| D03 · domain | Lab attempts feedback and resume | Live | 3 / 2 / 2 / 1 | [learning_store.rs](../../engine/src/services/learning_store.rs)<br>[learning.rs](../../engine/src/routes/learning.rs)<br>[teaching.tsx](../../frontend/pages/teaching.tsx) | [release-049](../../docs/en/project-status.md) | Legacy attempts and feedback; source resume does not rerun or migrate to Task/Review. |
| D04 · domain | Owner-scoped saved scripts | Live | 3 / 2 / 2 / 1 | [script_store.rs](../../engine/src/services/script_store.rs)<br>[scripts.rs](../../engine/src/routes/scripts.rs) | [release-049](../../docs/en/project-status.md) | Existing script identity and fallback policy; not generic Artifact storage. |
| R01 · execution | eBPF compiler and C assistance | Live | 3 / 2 / 2 / 1 | [check.inc.rs](../../engine/src/routes/ebpf/check.inc.rs)<br>[semanticCompletion.ts](../../frontend/src/utils/semanticCompletion.ts) | [release-049](../../docs/en/project-status.md)<br>[bughunt-local](../../docs/en/project-status.md) | Engine-backed C tooling may send source; separate from local Task language services. |
| R02 · execution | Local eBPF run observe and detach | Live | 3 / 2 / 2 / 1 | [handlers.inc.rs](../../engine/src/routes/ebpf/handlers.inc.rs)<br>[attach.inc.rs](../../engine/src/services/ebpf_loader/attach.inc.rs)<br>[useRuntimeActions.ts](../../frontend/src/features/ebpf/useRuntimeActions.ts) | [release-049](../../docs/en/project-status.md)<br>[kernel-history](../../reports/acceptance/2026-09-09-kernel-vm/README.md) | No current kernel acceptance; privileged shared kernel is not hostile-student isolation. |
| R03 · execution | Local capacity leases and quotas | Live | 3 / 2 / 2 / 1 | [runner_manager.rs](../../engine/src/services/runner_manager.rs)<br>[runner_driver.rs](../../engine/src/services/runner_driver.rs) | [release-049](../../docs/en/project-status.md) | In-memory admission; no durable Run, restart recovery or multi-Engine attachment ownership. |
| R04 · execution | Signed Agent probe and compile jobs | Live | 3 / 2 / 3 / 2 | [runner_agent_client.rs](../../engine/src/services/runner_agent_client.rs)<br>[runner_agent_executor.rs](../../engine/src/services/runner_agent_executor.rs)<br>[runner_job_queue.rs](../../engine/src/services/runner_job_queue.rs) | [release-049](../../docs/en/project-status.md)<br>[agent-history](../../reports/releases/0.4.3/README.md) | No remote kernel execution; jobs/registration are ephemeral. Self-reported isolation is not VM/container proof; no implicit fallback. |
| E01 · delivery | Owner telemetry and bounded recovery | Live | 3 / 2 / 2 / 1 | [event_bus.rs](../../engine/src/services/event_bus.rs)<br>[eventStream.ts](../../frontend/src/features/events/eventStream.ts)<br>[useEventActions.ts](../../frontend/src/features/events/useEventActions.ts) | [release-049](../../docs/en/project-status.md) | Recent-history best effort; no business-event cursor, exactly-once or cross-Engine ordering. |
| E02 · delivery | Transactional resource outbox records | Prepared | 3 / 1 / 3 / 1 | [records.rs](../../engine/src/services/task_store/records.rs)<br>[records.rs](../../engine/src/services/artifact_store/records.rs)<br>[records.rs](../../engine/src/services/review_store/records.rs)<br>[content_records.rs](../../engine/src/services/task_store/content_records.rs) | [c2-database](../../docs/en/project-status.md)<br>[content-storage](../../docs/en/task-content-store.md) | Atomic recording only; no delivery service or consumer replay. |
| E03 · delivery | Public API and SDK compatibility | Live | 3 / 2 / 2 / 2 | [application.rs](../../engine/src/application.rs)<br>[openapi.json](../../engine/openapi/openapi.json)<br>[index.ts](../../sdk-js/src/index.ts) | [release-049](../../docs/en/project-status.md) | Generic preparation commands are not public; registry publication/support ownership pending. |
| O03 · operations | Instance startup and diagnostics | Live | 3 / 2 / 2 / 2 | [start.sh](../../start.sh)<br>[config.rs](../../engine/src/config.rs)<br>[state.rs](../../engine/src/state.rs) | [release-049](../../docs/en/project-status.md) | Health is not kernel readiness; startup does not initialize prepared stores. |
| O04 · operations | Package verification and SSH deployment | Live | 3 / 2 / 2 / 2 | [package.rs](../../engine/src/bin/cyanrex-release/package.rs)<br>[ssh_cli.rs](../../engine/src/bin/cyanrex-release/ssh_cli.rs) | [release-049](../../docs/en/project-status.md) | Preinstalled package targets only; no fresh candidate/LAN/SSH acceptance or bare-host bootstrap. |
| O05 · operations | Settings metrics and Agent administration | Live | 3 / 2 / 2 / 2 | [useSettingsForm.ts](../../frontend/src/features/settings/useSettingsForm.ts)<br>[usePerformanceMetrics.ts](../../frontend/src/features/settings/usePerformanceMetrics.ts)<br>[useRunnerAgentAdmin.ts](../../frontend/src/features/runner/useRunnerAgentAdmin.ts) | [release-049](../../docs/en/project-status.md) | Legacy teacher authority; uncertain mutations require explicit reconciliation, not blind retry. |
| O06 · operations | Module headers and typed commands | Live | 3 / 2 / 2 / 1 | [module_manager.rs](../../engine/src/services/module_manager.rs)<br>[c_header_module.rs](../../engine/src/services/c_header_module.rs)<br>[command_dispatcher.rs](../../engine/src/services/command_dispatcher.rs) | [release-049](../../docs/en/project-status.md) | Module state is not executable plugin loading; terminal is not an arbitrary shell. |
| P01 · work | Browser to server Task save | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | W10-W13 provide contracts, storage, Session/text checks and an unmounted HTTP boundary; secure login/installation, publication, draft mapping and browser save/read/conflict/outcome handling remain absent. |
| P02 · identity | Live authority cutover and migration | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Inventory data, restore backups, fence old writers and review install/migration/rollback before routing. |
| P03 · work | Sharing and cross-user Review | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Explicit content grants and reviewer delegation before any private-owner relaxation. |
| P04 · domain | Artifact evidence to authorized rule Review | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Bind exact bytes and policy/provider version to authorized typed evidence and stored judgment. |
| P05 · work | Generic Task acceptance | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Define acceptance policy/state and explicit Review effect; execution success is insufficient. |
| P06 · execution | Durable generic Run orchestration | Planned | 1 / 0 / 0 / 0 | [references.rs](../../engine/src/models/collaboration/references.rs) | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Run reference exists, not scheduler/store; define inputs, leases, cancellation and restart reconciliation. |
| P07 · delivery | Reliable business-event delivery | Planned | 1 / 0 / 0 / 0 | [event.rs](../../engine/src/models/collaboration/event.rs) | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Envelope and rows exist, not dispatcher, consumer deduplication or cursor replay. |
| P08 · execution | Managed isolated student runtimes | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Implement unprivileged control, VM ownership, reset/quarantine and cleanup acceptance. |
| P09 · ui | External LSP and project workspaces | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Define file/process authorization, transport, lifecycle and resource isolation. |
| P10 · execution | Bounded AI delegation | Planned | 1 / 0 / 0 / 0 | [identity.rs](../../engine/src/models/collaboration/identity.rs) | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Agent principal is a type, not planner, tool authorization, budget or independent Review. |
| P11 · delivery | External ecosystem adapters | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | No implemented partner adapter; external project assumptions have not been verified. |
| P12 · domain | Executable domain plugins | Planned | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Define installation, trust, runtime loading and revocation independently of manifests. |
<!-- capability-tensor:end -->

## Representative paths and missing connections

`→` means current-application or local wiring; `⇢` means explicit internal composition; `×→` means a
missing edge. Arrows represent content, calls or dependencies, **not transferable authorization**.
Each private command independently rechecks current Session/account/member/owner: e52–e56 record
these authorization edges explicitly. e57–e59 record that all three resource stores write outboxes.
e64 adds separate content-storage outbox recording. e66 now leads through W12's dedicated Session
adapter; e67–e69 expose its storage and text checks. e71/e72 add explicit W13 HTTP composition;
browser e70 now targets W13 and remains missing, as does installation/authority e73.
The JSON retains all 73 edges and their failure/trust boundaries.

| Representative path | Coordinates in order | Status and boundary |
|---|---|---|
| Local content editing | U01 → U02 → U04 → U05 | Connected in the browser, with mocked Engine shell tests; content is not saved to the server. |
| Existing teaching workflow | A02 → A01 → D04 → R01 → R03 → R02 → D02 → D03 | Source-wired workflow; not a fresh real LAN/kernel end-to-end acceptance. |
| Optional remote compilation | R01 → R04 | Compile/probe only; no remote loading, artifact return or implicit local fallback. |
| Private content and Task revisions | I04 ⇢ W05 ⇢ W06 ⇢ W09 ⇢ E02 | Explicit preparatory composition; content publication and Task update are separate operations. |
| Private human review | I04 ⇢ W05 ⇢ W07 ⇢ W03 | Private owner only; no cross-user review or automatic acceptance. |
| Missing server save bridge | U02 ×→ P01 ×→ W05 ⇢ W06 ⇢ W09 | Missing browser/public bridge blocks a saved user workflow despite tested backend components. |
| Missing live authority cutover | U01 ×→ P02 ×→ I04 ⇢ W05 | Requires an explicit installation/authentication decision and reviewed recovery/migration. |
| Missing rule review and acceptance | W02 ×→ P04 ×→ W03 ×→ P05 | No transitive acceptance from content, catalogue, execution or stored judgment. |
| Missing generic execution bridge | W01 ×→ P06 ×→ R03 ×→ P08 | Durable orchestration and isolated resource ownership are separate missing boundaries. |
| Missing business-event delivery | W09 ⇢ E02 ×→ P07 | Store-side atomicity stops at the outbox; no delivery or replay claim. |
| Explicit prepared authority setup | O01 ⇢ I04 ⇢ O02 | Local operator tooling exists; no deployed-data migration, repair or live source switch. |
| Package to existing runtime | O04 → O03 → A01 | Source/tooling path only; actual target apply and acceptance remain explicit separate work. |
| Supplied content snapshot | W01 ⇢ W10 | Pure Task/metadata matching; W02 ⇢ W10 adds byte/pin consistency. Neither edge reads storage or authorizes a Session. |
| Trusted content persistence | W10 ⇢ W11 ⇢ E02 | Complete metadata is stored atomically in a separate namespace; no Session/Artifact-byte adapter or browser save. |
| Authorized content editing | I04 ⇢ W12 ⇢ W11 ⇢ E02 | Current Session and old/new text verified on one transaction; e68/e69 supply content checks, but public/browser e70 is missing. |
| Explicit HTTP to authorized content | W13 ⇢ W12 ⇢ W11 ⇢ E02 | Standalone router tested with in-process HTTP and real SQL; not mounted in the app or connected to browser/login issuance. |

Wired does not mean newly accepted end to end. In particular, do not merge these concepts:

- Runner is not a durable generic Run; Task Cancelled does not automatically cancel a Runner job.
- Runner Agent is not an AI Agent; advertised isolation is not proof of isolation.
- Atomic outbox rows are not reliable delivery; legacy EventBus is not the generic business-event bus.
- Local language workers are not external LSP; module manifests are not executable plugins.
- Teaching feedback is not generic Review; a Review does not itself accept a Task.
- A manual VM kernel test is not product-managed VM lifecycle; a workspace role is not a deployment Grant.

## Suggested order for closing gaps

These are recommendations derived from missing edges, not shipped commitments. Close one private,
non-teaching saved-content workflow before expanding sharing or execution.

| Order | Coordinates or edges | Contract and verification needed |
|---|---|---|
| 1 | P01, e36/e37/e70 | W10–W13 provide contracts through an unmounted HTTP adapter. Define secure Session issuance, explicit installation, public publication, whole-draft mapping and browser conflict outcomes; do not silently switch legacy instances. |
| 2 | P02, e38/e39 | An explicit preparation-layer connection plan; if adopting old data, verify backup/restore, migration and writer fencing first. No operations level 3 without recovery evidence. |
| 3 | U02 → P01 → W05/W06/W09 | Explicit save/read, restart recovery, revoked access, concurrent editing and uncertain outcomes. Publication/replacement failure does not permit blind deletion or retry. |
| 4 | P03, P04, P05 | Sharing/reviewer grants; exact content to typed evidence to version-bound rule Review. Define acceptance policy separately. |
| 5 | P06, P07, P08 | Independently test durable Runs, recoverable delivery and isolated resource ownership; do not substitute broader Runner privileges. |

For further bug hunts, select by edge: U02/U04/U06 for generations, revisions and asynchronous
replacement; I04→W* for revocation and lock waiting; W*→E02 for commit/acknowledgement faults. For P*
gaps, first define contracts and rejection tests rather than obtaining zero-test success on an absent workflow.

## Evidence catalogue and freshness

| Evidence ID | Date | Record and limit |
|---|---|---|
| [otp-consumption](collaboration-auth-source.md) | 2026-10-04 | 11 exact consumption and four schema SQL cases after reproduced double use. Independent sources, atomic rollback, collisions, old-format refusal and fresh initialization; no live switch, migration or commit-ack recovery proof. Adjacent/gate totals are in project status. |
| [otp-consumption-policy](collaboration-auth-source.md) | 2026-10-04 | Earlier pure-only stage: 16 tests and three source guards for watermarks, collisions, binding and input/time limits. At that stage it was unwired; atomic integration has its own later record above. |
| [otp-freshness](collaboration-auth-source.md) | 2026-10-03 | Historical freshness-only stage: seven private-clock SQL and 11 pure tests for expiry, rollback, success and reuse. Consumption was absent then and integrated later; neither stage guarantees commit-ack freshness or live cutover. |
| [password-profile](collaboration-auth-source.md) | 2026-10-03 | Six exact prepared PHC SQL cases and 25 profile/worker units, after a safe low-cost rejection regression. Fixed parameters, writer compatibility, existing Sessions and post-write rollback; no migration, legacy switch, process RSS/time bound or CLI acceptance. Adjacent reruns are recorded in project status. |
| [password-work](collaboration-auth-source.md) | 2026-10-03 | 11 worker lifecycle/real-hash unit tests and two five-call-site wiring guards in the preceding slice. Four dispatched/twenty total prepared jobs only; that slice did not bound individual PHC cost or legacy work or add public issuance/CLI acceptance. Database reruns are recorded separately in project status. |
| [session-boundary](collaboration-auth-source.md) | 2026-10-03 | 14 exact source-only Session cases on private PostgreSQL: false logout acknowledgement, namespace/table pins, post-write facts, password-worker gap and lock ordering. No public issuer or live-auth cutover; old-suite and gate reruns are recorded separately in project status. |
| [content-http](task-content-http.md) | 2026-10-03 | 11 exact PostgreSQL 16 HTTP-to-C2-M cases over a private Unix socket; in-process routing, current authorization and storage faults, not a listener/browser/TLS or live-auth cutover. Full gate and preceding-suite totals are recorded in project status. |
| [session-content](session-task-content.md) | 2026-10-03 | 25 exact PostgreSQL 16 Session-content cases over a private Unix socket; old/new text, authorization, namespace and transaction boundaries, not public/browser saving. Full gate and preceding-store totals are recorded in project status. |
| [content-storage](task-content-store.md) | 2026-10-03 | 21 exact PostgreSQL 16 content-store cases over a private Unix socket; trusted schema 3 only, not Session or browser saving. Shared regression totals are recorded separately in project status. |
| [content-contract](task-content-manifest.md) | 2026-10-03 | 12 metadata and 14 supplied-byte regression cases; pure internal contract, no authorized storage or public save. Full checks are dated separately in project status. |
| [release-049](../../docs/en/project-status.md) | 2026-10-03 | 0.4.9 source release checks: 402 default Rust, 395 ignored, 92 scripts, 164 frontend, 16 SDK; 59 separate browser cases. No fresh opt-in database, kernel or LAN acceptance. |
| [bughunt-local](../../docs/en/project-status.md) | 2026-10-03 | Unreleased bug hunt after 0.4.9: 165 frontend, 92 scripts and 72 browser cases across seven named suites. Engine responses mocked; real local Monaco/file behavior. This precedes the tensor review. |
| [c2-database](../../docs/en/project-status.md) | 2026-10-03 | C2-J implementation run: 155 selected Task/Artifact/Review PostgreSQL cases, not every identity/lifecycle suite. Separate from release/browser runs. |
| [c1-identity](../../docs/en/collaboration-identity-lifecycle.md) | 2026-09-27 | Original C1-D isolated PostgreSQL identity binding/retirement and preceding registry/policy cases; historical scope, not current deployment. |
| [c1-policy](../../docs/en/collaboration-policy-audit.md) | 2026-09-27 | Original C1-C isolated PostgreSQL membership/grant audit evidence; no live cutover. |
| [c1-auth](../../docs/en/collaboration-auth-source.md) | 2026-09-27 | Original C1-E durable authentication source PostgreSQL evidence; public AuthService was not switched. |
| [c1-session](../../docs/en/collaboration-session-commands.md) | 2026-09-27 | Original C1-F current-Session transaction composition evidence; internal staging only. |
| [c1-password](../../docs/en/collaboration-password-change.md) | 2026-10-02 | One default and 17 password-lifecycle PostgreSQL cases, plus separately listed preceding regressions; no live recovery. |
| [c1-provision](../../docs/en/collaboration-provisioning.md) | 2026-10-02 | Explicit CLI and disposable PostgreSQL, including loopback SCRAM; no installed platform cutover. |
| [c1-reconcile](../../docs/en/collaboration-reconciliation.md) | 2026-10-02 | C1-K development run on 2026-10-02 (also dated in development-history.md), including read-only SQL/CLI and SCRAM fixtures; not repairs or migration. |
| [design](../../docs/en/project-status.md) | 2026-10-03 | Current remaining boundaries and proposed next steps; no execution evidence. |
| [target](../../docs/zh-CN/next-architecture.md) | 2026-10-03 | Reviewed target architecture; its original 0.4.3 inventory and external-project assumptions remain historical, not verified integrations. |
| [agent-history](../../reports/releases/0.4.3/README.md) | 2026-09-25 | Historical signed loopback HTTP and Clang integration; not current 0.4.9 Agent transport, TLS/LAN or remote kernel acceptance. |
| [kernel-history](../../reports/acceptance/2026-09-09-kernel-vm/README.md) | 2026-09-09 | Historical manual VM/native dirty-build tracepoint acceptance, candidate null. Does not establish managed VM isolation or current kernel acceptance. |

Dates belong to the named records; design/target dates are review dates, not test dates. The initial
tensor review added inventory and consistency checks; C2-K through C2-N add separately recorded pure-contract
and isolated database execution. Prior frontend/browser, release and database totals are not combined into fresh acceptance. V3 permits explicitly scoped historical real
database/service evidence. R04's old loopback service evidence supports only its named protocol scope;
kernel history is retained as reference, not proof of the current complete run/attachment/deployment path.

## Maintenance and validation

For capability changes, update JSON coordinates, source/test references, dated evidence and gaps first,
then this page and its Chinese reading view. Connected edges should name authorization ownership,
error/cancellation outcomes and persistence responsibility. A changed score needs new evidence for its
gate. Do not rewrite the historical 0.3.8 [functional-network.json](../functional-network.json) to imply
current coverage.

```bash
node scripts/check-capability-tensor.mjs
node --test scripts/tests/capabilityTensor*.test.mjs
node frontend/scripts/sync-course-docs.mjs
node frontend/scripts/sync-course-docs.mjs --check
```

`renderTensorRows(data, locale)` generates the score table above. Replace the section between the
`capability-tensor:start/end` markers in both reading views; common script tests reject table/data drift.
Structural validation rejects dangling references, duplicate coordinates, escaping paths, invalid
scores, planned nodes on connected edges and mock/historical evidence promoted to C3/O3. Human review
still owns the truth of a score. The validator does not execute referenced tests, detect implementation
content changes or provide security certification.

Useful slices: `architecture == work` for resource work; `C == 1 && V == 3` for tested-but-unwired
preparation; `edges.state == missing` for blockers. Join `implementation` to source/tests and evidence
IDs to execution scope. Never fill unknown coordinates with zero or average across these slices.
