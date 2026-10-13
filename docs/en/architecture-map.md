# Current architecture and change boundaries

Included in source **0.5.9**, API **0.3.0**, reviewed **2026-10-13**. This organization originally
followed the 0.5.8 tag; release inclusion does not change storage schemas or service mounting.

Cyanrex has one connected teaching application, a separately composed collaboration backend and
a local Task payload editor. Generic contracts no longer require eBPF or executable code, but the
browser-to-platform save and general Task-to-Run connections are not implemented. Module groups
describe ownership; the live Engine still combines control with privileged Linux execution.

Read this map for current responsibilities, [detailed architecture](architecture.md) for decisions,
[platform network](platform-network.md) for feature edges and [capability maturity](capability-maturity.md)
for dated scores. [Project status](project-status.md) and [testing guide](testing-guide.md) retain
their own progress and evidence scopes; no aggregate completion score is implied here.

## Runtime wiring

The [local text-file importer](local-text-file-import.md), included in 0.5.9, prepares reviewed batches
at the UI boundary, retaining the Task contract's editor independence and adding no server edge.

The only normal Engine composition starts at `engine/src/main.rs`. `engine/src/state.rs` constructs
AppState; `engine/src/application.rs` builds its routes. Public/private/teacher guards remain in
`engine/src/routes/`. Exporting a preparation module does not add it to this sequence.

| Path | Current connection | Separate boundary |
|---|---|---|
| Teaching application | Browser → existing AuthService/route guards → teaching, events, deployment and Runner services | No generic Task or Artifact is created by a lab run |
| Explicit collaboration backend | Trusted host → DurableAuthSource Session commands → private resource stores | Not constructed in AppState; platform HTTP routers are unmounted |
| Local payload editor | `/tasks/new`, compatible `/editor` → TaskDraftWorkspace → controlled text editor → local JSON | Local IDs and language hints confer no server reference or execution authority |
| Explicit file delivery | Current exact file grant → Source manifest/chunk checks → supplied router → controlled client → selected saving | Real Source/HTTPS tests exist; normal-app/UI/SDK mounting is still absent |

Teacher authority manages the live teaching deployment, including single-user use. It does not become
generic access to another person's private Tasks, Artifacts, Reviews or recipient-granted bytes.
Live AuthService and `engine/src/services/auth_service/durable_source/` are distinct composition
paths, not two interchangeable login implementations.

## Ownership and dependency direction

| Responsibility | Source entry | Dependency rule |
|---|---|---|
| Shared contracts | `engine/src/models/collaboration/` | Exact identity/reference/Task/Artifact/Review/file values; decoding validates structure, not authority |
| Definitions and pure content planning | `engine/src/services/task_catalog/definitions.rs`, `engine/src/services/task_content.rs`, `engine/src/services/task_draft_import/` | TaskDefinitionCatalog is frozen metadata; no evaluator, database, discovery or execution is required |
| Typed domain evaluation | `engine/src/services/task_catalog.rs`, `engine/src/domain_packs/ebpf_teaching/` | TaskCatalog<P> owns provider-bound evidence and assessment; the teaching pack stays outside neutral contracts |
| Current-Session commands | `engine/src/services/auth_service/durable_source/`, `engine/src/services/collaboration_identity_store/` | Derive exact current actor, audited membership and original pins; resource checks and final authority stay in the source-owned transaction |
| Resource persistence | `engine/src/services/task_store/`, `engine/src/services/artifact_store/`, `engine/src/services/review_store/` | Trusted direct stores do not authenticate supplied owners; Session adapters borrow their transaction mechanics |
| HTTP projection | `engine/src/platform_http/` | Validate requests and project existing commands; do not duplicate permission logic, install schemas or issue Sessions |
| Local host interaction | `frontend/src/features/tasks/`, `frontend/src/features/editor/`, `sdk-js/src/agents/` | The draft owns payload; the editor owns display/tools; SDK tools retain current Engine authority and host approval |

Neutral contracts and frozen definition metadata do not depend on the teaching provider. Domain
evidence is not interchangeable with ordinary Task payload: code, notes and configuration may all
be content without requiring assessment or a Run. Review records, rule outcomes and Task status
changes are distinct operations, not automatic transitions.

`prepared_resource_sql` shares private SQL setup and table checks, not policy decisions.
`legacy_workspace` is an offline permission preview, not live authentication or migration.
The broad durable_source directory currently contains three responsibilities: account/Session source,
transaction authorization and explicit resource commands. Their co-location does not authorize moving
command transactions into a generic permission precheck.

## Service navigation

`engine/src/services/mod.rs` groups all **44** existing declarations by responsibility while retaining
their names, visibility and Unix gates. It adds no module namespace, public API or runtime service.

| Index group | Main responsibility |
|---|---|
| Live instance services | Existing login, classroom/configuration and host management |
| Teaching and local execution | Teaching compatibility, learning/scripts, compilation and kernel runtime |
| Signed Runner Agent services | Authenticated compute protocol, inventory, execution and jobs; not AI identity |
| Event telemetry | Live EventBus and its private persistence/filter/policy helpers |
| Prepared identity and SQL support | Explicit registry, offline legacy preview and private storage mechanics |
| Prepared Task and content | Pure catalogue/content planning plus explicit Task/Artifact/Review/journal stores |
| Prepared file distribution | Stream/descriptor verification, upload/publication/grant ledgers, receiver and client |

`frontend/src/transport/privateRequest.ts` owns browser credentialed waiting and cancellation.
Feature modules retain response decoding and exact acknowledgements; transport is not a server
transaction. `sdk-js/src/generated/` follows the live OpenAPI surface, not every Rust export.
`sdk-js/src/agents/bridge.ts` adapts selected existing tools; provider transport, secrets and per-write
approval stay with the host. AI Agents and signed Runner Agents are different capabilities.

## File authority and resource ownership

File delivery has distinct boundaries rather than one general transfer permission:

1. Upload intent, private staged bytes, journal confirmation and Published metadata are separate
   states. A matching file or an Unknown result cannot be adopted as a successful receipt.
2. Publisher permission allows current publishing/sharing; an independent exact recipient grant
   controls manifest/chunk access. Teacher status or publisher revocation does not replace it.
3. Source metadata observation is not a reusable byte permit. Each chunk independently rechecks
   current authority around bounded private verification; no SQL lock spans the file work.
4. HTTP adapters project the Source result. `engine/src/platform_http/errors.rs` owns one private
   metadata-error mapping reused by manifest and chunk projection, retaining private 404 and
   dispatched/unconfirmed outcomes. Chunk-specific input/capacity errors remain separate.
5. Client transport, framed receiver, staging, assembly and selected saving retain their own proofs.
   `engine/src/services/file_chunk_client/` keeps its compatible name but already contains the whole
   controlled delivery chain; it is not limited to the single-chunk entry.

| Budget | Actual owner and release boundary |
|---|---|
| Metadata HTTP | Actual response body completion/drop, not just handler return |
| Binary HTTP and Source bytes | When projected over HTTP, the last original owner-retaining Bytes reference, including clones/slices |
| Client request/buffer/flow | Matching transport or actual work; already-dispatched file jobs retain their flow lease after waiter cancellation |
| Staging worker | Actual queued/running job, not observer lifetime |

Direct Source returns retain their own byte lease in SessionFileChunkRead. Metadata waiting with
no dispatched file job can release its flow lease on cancellation. These budgets are not interchangeable
or global memory/disk quotas. Combining them can cause early
release or self-blocking at capacity one. Cancelled stops the selected wait/continuation, not already
dispatched OS work. Progress closure and partial percentages do not prove a saved file or rollback.
See [real Source HTTPS](large-file-source-https.md) for combined execution evidence.

## Change boundaries

Small cleanup may share pure helpers, clarify ownership and remove exact projection duplication.
It must preserve state-family budgets, full-version matching, error priority, source transaction pins,
uncertain outcomes and old API names. This pass shares only the file metadata-error projection.

Further candidates, not implemented by this pass, are a private exact-selector parser shared by
manifest/chunk routes, pure target allocation shared by manual/catalogue draft preparation, and a
typed conversion between legacy learning results and teaching evidence. Their distinct admission,
fingerprint, assessment and transaction boundaries must remain separate.

New runtime composition requires a separately reviewed proposal: secure durable Session issuance,
an unprivileged host boundary, explicit storage provisioning, then public/UI/SDK contract and real-LAN
acceptance. A route export or passing loopback fixture cannot substitute for those steps. Recovery,
automatic retry/cleanup, desktop peer transfer and generic scheduling are separate changes.

## Documentation ownership

Current responsibility lives in this map; detailed architecture preserves earlier decisions.
The platform network owns feature edges, the tensor owns dated scores, and release reports own
release acceptance. Execution tails are historical evidence, not text to rewrite during reorganization.
Sources are `docs/`; `frontend/public/course/` is synchronized, never separately authored.
Boundary references: [Task/domain separation](task-domain-boundary.md), [AI host adapters](ai-agent-integration.md),
[security](security.md), [file distribution](large-file-distribution.md).

## Validation on 2026 10 13

- Before organization, the five new structural guards had one pass and four failures for the
  missing grouping/navigation and incomplete adapter index. No production bug Red is claimed.
  The final focused architecture/adjacent suite passed **27** checks.
- All **25 platform HTTP unit tests** passed, including three new projection tests. Each metadata
  adapter checks **51** explicitly expected error variants; chunk-specific input/capacity/unavailable
  checks remain independent. Status, exact sanitized JSON and unconfirmed outcome are preserved.
- Fresh native regression passed **9 unique cases**: three existing Source, three combined real
  Source/HTTPS, and three existing controlled mock-TLS cases. These groups are not double-counted.
- Full backend/common gate passed **1,100 Rust tests**, **861 explicitly ignored**, **98 result
  groups** and **361 common checks**, with zero failures and zero Rust warnings.
- Hash checks preserved **479 unrelated engine source files**, **1,168 protected files**,
  **106 historical status blocks**, **58 old execution tails** and published release reports.
  Version/API, OpenAPI, dependencies, tensor scores and all pre-existing Source/HTTPS edits stayed intact.
- The owned socket-only temporary PostgreSQL was stopped and its exact data/socket directories
  removed (**40,228 KiB**); logs remain at `/tmp/cyanrex-architecture-WRV3tmc3`.

These are cleanup/regression results, not new frontend/SDK builds, deployed-LAN/kernel acceptance,
security audit, production mounting or release publication.
