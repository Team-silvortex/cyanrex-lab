# Current platform feature and module network

**2026-10-16, included in 0.6.2:** the [bound Session path](platform-session-connection.md) is
retained storage → one fresh connection → original catalog/markers and Source pins in one business
transaction → fresh Session/absence read → confirmed commit → explicit HTTP projection. It has no
pool fallback, issuer or normal-app mounting; Task/Artifact resource composition remains separate.

[Format 3 inspection](platform-marker-expectation.md) separately follows trusted catalog/marker
declaration → one fresh connection → all identities before marker rows → close/discard, not host startup.

For a concise source-responsibility and live/prepared boundary view, start at the
[current architecture map](architecture-map.md). This page retains the detailed feature edges.

Source release **0.5.8** (**2026-10-13**) includes these follow-ups. Dated `Unreleased` labels retain
their original implementation stage, source versions and verification scope. Source inclusion does not
mount preparation APIs, connect browser saving, switch live authentication, migrate storage or validate
a running deployment.

The [ordered download](large-file-download-client.md), [manifest HTTP](large-file-manifest-http.md)
and [exact manifest client](large-file-manifest-client.md), selected saving, observation/cancellation
and paired distribution router are now included in 0.5.8; dated 0.5.7 evidence stays separate.

Base source review date: **2026-10-03**. Current scope: **0.5.8 source**, including prepared-authentication
hardening, the 0.5.0 navigation/filename fixes and C2-K/L/M/N content preparation, alongside the preceding
collaboration commands and local task payload editor. The review retains its
0.4.9 base commit and dated evidence; source inclusion is not a new release acceptance report or a
claim about an installed deployment.

The 2026-10-07 [login-denial follow-up](collaboration-auth-source.md), included in 0.5.2, also routes missing
accounts through bounded password work. It changes neither this topology nor the missing public
Session-issuance connection, and does not establish constant-time login.
The same release's explicit expired-Session cleanup is also internal-only, limited to 128 selected rows
per transaction; no scheduler, operator endpoint or global storage quota is connected.
Source Session lookup and reconciliation in 0.5.2 also guard stored expiry before decoding; this is internal
read hardening. The subsequent [registry-time follow-up](collaboration-reconciliation.md) guards
nullable retirement and both audit times in their shared readers, without changing topology, schema
or age/order policy. Neither slice proves every database timestamp safe.

0.5.3 [C2-O](task-draft-publication.md) adds a separate pure draft importer/publication plan and
supplied-content binding coordinate. Its new internal connections do not publish bytes or complete
the browser/public save bridge.

0.5.3 [C2-P](session-task-draft-publication.md) separately joins the plan to existing authorized
Artifact publication and Task creation, one explicit step at a time. Its caller-owned outcomes are
in-memory only; failed or dropped steps stop advancement, not undo earlier commits. No browser or
public route is connected.

0.5.3 [C2-Q](session-task-draft-observation.md) adds a read-only branch for one fixed unconfirmed
target. Every read uses the original current Session; its body-free result leaves the attempt unchanged
and does not establish the earlier write’s outcome or enable retry.

0.5.3 [C2-R](session-task-draft-checkpoint.md) exports allocated targets and reported progress as
bounded metadata. Its pure parse result is caller data, not an attempt, journal or authority. Export
does not save, read storage or invoke C2-Q; the codec itself supplies no recovery connection.

0.5.3 [C2-S](session-task-draft-inspection.md) separately combines a parsed checkpoint with an
explicit trusted workspace and current Session for one existing authorized read. It compares current
metadata without an original attempt, not original caller/body provenance or storage incarnation.
It does not restore C2-Q, adopt confirmations or supply a journal or retry policy.

0.5.4 [C2-T](session-task-draft-intent.md) separately connects a real Ready/zero-confirmed
attempt to immutable intent registration and a current-Session record read. The journal binds exact
owner/account generation and configured namespace names, not resource dispatch or outcome history.
Original C2-P `advance` still does not require the record; browser saving remains absent.

0.5.4 [C2-U](session-task-draft-dispatch.md) adds a separate consumed wrapper with schema 2
step records. An acknowledged Unknown reservation precedes each resource transaction; exact current
identity, intent and nonce checks couple its completion marker with Artifact or Task SQL. This is a
live internal path only, not restoration from records, automatic retry or a public/browser connection.

0.5.4 [C2-V](session-task-draft-journal-inspection.md) separately reads the schema-2 intent and
recorded step prefix under exact current owner/account authority. Counts include the final Task;
the optional Unknown target exposes no nonce. It neither reads resources nor restores the wrapper,
and no record or absence permits retry, cleanup or adoption.

0.5.4 [C2-W](session-task-draft-journal-observation.md) separately reads one recorded resource
after capturing the journal in the same authorized transaction, then rechecks the full original prefix
and first pins. Body-free output separates recorded status from current metadata agreement; this read
does access files, unlike C2-V, but grants no receipt, recovery, retry or public/browser connection.

For architecture × capability × implementation × maturity, use the new
[capability tensor and score rubric](capability-maturity.md) and its
[machine inventory](../platform-capability-tensor.json): 70 implementation coordinates, 114 directed
edges and 19 representative paths. This page remains the route/module topology view; scores and
evidence are maintained separately, with no aggregate completion percentage.

Cyanrex is moving from an eBPF teaching application toward a task-oriented collaboration platform.
Three areas coexist: the connected teaching runtime, the separately prepared generic backend, and a
local-only task editor. Code is one optional kind of task payload; neither a task nor the platform
requires code. The important unfinished work is connecting these areas without transferring the
legacy teacher role or browser-supplied identities into generic permissions.

0.5.3 [AI Agent integration](ai-agent-integration.md) connects a separate Settings panel to
teacher-only, revision-fenced profile metadata storage. A caller-owned SDK bridge adapts explicitly
selected existing tools; the host owns provider transport, secrets and approval. This adds neither
an autonomous AI identity/delegation edge nor the missing generic Task browser-save connection.

The independent **2026-10-11 Unreleased [task-content index](task-content-index.md)**, on source
0.5.5, connects trusted schema 3 metadata reads and current-Session owner-private listing. A default
20-entry page is bounded to 1–32 plus one strictly checked lookahead; neutral catalogue configuration
admits manual and complete configured definitions. Cursor position is not authority, content availability
or whole-store health. No Artifact/file read, public edge, UI, write, installer, migration or recovery is added.

The separate same-day **Unreleased [index HTTP adapter](task-content-index-http.md)** on source
0.5.5 adds only explicit GET transport → existing current-Session index → bounded metadata projection.
Its exact `/platform/v1/task-index` route has strict raw pagination, private headers and shared
response-lifetime admission. The route remains unmounted; old content routers and all browser,
SDK/OpenAPI, storage, execution and recovery connections remain unchanged.

The separate **2026-10-11 Unreleased [large-file core](large-file-core.md)** introduces
strict shared manifest → bounded supplied-stream inspection/chunk copy/whole-file verification.
Preopened Unix descriptor checks observe source metadata changes without opening paths. This
transport-neutral branch neither enters Artifact/Task storage nor mounts HTTP. Its separate Unix
[staging branch](large-file-staging.md) adds exact manifest/index binding → independent worker
admission → exclusive private chunk → sync/readback. A separate [upload-intent ledger](large-file-upload-intent.md)
adds trusted owner/account generation → immutable manifest → atomic logical quota reservation.
The separate [upload-bound local receipt](large-file-upload-staging.md) connects exact intent/source pins
→ exclusive staged bytes → original-file confirmation. The independent [chunk journal](large-file-chunk-journal.md)
adds acknowledged Unknown → exclusive file work → recorded confirmation → fresh original-source/file checks.
Separate [whole-byte observation](large-file-whole-verification.md) connects the original upload binding
→ ordered current chunk/whole digest → fresh original-source check → original file snapshot recheck,
without a journal-state, recovered-receipt or publication edge.
Separate [persistent publication metadata](large-file-publication.md) connects original source and
complete confirmed journal → fresh whole bytes → immutable insert acknowledgement → original
source/journal/publication/same-file checks. Metadata reads do not recreate its one-shot receipt,
prove current health, authorize downloads or settle an Unknown caller result.
Separate [Session private metadata](large-file-session-metadata.md) connects current audited Human
binding → derived owner/account → own reservation or selected upload/Published metadata → original
pin/record checks → fresh Session/policy checks → source commit acknowledgement. It has no byte IO
or publisher edge. Separate [pure file-grant contracts](large-file-access-contract.md) connect supplied
account/version metadata to exact active/time matching only, not authenticated or persisted authority;
publisher and recipient revocation are independent. Reviewed recovery, physical allocation, audited grants,
central transfer and later desktop pairing remain
missing edges in [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2).
The existing capability tensor's dated inventory and maturity evidence are unchanged.

An [Unreleased publisher-grant follow-up](large-file-publisher-grants.md), outside the 0.5.6 tag,
adds trusted issue → immutable audit → CAS revocation, and current audited Session → exact own
account → selected grant/history fence → final authorization → commit acknowledgement. No edge
authenticates a grant manager or consumes publishing permission; recipient grants, byte transport and
live mounting remain missing. The dated tensor scores are not refreshed by this implementation note.

Later [Session grant management](large-file-grant-management.md) adds trusted exact manager config
→ current audited actor → issue-target checks or historical-subject revocation → original complete
ledger/deferred/source checks → fresh Session → source commit. It adds no Registry role permission,
durable online config revocation, publisher consumption, recipients, bytes or public edge; dated tensor
scores and evidence remain unchanged.

Independent [Session publication](large-file-session-publication.md) adds current exact actor/grant
admission → lock-free real bytes → original source/Registry/grant re-entry → fenced upload/journal and
create-only insert → deferred/full-ledger/final fresh actor and expiry → source commit → original
post-acknowledgement checks. It is an explicit internal edge, not recipients, transport or live mounting;
the dated tensor inventory, scores and evidence remain unchanged.

The later [recipient-grant edge](large-file-recipient-grants.md) is trusted exact-file issue/revoke →
independent retained ledger/history → current own-recipient selection → original three-table SHARE
fence and final source authorization → acknowledged metadata. It does not connect a Published lookup,
download gate or bytes; publisher revocation does not cascade, and dated tensor scores remain unchanged.

The October 12 [publisher-owned sharing edge](large-file-sharing.md) connects current publisher →
original own grant and exact Published selection → current recipient → recipient write/deferred exact
delta → original records and final fresh permission/Session → acknowledged source COMMIT. Independent
owner revocation skips publishing permission and publication reads. No download or byte edge is added;
the dated tensor snapshot is unchanged.

The independent [download-manifest edge](large-file-download-manifest.md) is current recipient → own
Active exact-file grant → recorded publisher/account and exact Published version → original records →
final fresh grant/Session clock → acknowledged source commit → private manifest. No byte, reusable
permission, transport or client-save edge is connected; the dated capability tensor is unchanged.

The new [chunk-byte edge](large-file-chunk-read.md) is exact current recipient eligibility and
acknowledged source commit → lock-free private bounded file verification → original Session/metadata
reentry and final fresh grant/Session clock → acknowledged source commit → leased immutable bytes.
This Source edge itself connects no HTTP, client-save or restart edge. The dated tensor is unchanged.

The independent [binary HTTP edge](large-file-chunk-http.md) is strict exact GET and private request
admission → one existing current-recipient chunk command → fixed binary projection → leased Bytes
frame. Both permits survive clones/slices; a ten-second first-frame window rejects late polling.
This router is unmounted, not a listener/client-save/delivery edge. The dated tensor stays unchanged.

The separate [receiver edge](large-file-chunk-receiver.md) is supplied exact response → strict head
and fail-fast controls → private bounded frame/EOF/digest checks → existing exclusive staging job
owning buffer/control → this call's opaque local completion metadata. It connects no active fetch,
whole-file/resume or runtime edge; the dated tensor remains unchanged.

The explicit [client edge](large-file-chunk-client.md) is fixed trusted origin + exact per-call
selectors/Session → whole-call admission → controlled HTTPS/opt-in literal-loopback headers →
framed receiver → existing exclusive staging with request/buffer/worker ownership → opaque local
acknowledgement. This `fetch_and_stage` edge adds no whole-file/resume or discovery; dated scores stay unchanged.

The separate [private assembly edge](large-file-chunk-assembly.md) is existing original-root `v1`
chunks → fail-fast shared actual-job admission → exact ordered/whole preverification → exclusive
manifest-bound `f1` copy → 0400 sync and original-inode byte readback → original input/root recheck →
this call's opaque local acknowledgement. It adds no user save-path, network scheduler, adoption,
resume, publication or live mounting edge; the dated tensor is unchanged.

## Status vocabulary

Included in 0.5.8, the [ordered download edge](large-file-download-client.md) is supplied expected
manifest → independent whole-flow admission → per-index HTTP/receiver/stage ACK → same-root assembly.
First error/drop stops future requests; the current real file job retains flow capacity. Empty files
skip HTTP without remote authority. This edge adds no manifest fetch, final user save, resume or runtime
mounting and does not update dated scores.

The independent Unreleased [manifest HTTP edge](large-file-manifest-http.md) is six strict raw
selectors/private credentials → HTTP admission/actual empty body → one current-recipient Source
observation → bounded complete manifest JSON → body-lifetime release. The adapter itself adds no
filesystem IO, final byte permission or live mounting; dated scores remain unchanged.

The new [exact manifest client edge](large-file-manifest-client.md) is expected full version →
original flow admission (for composition) → independent metadata admission → controlled fixed GET →
strict framed EOF/JSON/full-version match → ordered chunk ACKs → same-root private assembly.
Empty granted files still require GET; the original supplied-manifest entry keeps zero-HTTP empty
assembly. Each later Source chunk reauthorizes. No UI save/resume, live mounting or tensor rescore.

The new [explicit private save edge](large-file-save.md) is original assembly descriptor proof +
independently host-selected private parent/basename → shared staging-worker admission → actual
original-source verification → exclusive new checked copy → sync/original-inode readback → final
source/output/root checks → local saved metadata. It makes no GET and adds no public/ordinary-folder
UI, overwrite/adoption, resume, automatic cleanup or dated rescore.

The new [authorized delivery call](large-file-delivery-client.md) joins these stages in one explicit operation.
One original whole-flow lease remains owned by actual chunk/assembly/save work after cancellation;
first failure stops with no retry/adoption, and UI saving, resume and normal mounting remain separate.

The separate [owned user-directory target](large-file-user-save.md) explicitly accepts 0700/0750/0755
without relaxing private staging. Original directory UID/GID/mode checks and checked saving remain;
file picking, writable shared folders, resume and live mounting are separate.

The optional [live delivery observer](large-file-delivery-progress.md) exposes one latest per-call phase
and actual chunk-ACK prefix without callback/back-pressure queues. Full staging is not saved output;
closure is not job termination or a receipt, and UI/durable progress/resume remain separate.

The optional [cooperative cancellation handle](large-file-delivery-cancellation.md) stops one call's
waiting when its request is selected. Handle drop is not cancellation; already-dispatched jobs keep
resources and may finish, and a request is not an instantaneous dispatch fence or cleanup authority.

Explicit [whole-delivery HTTPS verification](large-file-delivery-https.md) exercises the existing
chain with disposable native TLS peers. Transport identity is not current file authorization;
loopback acceptance does not establish deployed LAN, UI or unprivileged-service isolation.

An explicit [distribution router composition](large-file-distribution-router.md) now derives both
endpoints from one supplied Source and frozen chunk workspace. HTTP budgets remain independent;
this cached configuration consistency is not a Session issuer, live mounting or privilege isolation.

Unreleased after the 0.5.8 tag, [real Source over HTTPS](large-file-source-https.md) adds one
combined native authorization/file/router/TLS/saving test chain, including pre-dispatch revocation
and actual response-body cancellation. It changes no production listener, policy or dated score.

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

The 2026-10-08 Unreleased follow-up validates portable language hints in the task layer, not against
that profile registry. Unknown valid hints remain intact in import/edit/export; capability resolution
uses plain text with a visible no-tools notice, without itself editing metadata/revisions or undo.

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

The metadata split included in source release 0.5.5 adds [`TaskDefinitionCatalog`](../../engine/src/services/task_catalog/definitions.rs):
explicit, frozen metadata independent of evaluator code. Its original `new` registers one package. Task drafts and
Session catalogue workspaces consume this metadata directly; `TaskCatalog<P>` retains typed assessment.
Existing constructors, exact-version checks and authorization transactions remain compatible, with no
new route, schema, authority or execution connection.

The **2026-10-11 Unreleased multi-package follow-up**, still on source 0.5.4, adds trusted explicit
metadata composition: 1–256 configured catalogues → at most 256 flattened definitions → the existing
Task draft, schema 2 Session catalogue or schema 3 store/Session-content constructor. Nested composition
has the same global definition limit. Disjoint same-package registrations and distinct exact versions
retain their order; complete-reference overlap is rejected rather than deduplicated or overwritten.
This extends internal configuration, not provider composition, discovery, publisher authentication,
execution, public HTTP or the manual draft-publication workflows.

The separate **2026-10-11 Unreleased assessment-binding follow-up**, on source 0.5.4, admits an
explicitly unbound definition alongside bound entries in that same metadata path. Both schema/policy
JSON fields are required and must both contain references or both be null; an unbound catalogue Task is
not a manual Task. Existing full-definition checks reject adding or removing bindings under the same
reference. Typed-provider registration/dispatch and assessment results remain fully bound. The internal
Rust fields become paired `Option` values, while old bound JSON stays unchanged; old binaries reject
new double-null records. This adds no topology, authorization, schema number, migration or public
HTTP/editor/Review/execution connection. See [the contract](task-domain-boundary.md).

## Prepared collaboration backend

The shared model distinguishes Authority, Principal, Workspace, membership, deployment permission,
Task, immutable Artifact revision and Review. Their identifiers are scoped references, not usernames,
paths, editor IDs or client role claims. See [collaboration concepts](collaboration-foundation.md).

The **2026-10-08 catalogue-content follow-up, source 0.5.4, Unreleased** composes the frozen definition
catalogue with the existing schema 3 content store and Session content executor. Explicit catalogue
handles admit exact full definitions; manual handles remain definition-free, and both reject records
from the other mode. Title/labels/text pins remain instance content, not provider Evidence. The same
old/new byte and final-authorization checks apply without schema changes, rule execution or a new
public edge. Existing HTTP and C2-P–W publication configurations remain manual-only.

The **2026-10-08 Unreleased C2-N follow-up** adds state-shared request/response capacity on the
existing prepared HTTP path: cheap checks → fail-fast slot → body/C2-M/serialization → response-body
completion or drop. The default is four, with a trusted range of 1–16; state clones share it, newly
constructed states do not. This does not add a public edge, global memory/worker quota or recovery path,
and is separate from the original 0.5.0 HTTP evidence and the published 0.5.4 release.

The separate **0.5.5 C2-X** [text Artifact router](text-artifact-http.md) publishes fixed-profile
text and reads exact revisions through current-Session commands. Its 2 MiB wire / 256 KiB text bounds
and shared state-family admission do not change C2-N's 64 KiB budget; independent states add capacity.
It stays unmounted and does not automatically merge with or call the Task router.

The independent **2026-10-11 Unreleased [C2-Y catalogue-content adapter](catalog-task-content-http.md)**,
on source 0.5.4, exposes `/platform/v1/catalog-tasks` only through a separately constructed typed
state. Creation selects an exact server-admitted definition; read/edit/status keep full metadata,
old/new text and current-Session checks in the existing catalogue commands. Bound and unbound
definitions remain catalogue work without assessment. Manual JSON/workspaces remain unchanged.
Security, 64 KiB/two-second request limits and response-body admission are shared, but independent
state-family budgets add. There is no normal-app mounting, discovery, client mode switch, automatic
merge, Session issuer, C2-P–W invocation, OpenAPI/SDK/browser connection or migration.

The independent **2026-10-11 Unreleased [C2-Z catalogue draft publication](session-catalog-task-draft-publication.md)**,
on source 0.5.4 outside the published tag, joins the existing C2-O plan to a separate typed catalogue
attempt. No-I/O preparation freezes an exact complete definition before allocating targets. Each
advance publishes one Artifact or finally creates one catalogue Draft, retaining confirmed prefixes
and terminal uncertainty in memory. Full `Some` definition confirmation remains separate from the
manual `None` matcher. Sharing two pure receipt/manifest checks does not connect the new attempt to
old C2-Q–W observation/checkpoint/journal types, HTTP, providers, recovery, issuer or browser saving.

The independent **2026-10-11 Unreleased [C2-AA catalogue observation](session-catalog-task-draft-observation.md)**,
on source 0.5.4 outside the published tag, adds one current authorized read of a surviving C2-Z
attempt's exact unknown target. It compares full frozen definition and ordered text, preserves typed
corruption and leaves attempt state unchanged. W16 gains implementation references, not new scores,
evidence or topology; matching/invisibility does not establish prior commit/rollback or allow retry.

| Module | Main source | Implemented connection and remaining limit |
|---|---|---|
| Text Artifact HTTP (0.5.5 C2-X) | [separate router](text-artifact-http.md), [`text_artifact`](../../engine/src/platform_http/text_artifact/mod.rs) | Current Session → fixed text publication/exact revision → private JSON. No public mount, issuer, browser/SDK connection, journal workflow, adoption or recovery. |
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
| Content storage (0.5.0 C2-L; 0.5.5 catalogue mode) | [separate schema 3 store](task-content-store.md), [`content.rs`](../../engine/src/services/task_store/content.rs) | Explicit manual/catalogue metadata → atomic Task/manifest/outbox and one revision; trusted store only, no Session/Artifact-byte adapter, public API or schema 2 migration. |
| Authorized content (0.5.0 C2-M; 0.5.5 catalogue mode) | [Session content adapter](session-task-content.md), [`content.rs`](../../engine/src/services/auth_service/durable_source/task_commands/content.rs) | Explicit manual/catalogue workspace + current Session → old/new exact Artifact text → atomic schema 3 edit → final authorization and commit; no live/browser connection. |
| HTTP content boundary (0.5.0 C2-N) | [explicit router](task-content-http.md), [`platform_http`](../../engine/src/platform_http/mod.rs) | Bounded HTTP → C2-M → private versioned response; standalone only, no main-app mounting, Session issuer, public publication or browser wiring. |
| Catalogue content HTTP (0.5.5 C2-Y) | [typed independent adapter](catalog-task-content-http.md), [`catalog_task`](../../engine/src/platform_http/catalog_task/mod.rs) | Exact configured definition + bounded HTTP → catalogue Session content commands → private Task/manifest/text projection. Manual mode stays separate; no assessment, C2-P–W, automatic merge, app/browser/SDK mounting or migration. |
| Catalogue draft publication (0.5.5 C2-Z) | [opaque create-only attempt](session-catalog-task-draft-publication.md), [`catalog_draft_publication`](../../engine/src/services/auth_service/durable_source/catalog_draft_publication/mod.rs) | Exact frozen definition + C2-O plan → explicit Session Artifact steps → final catalogue Draft with full-definition confirmation. In-memory partial outcomes only; no manual conversion, journal/checkpoint, retry/recovery, HTTP/browser or assessment; independent C2-AA observation never adopts progress. |
| Catalogue target observation (0.5.5 C2-AA) | [read-only comparison](session-catalog-task-draft-observation.md), [`observation.rs`](../../engine/src/services/auth_service/durable_source/catalog_draft_publication/observation.rs) | Exact unknown target → one existing current-Session Artifact/catalogue Task reader → body-free current comparison. Full definition and ordered bytes; corruption remains an error, no state change, historical confirmation, rollback inference or retry. |
| Draft publication mapping (0.5.3 C2-O) | [strict importer](task-draft-publication.md), [`task_draft_import`](../../engine/src/services/task_draft_import/mod.rs) | Strict draft JSON → bounded publication values; supplied exact content → manifest. No I/O, server-ID allocation, publication history proof, authorized dispatch or browser save. |
| Session draft publication (0.5.3 C2-P) | [create-only steps](session-task-draft-publication.md), [`draft_publication`](../../engine/src/services/auth_service/durable_source/draft_publication/) | Fixed in-memory attempt → one Session Artifact write per step → C2-M Task create. Confirmed prefix retained; unknown steps stop. No durable recovery, atomic workflow or browser route. |
| Draft target observation (0.5.3 C2-Q) | [read-only comparison](session-task-draft-observation.md), [`observation.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/observation.rs) | Exact unconfirmed target → one authorized read → body-free current comparison; no state change, prior-commit proof, rollback inference or recovery. |
| Draft metadata checkpoint (0.5.3 C2-R) | [bounded checkpoint data](session-task-draft-checkpoint.md), [`checkpoint.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/checkpoint.rs) | Attempt → metadata export/parse. Reported progress is caller data, not save, provenance, authorization or a restored attempt. |
| Checkpoint target inspection (0.5.3 C2-S) | [explicit authorized inspection](session-task-draft-inspection.md), [`inspection.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/inspection.rs) | Parsed data + trusted workspace + current Session → one existing read. Metadata comparison only; no original-attempt restoration, provenance, journal or retry. |
| Immutable draft intent (0.5.4 C2-T) | [intent registration and reading](session-task-draft-intent.md), [`draft_intent_journal`](../../engine/src/services/draft_intent_journal/), [`intent.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/intent.rs) | Real Ready/zero-confirmed attempt → Session-bound immutable record. New-Session read requires the same owner/account generation; no target lookup, restored attempt or write-ahead resource dispatch. |
| Journaled draft dispatch (0.5.4 C2-U) | [live journaled wrapper](session-task-draft-dispatch.md), [`draft_publication/dispatch.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/dispatch.rs), [`draft_intent_journal/dispatch.rs`](../../engine/src/services/draft_intent_journal/dispatch.rs) | Confirm Unknown step → reauthorized resource transaction with marker/business commit → in-memory confirmation. Fresh schema 2 only; original C2-P stays independent, no restart/retry or browser connection. |
| Draft journal inspection (0.5.4 C2-V) | [recorded-prefix inspection](session-task-draft-journal-inspection.md), [`journal_inspection.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_inspection.rs), [`inspection.rs`](../../engine/src/services/draft_intent_journal/inspection.rs) | Current exact owner/account → schema-2 intent/full prefix → metadata counts and optional Unknown. No resource/file read, nonce disclosure, restored attempt or retry. |
| Journal step observation (0.5.4 C2-W) | [one recorded target](session-task-draft-journal-observation.md), [`journal_observation.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_observation.rs) | Capture complete intent/prefix/first pins → actual Artifact or current Task content read → exact recheck and fresh auth/commit. No business write, receipt, recovery or retry; C2-V public API is not called. |

## Cross-module paths and unconnected boundaries

| Path | Current state |
|---|---|
| Text publication → exact reference → separate Task-content HTTP request | W23 ⇢ W13 ⇢ W12 ⇢ W11 ⇢ E02 is a caller-controlled independent in-process fixture sequence. No TextRouter-to-TaskRouter call, automatic merge, atomic batch, public mount or browser save; each command reauthorizes. |
| Exact definition reference → independent catalogue HTTP → Session content | C2-Y supplies a separate typed adapter within the W13 source slice. It uses the existing catalogue transaction checks without changing manual requests, invoking providers or connecting browser saving/public deployment. The dated tensor topology and evidence are not refreshed by this source addition. |
| Frozen catalogue definition + plan → Artifact prefix → catalogue Draft | C2-Z extends W15's source implementation with an independent typed attempt. Each step reauthorizes separately; no batch transaction, durable receipt or old manual/journal type conversion. This source sequence does not refresh tensor scores, evidence or topology. |
| Catalogue unknown target → current reader → comparison metadata | C2-AA extends W16 implementation references without changing tensor topology/evidence. Original-token admission precedes current reader authority; a comparison never becomes confirmation, rollback proof, adoption or retry. |
| Browser → legacy Session → teaching/script/event operation | Connected; authorization and ownership are enforced by the Engine, not sidebar visibility. |
| Source → local Runner → observation → teaching pack → attempt → teacher feedback | Connected teaching path; an assessment result is not a generic durable Review. |
| Browser remote check → owner-bound job → signed Agent → compile report | Connected optional compiler path; no local-execution fallback is granted by Agent failure. |
| Local TaskDraft → selected payload → controlled editor → local export | Connected entirely in the browser; login/unread reads are shell behavior, not payload saving. |
| Durable Session → own Artifact revision → own Task exact inputs → Draft replacement | Implemented preparatory backend path; public API and browser adapter are absent. |
| Durable Session → exact owned Artifact target/evidence → human Review history | Implemented preparatory backend path; cross-user review authority and acceptance remain absent. |
| Legacy accounts/scripts/attempts → durable identity/Artifacts/Tasks/Reviews | Planned migration, writer fencing and cutover; no implicit dual write or adoption. |
| Browser task save → Artifact publication → Task create/replace → conflict recovery | W10–W14 provide content contracts, an unmounted HTTP adapter and pure mapping; W15 composes internal create-only steps and W16 observes current targets without recovery. W23 separately supplies unmounted text HTTP publication. Explicit login/installation, public dispatch, durable recovery and browser save/conflict handling remain missing. |
| Supplied Artifact content → draft plan binding → Task content manifest | W02 ⇢ W14 ⇢ W10 checks supplied bytes and metadata only; no storage call, current authorization or proof of prior publication. Local U03 → W14 remains unconnected. |
| Draft plan → explicit Session publication steps → Task create | W14 ⇢ W15 ⇢ W12 uses I04 and W05 independently for current authorization/publication. Separate transactions, in-memory outcome tracking; no browser bridge or automatic recovery. |
| Unconfirmed attempt → current target observation | W15 ⇢ W16 uses I04 and one W05 or W12 read. Separate observation times, no returned body or state restoration; this read branch adds no representative write chain. |
| Surviving attempt → metadata checkpoint | W15 ⇢ W17 exports reported progress without text or credentials. Its codec invokes no reader or restored attempt; C2-S inspection is a separate operation below. |
| Ready attempt → immutable intent record → authorized record read | Separate internal C2-T path binds current owner/account generation. Registration does not require or cause C2-P dispatch; reading neither observes targets nor reconstructs the attempt. |
| Consumed live attempt → Unknown journal step → resource and marker commit | C2-U explicitly couples one resource to its step under current bound identity. Each successful step can continue; uncertainty or loss of the wrapper supplies no retry or recovery path. |
| Recorded journal steps → explicit current-authority inspection | W20 ⇢ W21 reads separate intent/step metadata through W19 and I04; this data/read edge does not invoke inspection automatically, query resources or create a recovery chain. |
| Recorded ordinal → same-transaction resource observation | W22 shares private journal capture with W21 and depends on W19/I04 plus one W05/W12 borrowed reader. It does not call the W21 public method, transfer authority or add a representative recovery chain. |
| Checkpoint data → explicit current-authority inspection | W17 ⇢ W18 uses I04 and one W05 or W12 read. No W17 → W16 edge, no historical caller/commit proof, no state adoption or new representative recovery chain. |
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
