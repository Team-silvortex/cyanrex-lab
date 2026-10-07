# Project Status

Snapshot date: **2026-10-07**. Source version: **0.5.3**, including C2-O–S draft preparation, teacher-managed AI profiles and SDK tool adapters, internal architecture simplification and DOMPurify/Monaco hardening. Earlier prepared-authentication protections remain in place.

Cyanrex is becoming a domain-neutral collaboration platform, with eBPF teaching retained as its first
domain rather than the definition of all work. The transition has produced a durable collaboration
preparation layer and a local task-payload editor. It has **not** yet produced a server-backed generic
task workflow for users or replaced the existing teaching runtime. This page describes the whole project now;
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
| 0.5.3 · C2-O preparation | Strict whole-draft importer, bounded publication plan and pure exact-content-to-manifest binding | No actual publication, authorization receipt, save orchestration, HTTP/body-limit expansion or browser connection |
| 0.5.3 · C2-P preparation | Caller-owned, Session-authorized create-only Artifact/Task steps with fixed identities and in-memory confirmed progress | No whole-draft transaction, durable receipt/recovery, retry, cleanup, HTTP mounting or browser connection |
| 0.5.3 · C2-Q preparation | Explicit current-Session read of an existing attempt's unconfirmed target, with body-free comparison | Observation only: no historical confirmation, rollback proof, state recovery, retry, public route or browser connection |
| 0.5.3 · C2-R preparation | Strict bounded export/parse of allocated targets and reported progress as metadata | Data only: no save, authenticated provenance, durable journal, restored attempt, read-on-import or retry |
| 0.5.3 · C2-S preparation | Explicit one-target checkpoint inspection through a supplied trusted workspace and current Session | Current metadata only: no original-caller/byte provenance, storage-incarnation proof, journal, restored attempt, retry or public/browser connection |
| 0.5.1 · Session source | Login, validation and logout pin source namespace/table identity, including the password-worker gap and post-write checks | Source-only hardening, not a public issuer, uniform redesign of all auth primitives, installation or live cutover |
| 0.5.1 · Password work | Shared prepared registration/login/rotation/bootstrap gate, four dispatched jobs and twenty total admissions retained for the job lifetime | Job-count limits only; not single-job PHC cost, legacy runtime or distributed limits, public Session issuance or live cutover |
| 0.5.1 · Password profile | Pre-admission PHC checks and explicit existing Argon2id cost/profile for prepared reads and writers | No imported profile adoption, implicit Session revocation, legacy change, process RSS bound or public issuer |
| 0.5.1 · OTP freshness | Login/rotation recheck the existing OTP window behind writer locks and after final SQL before requesting commit | Freshness alone is not consumption, complete clock-rollback defense or a commit-ack freshness guarantee |
| 0.5.1 · OTP consumption | Schema-2 per-account monotonic consumption commits with login Session issuance or password rotation/revocation; pure collision-aware exact-counter policy | Fresh explicit installation only; old schema 1 rejected unchanged. No live-auth switch, public issuer, secret recovery or existing-data migration |
| 0.5.2 · Missing-account password work | Genuine absent accounts run one fixed-profile verification through the same bounded gate after releasing the read transaction, then remain rejected | Removes the missing-account KDF shortcut; not equal latency, complete enumeration defense, a public issuer or a live-auth switch |
| 0.5.2 · Expired Session cleanup | Explicit trusted source command deletes at most 128 validated expired records per confirmed transaction, with source and exact-result checks | No automatic cleanup, CLI/HTTP access, active-device eviction, full-source emptiness proof or global storage quota |
| 0.5.2 · Session expiry reads | Shared prepared Session lookup rejects infinite/unrepresentable expiry before decoding; source reconciliation derives a checked expiry boolean in SQL | Covers source Session expiry, with existing locks and snapshot cutoffs; registry times are covered separately below |
| 0.5.2 · Registry time reads | Guard retirement and identity/policy audit heads, history, replay and write readback; reject invalid non-null retirement without making it active | Scoped reads only; no age rule, driver-wide guarantee, repair, schema change or live-auth cutover |
| Architecture targets | Shared collaboration, generic Run orchestration, AI delegation, reliable business-event delivery, isolated workers and ecosystem integration | A proposed type, diagram or milestone is not an implemented user workflow |

Product version, API compatibility baseline and individual storage schema versions are independent.
The source version is 0.5.3 and the public API compatibility baseline remains 0.3.0. The current Task
store requires schema 2 in a fresh dedicated namespace and rejects schema 1 without migration. Neither
the normal startup path nor this source release upgrades a database.
The content store included in 0.5.0 separately requires schema 3; the schema 2 handle and existing Session
adapters reject it. Both formats require their own explicitly installed fresh namespace.
Prepared authentication now separately requires source schema 2 for atomic OTP consumption. Its
installer and readers reject existing source schema 1, including empty instances, without migration.

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
| Whole-draft publication mapping | 0.5.3 C2-O strictly parses portable draft JSON, prepares text publication values and binds supplied exact content | Pure values only; no dispatch, confirmed publication history, current authorization or persisted draft |
| Review | Immutable revision history; Session-authorized private human opinions on exact Artifact targets/evidence included in 0.4.9 | No cross-user reviewer grants, generic Task approval, automatic rule execution or AI review |
| Teaching and eBPF workbench | Five labs, attempts/progress, teacher feedback, historical resume, Clang assistance, bpftool and supported Aya tracepoint execution | Remains a separate compatible workflow; old attempts have not been migrated into generic Tasks/Runs/Reviews |
| Execution and resources | Local leases/quotas/timeouts; signed remote Agent registration, jobs, probes and optional compile-only diagnostics | Local kernel remains shared; no durable generic Run service, remote eBPF loading or VM lifecycle |
| Events and persistence | Owner-scoped telemetry, bounded history/reconnect recovery and service-specific persistence; preparation stores commit their own outbox records | No generic durable business-event dispatcher, cursor replay or multi-Engine coordination |
| Extensions and integrations | State-only module manifests, structured teacher commands, generated OpenAPI/internal SDK; 0.5.3 AI connection metadata and explicit SDK tool adapters | No provider transport, executable plugin runtime, AI Agent planning/delegation or implemented Viento/Lese/Nuis integration |
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

0.5.3 [C2-O](task-draft-publication.md) supplies the pure import and publication-plan mapping:
blank titles are rejected without inventing text; checked local identities are discarded. Exact
supplied content may be bound to a manifest, but matching bytes do not prove publication or authorize
a later command. The server accepts valid extension language hints without changing the browser's
allowlist. The pure importer itself dispatches no publication; C2-P supplies separate internal
composition, while browser saving remains absent.

0.5.3 [C2-P](session-task-draft-publication.md) separately composes current-Session publication
and C2-M Task creation one write at a time. A fixed caller-owned attempt retains confirmed progress
and stops at an unconfirmed step after dispatch failure or cancellation. It supplies no persistent
receipt, automatic recovery or browser/public connection; each command still reauthorizes independently.

0.5.3 [C2-Q](session-task-draft-observation.md) separately observes a fixed unconfirmed target
with the original Session. Matching data does not confirm the earlier write; invisibility does not
prove rollback. Observation leaves the attempt stopped and provides no new-login or restart recovery.

0.5.3 [C2-R](session-task-draft-checkpoint.md) exports a 64 KiB-bounded metadata checkpoint without
text or authentication material. Parsing validates structure and consistency, not the reported history.
It performs no save or read and cannot restore C2-P or invoke C2-Q; durable outcomes remain separate.

0.5.3 [C2-S](session-task-draft-inspection.md) adds a separate explicit read of one checkpoint
target under supplied current authority, without needing the original attempt. Matching metadata is
not original-byte or historical-caller proof; parsing still does no I/O, and C2-Q's original Session
requirement remains unchanged. No confirmation, journal, state restoration or retry follows.

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

### 0.5.3 source candidate checks · 2026-10-07

The fresh local full gate at version 0.5.3 passed **589 default Rust, 162 common, 206 frontend and
42 SDK tests**. Rust covered 54 test targets with no failures or compiler warnings;
**563 ignored tests are not counted as passes**. Next production build/TypeScript, SDK build/type/package
checks, generated contracts, the frozen 0.3.0 compatibility baseline, version metadata, course mirrors
and tooling checks passed. Frontend and SDK production dependency audits each reported zero findings.
Deployment/database variables were cleared and fixtures used private temporary storage.
This release check did not rerun opt-in PostgreSQL or browser suites, Rust advisory audit, remote CI,
container builds/deployment, or real kernel/SSH/LAN acceptance. Historical evidence below remains unchanged.

The AI, C2-O–S, architecture and DOMPurify implementation records below retain their original
0.5.2 workspace versions, Unreleased status, dates and counts. Those changes are now included in
0.5.3; the records are not a new release gate or deployment acceptance.

### Unreleased AI Agent host adapters and configuration · 2026-10-07

[AI Agent integration](ai-agent-integration.md) adds teacher-only profile metadata, a separately
reviewed Settings panel and explicitly selected SDK tool adapters. It does not call providers, store
API keys, create AI identities/delegation, execute general Tasks or alter the signed Runner protocol.
The 28-tool catalogue excludes auth/settings/kernel mutations; its five write/diagnostic operations
require per-call trusted host approval and retain the current server authorization.

Actual Red → Green covered persistence, missing nullable fields, Unicode/FEFF blank boundaries,
raw URL and empty-port normalization, private-parent permissions, total size and cancelled caller
ordering. Defaults ran **13 new file/router cases and 2 cancellation units**. Frontend **12 new units**
and **15 new actual component-browser fixtures** passed; adjacent Settings 17 and Runner 24 cases
also passed, **56 browser cases** in the final rerun. These fixtures mock Engine responses and reject
external networking; they are not real provider or deployed-service acceptance.

The full isolated gate passed **589 Rust, 162 common, 206 frontend and 41 SDK tests**, including
production Next/TypeScript, SDK runtime/type/package and compatibility checks. Rust covered 54 targets;
**563 ignored cases are not passing-run evidence**. No opt-in PostgreSQL or live kernel/SSH/deployment
acceptance ran here. Inherited deployment/database variables were removed and temporary data was used.
Frontend and SDK production audits each returned zero findings in every severity and in total.

After that gate, an additional SDK late-success-after-abort regression first failed, then passed with
the fix: the final SDK check passed **39 runtime + 3 package cases (42)**, plus generated/type/compatibility
checks. This final SDK-only delta does not claim a second full Rust/frontend build. Tool cancellation
remains cooperative, not a new total deadline, HTTP stream bound or server rollback guarantee.
The tensor adds U08/O07/E04 and e90–e95, but P10 autonomous delegation and P01 generic browser saving
remain planned. Metadata backup requires the Unix service user, private permissions and stable instance
name; host transport, secret allowlists and private-output disclosure remain explicit caller duties.

### Unreleased C2-S current-authority checkpoint inspection · 2026-10-07

The [explicit checkpoint inspector](session-task-draft-inspection.md) separately reads one reported
unconfirmed target using a supplied trusted workspace and current Session. The original attempt need
not survive. It returns body-free metadata agreement, difference or invisibility; reader failures retain
their types. Neither a parsed claim nor a matching read confirms historical publication, restores an
attempt or permits retry. C2-R parsing remains pure and C2-Q's original-attempt/token rules are unchanged.

**Six new default Rust units, twelve exact PostgreSQL cases and three common guards passed**:
21 new checks, with an actual behavior Red before Green. The SQL cases cover parsed records after
attempt drop/source-handle reopening, a fresh same-owner Session without replacing the original C2-Q
fingerprint, foreign-owner filtering, observable metadata versus manifest labels, exact historical
Artifact revisions, binary/control-text rejection, typed digest/blob/namespace faults, missing versus
existing empty Tasks, logout/revocation/expiry including lock waits, cancellation and pending creators
that abort, commit or fail deferred commit. Checkpoint bytes, business records and content files remain
unchanged by inspection. Reopening handles is not a process-crash or durable-recovery test.

The adjacent C2-Q, C2-P, Session Artifact and Session content suites separately passed **67 exact SQL
cases** (12 + 12 + 18 + 25): **79 distinct PostgreSQL cases** including the twelve new ones. A fresh
PostgreSQL 16 fixture used only a private Unix socket, with no TCP listener or deployed database.
No custom fixture namespaces remained after the suites. The server was stopped and its disposable
132 MiB data directory removed; diagnostic logs were retained. This cleanup did not remove project data.

The complete local gate passed **574 default Rust tests, 156 common checks, 194 frontend tests and
16 SDK tests**. Rust covered 53 test targets with no failures or warnings; **563 ignored cases are
not passing-run evidence**. The six new units and three guards are included in those totals; the
opt-in SQL cases above ran separately. Production Next build/TypeScript, SDK build/type/package,
API compatibility against baseline 0.3.0, version, documentation and tooling checks passed. Fresh
frontend and SDK production dependency audits each reported zero findings at every severity and in
total. Frontend direct TypeScript imports still emitted non-failing `MODULE_TYPELESS_PACKAGE_JSON`
diagnostics; no package-module-mode change is included. Browser suites, Rust advisory audit, remote
CI, container deployment and real kernel/LAN acceptance were not rerun.

The current capability inventory has **62 implementation coordinates, 61 capabilities, 89 directed
edges and 18 representative paths**. W18 is **3 / 1 / 3 / 1** with separately recorded database evidence;
W15, W16, W17 and browser-save P01 scores are unchanged. The four added edges connect checkpoint data
to explicit inspection and inspection to existing current-authority readers, not restoration or retry.
Source version remains 0.5.2; this slice is Unreleased and has not been committed, pushed or deployed.

### Unreleased C2-R draft metadata checkpoint · 2026-10-07

The [checkpoint codec](session-task-draft-checkpoint.md) exports all allocated targets, ordered
labels, expected byte lengths and reported progress from a surviving C2-P attempt. The separate
opaque value can encode or parse at most 64 KiB without text, Session credentials, owner or storage
routing. Parsed progress is only caller data: forged `complete` and stale `ready` records can be
structurally valid. No save, storage read, restored attempt or permission to retry follows from parsing.

**Six new internal Rust units, fourteen default contract cases and three common guards passed**:
23 new checks. Coverage includes exact object shapes at every record level, preserved duplicate-field
rejection, integer-token rules, UTF-8/framing/size bounds, RFC UUIDv4 target identities, canonical scope,
independently unique Artifact/revision IDs, state/count/length agreement, empty plans and maximum
metadata. Live-attempt exports cover ready prefixes, unknown Artifact/Task targets and completion
without changing state or exporting receipts. The array-rejection matrix uses actual declared field
order, avoiding a false pass from unrelated field-type errors. An actual behavior Red preceded Green.

The 21 existing C2-P/Q default units also passed the focused run. Fixtures use pure metadata and an
explicitly closed synthetic pool; no database server or opt-in PostgreSQL suite ran in this slice.
This does not inherit the earlier C2-P/Q SQL evidence or test crash durability, authorization after
restart, original write provenance or recovery.

The complete local gate passed **568 default Rust tests, 153 common checks, 194 frontend tests and
16 SDK tests**. Rust covered 52 test targets with no failures or warnings; **551 ignored cases are not
passing-run evidence**. The 20 new Rust cases and three guards are included in those totals. Production
Next build/TypeScript, SDK build/type/package, API compatibility against baseline 0.3.0, version,
documentation and tooling checks passed. Fresh frontend and SDK production dependency audits each
reported zero findings at every severity and in total. Direct TypeScript imports in frontend tests
emitted Node `MODULE_TYPELESS_PACKAGE_JSON` diagnostics without failures; no package-module-mode
change is included. Browser suites, Rust advisory audit, remote CI, container deployment and real
kernel/LAN acceptance were not rerun.

The capability inventory has 61 implementation coordinates, 60 capabilities, 85 directed edges and
18 representative paths. W17 is reviewed as **3 / 1 / 2 / 1**, with unit-record evidence only; W15,
W16 and browser-save P01 scores are unchanged. The only new edge is W15 → W17 metadata export,
not checkpoint → observation/storage/recovery. A durable-before-dispatch intent journal and authorized
read-only restart inspection remain separately scoped work. Source version remains 0.5.2; these
changes are Unreleased, not committed, tagged or pushed.

### Unreleased C2-Q draft target observation · 2026-10-07

The [read-only observation](session-task-draft-observation.md) inspects only a surviving C2-P
attempt's exact unconfirmed target with the original Session. One existing authorized reader validates
current storage and authority before a body-free match/difference/visibility report. Observing any
result, read error or cancellation leaves the attempt stopped, with its confirmation prefix unchanged.
Matching is not historical commit provenance; invisibility does not establish rollback or safe retry.

**Seven new default Rust units, twelve exact PostgreSQL cases and three common guards passed**:
22 new checks across distinct layers. SQL covers existing matching targets without adoption, legitimate
later Task edits, current-target-only reads, deferred failure with a remaining blob, missing-Task versus
present-empty-Task namespace checks, damaged/missing bytes, original Session loss/expiry/revocation,
exact read lock waits, competing creators and cancelled reads. Complete business-table/file snapshots
and source-pool recovery checks accompany the relevant read-only scenarios. No warning was emitted.

The broader SQL run passed **67 unique PostgreSQL cases**: the 12 new observation cases plus 12 C2-P,
18 Session-Artifact and 25 Session-content regressions. The 55 adjacent cases are not new observation
tests. All ran on a fresh PostgreSQL 16 fixture with disposable test accounts, source schema 2 and a
private Unix socket, with no TCP listener or ambient/deployed database. Custom namespaces remaining
after verification were zero. The server was stopped and its owned 132 MiB data directory removed;
verification logs were retained separately.

The complete local gate passed **548 default Rust tests, 150 common checks, 194 frontend tests and
16 SDK tests**. Rust covered 51 test targets with no failures or warnings; its **551 ignored cases
are not passing-run evidence**. Production Next build/TypeScript, SDK build/type/package, API
compatibility against baseline 0.3.0, version, documentation and tooling checks passed. Fresh frontend
and SDK production dependency audits each reported zero findings at every severity and in total.
The seven new default units and three common guards are included in these gate totals; the 67 exact
PostgreSQL cases ran separately. This slice did not rerun the unmounted HTTP or browser suites, run a
Rust advisory audit or remote CI, build/deploy containers, or establish real kernel/LAN acceptance.

The capability inventory now has 60 implementation coordinates, 59 capabilities, 84 directed edges
and 18 representative paths. W16 is reviewed as **3 / 1 / 3 / 1** with this scoped SQL evidence;
W15 and browser-save P01 scores are unchanged. No journal, restart/new-login recovery, automatic
retry, deletion, public route, browser connection, schema migration or live-auth switch is added.
Source version remains 0.5.2; these changes are Unreleased, not committed, tagged or pushed.

### Unreleased C2-P Session draft publication · 2026-10-07

The [create-only stepper](session-task-draft-publication.md) now consumes C2-O plans and dispatches
one existing Session-authorized Artifact write per advance, followed by a separate C2-M Task create.
Preparation fixes all random server IDs, the source/workspace and a private Session fingerprint
without I/O. Wrong-token admission leaves Ready unchanged; a dispatched error or dropped future
preserves confirmed progress and the exact unconfirmed step, then refuses continuation.

**14 new default Rust units, 12 exact isolated PostgreSQL cases and three common guards passed**:
29 new checks across distinct layers. The SQL run used a fresh PostgreSQL 16 database and private Unix
socket, not a deployed or ambient database. It covers empty/multiple items, Unicode and identical
bytes with distinct IDs, unpolled work, token substitution, partial Artifact failure, final commit
failure, damaged confirmed blobs, conflicting allocated targets, cancellation, revocation and fresh
Session expiry after lock waiting. The focused Rust and SQL runs emitted no warnings. SQL cases are
selected explicitly by `scripts/test-session-task-draft-publication.sh`, not by default Cargo execution.

The broader run passed **66 unique PostgreSQL cases**: the 12 new workflow cases plus 18 existing
Session-Artifact, 25 Session-content and 11 unmounted HTTP cases. The 54 adjacent cases rerun their
own authorization, persistence and routing boundaries; they are not additional new workflow tests.
All used disposable test accounts and fresh source schema 2 on the private PostgreSQL 16 fixture, with no
TCP listener. After verification, no custom fixture namespaces remained; the server was stopped and
its owned 101 MiB data directory removed. Verification logs were retained separately.

The complete local gate passed **541 default Rust tests, 147 common checks, 194 frontend tests and
16 SDK tests**. The Rust run covered 50 test targets with no failures or warnings; its **539 ignored
cases are not passing-run evidence**. Production Next build/TypeScript, SDK build/type/package, API
compatibility against baseline 0.3.0, version, course-documentation and tooling checks all passed.
Fresh frontend and SDK production dependency audits each reported zero findings in every severity
category and in total. The default Rust/common additions above are included in these gate totals;
the 66 explicit PostgreSQL cases were run separately.

This slice did not run a Rust advisory audit, remote CI, browser suites, container build/deployment
or real kernel/LAN acceptance. The private fixture is not a deployed-source migration or a public
listener, and in-process HTTP tests do not establish a connected browser workflow.

The workflow remains Unix-only internal preparation. Its in-memory progress is neither a durable
receipt nor whole-draft atomicity, outcome recovery, retry or cleanup. Existing commands retain their
current authorization, transaction and deadline boundaries. No public route, browser connection,
Session issuer, schema migration, live-auth switch or deployment authority is added. Source version
remains 0.5.2; source changes are Unreleased, not committed, tagged, pushed or deployed.

### Unreleased C2-O draft publication mapping · 2026-10-07

The [strict draft importer and publication plan](task-draft-publication.md) now validate a complete
bounded text draft before constructing publication values. Blank titles, duplicate/unknown JSON
fields, invalid Unicode and non-integer version/revision tokens are rejected; valid text and labels
are preserved. Local IDs/revisions are checked then discarded. Binding compares supplied exact
Artifact content with the plan and produces the existing manifest; matching values do not prove a
publication history, confirmed commit or authorization. Existing matching revisions can be reused
without republishing bytes for metadata-only edits.

Focused checks passed **45 Rust cases**: 18 new importer/binding cases, one new internal publication-value
unit and 26 existing manifest/supplied-content regressions. The unit checks kind/media, exact bytes
and selection of the Task title without exposing new public getters. The local draft parser suite passed
22 cases, including three new shared-fixture/format-difference cases; three new common source guards
also passed. The 19 Rust, three frontend and three common additions are **25 new checks**, not counts
to add again to the complete gate below.

The complete local gate passed **527 default Rust tests, 144 common checks, 194 frontend tests and
16 SDK tests**. The 527 ignored Rust cases are not passing-run evidence. Production Next build and
TypeScript, SDK build/type/package, API compatibility against baseline 0.3.0, version, documentation
and tooling checks all passed. The Rust run emitted no warnings; both frontend and SDK production
dependency audits reported zero findings in every severity category and in total.

This slice did not rerun opt-in PostgreSQL or browser suites, build or deploy container images, run
remote CI or a Rust advisory audit, or establish kernel/LAN acceptance. No listening service was
enabled. It adds no schema, live-authentication, public API or SDK change, actual publication, save
orchestration or browser connection. Source version remains 0.5.2; the changes are not committed,
tagged or pushed. The earlier dated records below retain their original scope and counts.

### Unreleased architecture simplification · 2026-10-07

The first simplification round shares mechanisms, not domain or authorization boundaries. Settings,
Events and Runtime now use one private request lifecycle; Runner administration and Metrics depend
directly on neutral transport rather than the Settings feature. Feature decoders and acknowledgements
remain separate. The new Event/Runtime regressions first failed because abort-ignoring fetch/body work
kept its caller waiting; cancellation/deadline races now end that wait without retries or server rollback.

Task, Artifact and Review share private transaction setup, namespace and ordinary-table checks.
Their table locks/queries, schema/scope and error checks, commit ownership and Session adapters remain
unchanged. Teaching runtime, backend preparation and local editing are still separate; no public API,
storage migration, AppState composition or Task-content save connection was added. See [architecture](architecture.md).

The complete local gate passed **508 default Rust tests, 141 common checks, 191 frontend tests and
16 SDK tests**, including production builds/type/package checks. The 527 ignored Rust cases are not
passing-run evidence. The two new default Rust guards, two common guards and eighteen frontend cases
are included in those totals. Frontend and SDK production audits both reported zero findings.

Separately, **80 unique PostgreSQL cases passed** on a fresh PostgreSQL 16/private-socket fixture:
70 direct-store cases, eight selected Session boundaries and two new SQL-helper cases. These cover
schema 2/3, table-shape rejection/decoding order, local transaction settings, namespace switches,
post-write authorization checks, expiry and cancellation/pool restoration. The two new helper cases
are selected by an exact runner and wired into CI; remote CI itself was not run.
All **118 browser cases passed**: 110 isolated component cases plus eight production Next-page cases
with mocked Engine responses. This is not deployed Engine authorization or real kernel/LAN acceptance.
The temporary services were stopped. Source version remains 0.5.2; these changes are not committed,
tagged, pushed or deployed, and the earlier dated results below remain unchanged.

### Unreleased DOMPurify follow-up · 2026-10-07

Source version remains 0.5.2; this working-tree fix is separate from the published 0.5.2 tag. DOMPurify
is now locked to 3.4.16 with an explicit offline security floor. It is the only changed dependency
resolution; fresh frontend production audit reports zero findings in every severity category. No audit
threshold or exception was relaxed, and no API, database schema, Engine or Monaco package version changed.

Monaco's prebuilt AMD editor contained its own DOMPurify 3.2.7, unaffected by the npm override. Tests
against that actual generated instance reproduced retained event handlers on removed fixture subtrees
and a returned unsafe rawtext root. The fix checks the original Monaco chunk version/hash and syntax,
replaces the vendor block with the official patched ESM implementation in an isolated expression,
and rewrites all 27 AMD references to a new chunk URL, removing the old file. Docker excludes host
Monaco caches and takes verified generated assets from the dependency-install stage; its source guard
does not constitute a built-image acceptance result.

All **10 explicit local browser cases passed** after regeneration: nine inspect the actual Monaco
sanitizer instance, hook/HTML/root behavior and real hover rendering; the tenth loads the unmodified
AMD loader, editor entry and language registration graph through intercepted local file responses.
The original nine had six failing cases before the asset fix. Fixture handlers only increment a benign
test counter; no Engine, external service or real user content is used. This is a library/consumer
regression result, not proof of an exploitable application path or a deployed sanitizer version.

The complete local gate passed **506 default Rust tests, 139 common checks, 173 frontend tests and
16 SDK tests**, including production frontend/SDK build, type and package checks. The 525 ignored
Rust cases are not passing-run evidence. Eight asset-tool units are included in the frontend count;
the new Docker-input guard is included in the common count. Frontend and SDK production audits both
reported zero findings. A subsequent clean install reproduced the patched resources through private
staging, and all ten browser cases passed again. No Docker image, opt-in SQL, remote CI or live
deployment acceptance was run in this follow-up.

The preceding published release's dependency results and counts below remain historical. This fix
is not committed, tagged, pushed or deployed; running frontends still require rebuilding and redeploying.

### 0.5.2 candidate checks · 2026-10-07 · dependency fixes verified

The complete local gate passed **506 default Rust tests, 138 common checks, 165 frontend tests and
16 SDK tests**, including the production frontend build/TypeScript and SDK build/type/package checks.
The 525 ignored opt-in Rust cases are not passing-run evidence. The frontend production dependency
audit at that stage had no moderate/high/critical findings and retained one low DOMPurify finding;
the SDK audit had no findings. Version metadata, release notes and current bilingual documentation were synchronized.

The first full-gate attempt stopped after a valid production audit reported two high findings in
sharp 0.35.4 and source-map-js 1.2.1; that attempt was not a passing full gate or a registry timeout.
The authorized follow-up installs [sharp 0.35.5](https://github.com/advisories/GHSA-wq5f-xc86-pv6w) and
[source-map-js 1.2.2](https://github.com/advisories/GHSA-68fv-2mgg-jv7q), with matching cross-platform
sharp/libvips packages. Unrelated dependency resolutions and Node requirements remain unchanged.
Three new offline override/lock regression checks are included in the 138 common checks, not added
again. Six separate bounded runtime probes passed: installed sharp/native-library versions, small
SVG-to-PNG/WebP resizing and malformed image rejection, ordinary source-map/SourceNode round-trip,
small indexed-map offsets, invalid/out-of-range offset rejection, and mappings beyond short code.
No amplifying payload was flattened or large generated image used; only this host's native runtime
was exercised, not every platform in the lockfile.

Earlier SQL evidence was not rerun for this version/dependency step. Commit/tag/publication checks
remain separate from the local gate. No browser, Rust advisory, real kernel/LAN, distribution artifact,
deployment/recovery or remote CI acceptance is established by these checks. Updating the source or
lockfile does not patch an already running deployment, nor establish complete dependency safety.

The four 2026-10-07 implementation records below retain their 0.5.1 working-tree version and
then-unreleased status. Their changes are included in 0.5.2; the recorded counts, dates and limits
are not new release-gate results or deployment acceptance.

### Registry retirement and audit reads · 2026-10-07

Before the fix, retirement/lifecycle and policy read paths reproduced the locked SQLx chrono decoder
panic. Fourteen SQL projections now check `retired_at` and identity/policy `recorded_at` before binary
decoding, including schema activation's `LIMIT 0` probes. An independent raw validity flag distinguishes
genuine NULL retirement from invalid non-null retirement: corrupt data cannot look like an active binding
that matches its original unretired audit head. Mandatory audit times fail with `InvalidRecord`.

**Ten new exact PostgreSQL regression groups passed** with the fixed focused runner: identity storage 2,
lifecycle audit 2, policy audit 3 and reconciliation/Session command composition 3. Inputs include both
infinities and finite year 262143, with genuine NULL, PostgreSQL 4713 BC and chrono maximum-microsecond
controls. A healthy current head plus a corrupt older receipt distinguishes history/replay rejection
from an unrelated actor failure. Hit-counted retirement/audit INSERT faults and full SQL row snapshots
prove binding/principal or policy/grant/receipt rollback. Fixtures are drained and cleaned before panic
assertions; read-only reconciliation retains `InvalidIdentity`, `IdentityHistoryMismatch` and
`PolicyHistoryMismatch`, without writes or partial success.

The broader serial run passed **132 distinct isolated PostgreSQL cases**: identity storage 17,
lifecycle audit 18, policy audit 19, access policy 15, bootstrap 14, current-Session collaboration 16,
account deletion 16 and reconciliation 17. The focused ten and the preceding Session-expiry pair in
reconciliation are included in this total, not added again. All used a fresh PostgreSQL 16 instance,
private Unix socket and synthetic records, never a deployed database.

The complete backend gate passed **506 default Rust tests and 135 common checks**; its 525 ignored
opt-in cases are not passing-run evidence. The 132 SQL cases above were run separately. Formatting,
bilingual mirrors and the updated tensor passed. Frontend/browser/SDK builds, dependency audits,
remote CI, real kernel/LAN, deployed-data and recovery acceptance were not rerun in this slice.

CI now enumerates identity storage 17, lifecycle audit 18, policy audit 19 and reconciliation 17 cases;
the focused ten are a subset, not additional coverage counted twice. Source stays 0.5.1, schemas and
dependencies are unchanged. Existing sequence continuity, authorization, locks, limits and snapshots
remain intact. No new audit age/order policy, data repair, live-auth switch or safety claim for every
database timestamp is made. See the [registry time contract](collaboration-reconciliation.md).

### Session expiry reads · 2026-10-07

Both ordinary Session validation and source reconciliation reproduced the locked SQLx chrono decoder
panic before the fix. Shared Session lookup now projects finite, chrono-representable expiry or NULL,
then returns `InvalidRecord` for NULL. Reconciliation reads a checked SQL expiry boolean and returns
`InvalidSource` for invalid time; corrupt rows are never filtered out or counted as normally expired.
Existing source pins, row locks, fresh Session time and the read-only snapshot cutoff remain in place.

The new regression groups cover validation, login INSERT readback with a hit-counted trigger and full
account/OTP/Session rollback, rejected password-change authorization, and read-only reconciliation.
They include positive/negative infinity, finite year 262143, normal valid/expired Sessions, a PostgreSQL
4713 BC date and chrono's maximum microsecond. Fixture state is observed and cleaned before
panic assertions. The source stays 0.5.1/schema 2; there is no dependency upgrade or live-auth cutover.

**105 distinct isolated PostgreSQL cases passed**: source Session boundary 17, source 14, password
rotation 17, account deletion 16, reconciliation 14, missing-account work 7, atomic OTP consumption 11
and expired-Session cleanup 9. An initial parallel rotation batch missed its short 1.5-second expiry
fault barrier; all 17 passed with serial execution, matching CI's sequential selection. The local CLI
fixture initially rejected a group-writable temporary parent; correcting only that disposable directory
allowed the restricted-reader case to pass without weakening private-file checks. All SQL used a new
PostgreSQL 16 instance, a private Unix socket and synthetic data.

The full backend gate passed **506 default Rust tests and 134 common checks**, with 515 opt-in cases
ignored in that gate; the 105 database cases above were run separately. Formatting, bilingual mirrors
and the updated capability inventory passed. Frontend/browser, dependency audits, remote CI, deployed
database and real kernel/LAN acceptance were not rerun in this source-expiry slice.

At this stage, registry `retired_at` and identity/policy audit `recorded_at` decoding remained the next
adjacent slice; the later registry-time record above covers it separately. These source-expiry checks
do not establish safety of every timestamp in a Session command or the complete reconciliation pipeline.
Earlier evidence below retains its original test counts.

### Explicit expired Session cleanup · 2026-10-07

**Nine exact new PostgreSQL cases** passed after two failing stages: a no-op maintenance seam could
not remove the expected 128-row batch, and an infinite timestamp caused the locked SQLx chrono decoder
to panic rather than return an error. Selection and deletion readback now guard finite, representable
timestamps in SQL before Rust decoding. Tests include positive/negative infinity and a finite 262143-year
creation time, oversized/non-hex digests, invalid usernames and an orphaned account reference. Invalid
candidates reject the whole batch without repair; the panic regression now cleans up before asserting.

The suite covers ordered 128/2/0 batches, a real Session remaining usable and its consumed OTP state
unchanged, source isolation, independent-pool batches, fresh time after source-lock waits, suppressed
deletion, future-expiry reinsertion, source/selected-row changes, deferred COMMIT failure and observed
rollback after cancellation. Fault counters establish that the targeted writes ran. Some selected-row
mutation faults are rejected by PostgreSQL itself; these are rollback checks, not isolated proof of
every application readback branch or arbitrary trigger side-effect detection.

Adjacent reruns passed source 14, source Session boundary 14, password rotation 17, account deletion 16,
read-only reconciliation 12, missing-account password work 7 and atomic OTP consumption 11 SQL cases:
**100 isolated PostgreSQL cases including the new nine**. Four default closed-pool checks also passed
in those selected integration targets. All database work used a new PostgreSQL 16 instance with a private
Unix socket and synthetic records, not an installed deployment.

The full backend gate also passed: **506 default Rust tests and 134 common checks**, with 510
database-dependent cases ignored in that gate; the 100 SQL cases above were run separately against
the disposable database. Formatting, documentation mirrors and the capability tensor checks passed.

This is an unreleased internal maintenance command on 0.5.1, without automatic invocation, a CLI/HTTP
surface, active-device eviction, migration or a global Session/disk limit. Ordinary reads remain
non-mutating. Browser/frontend/SDK builds, dependency audits, remote CI, real kernel/LAN, operator
deployment and restore acceptance were not rerun. The earlier records below retain their own scopes.

### Missing account password work · 2026-10-07

The held-executor PostgreSQL regression first failed because a missing account returned without a
password job. After the fix, **seven exact new SQL cases** passed: missing/known-wrong worker admission,
released database connection and source lock, queued-job cancellation, matching public dummy denial,
same-name account creation during the worker gap, unchanged state, global/username limits and distinct
input/profile/source/storage errors. The race fixture makes both dummy and new-account passwords match,
and confirms that only a new login request succeeds. Semaphore observations establish admission;
elapsed-response equality is not measured or claimed.

Adjacent reruns passed **87 PostgreSQL cases**: source 14, password rotation 17, bootstrap 14, password
profile 6, source Session boundary 14, OTP schema 4, OTP freshness 7 and atomic consumption 11.
Together with the new seven, this is **94 isolated SQL cases**, using fresh PostgreSQL 16 over a private
Unix socket. Three default closed-pool checks also passed in the selected integration targets.
All **14 password-profile unit tests** passed, including derivation and true/false verification of the
fixed public synthetic PHC. The exact CI runner and its enumeration guard are included in source.

The backend quality gate passed **505 default Rust tests and 131 common checks**, with no Rust
compiler warnings; **501 opt-in cases** were ignored by that default run. Formatting, file lengths,
version/course/tensor consistency, public API/SDK contracts and tool fixtures passed. The 94 SQL
cases above ran separately and do not imply execution of every ignored target.

The change is unreleased on 0.5.1. It neither mounts HTTP nor changes dependencies, live AuthService,
database format or maturity scores. Complete timing/ingress/Session policy and recovery remain open.
Browser/frontend/SDK builds, dependency audits, remote CI, live kernel/LAN and deployment acceptance
were not rerun for this slice; the release records below retain their original scope.

### 0.5.1 source release checks

On 2026-10-04, the complete local quality gate passed **504 default Rust tests, 129 common checks,
165 frontend tests and 16 SDK tests**. The default Rust run ignored **494** opt-in cases. Production
build, TypeScript, package checks, formatting, version/course/tensor consistency and public API/SDK
compatibility passed. Installed dependencies were reused with `--no-npm-install`; only root package
versions changed in lockfiles, not dependency resolutions. The public API baseline remains 0.3.0.

The first full attempt stopped at npm audit after registry timeouts, not a passing security result.
Fresh frontend/SDK audits then succeeded and the entire gate passed again. The frontend reports one
low-severity DOMPurify advisory, no moderate/high/critical findings; the SDK reports none. Rust had no
compiler warnings; existing Node module-type notices remain non-blocking. The release archives the
Session-source, password-work/profile and OTP changes without enabling public issuance or live cutover.

The **304 isolated PostgreSQL cases** in the immediately preceding atomic-OTP record were not rerun
for this version/documentation step. Browser suites, Rust advisories, real kernel/LAN, distribution
artifacts, deployed migration/restore and remote CI were not revalidated by this local release gate.
Auth source schema 2 is fresh-install only; source publication does not upgrade an existing instance.
Release links use the recorded 0.5.0 commit as the comparison base because its tag remains local;
only the new v0.5.1 tag is intended for publication with main.

The following implementation records preserve their original working-tree versions and then-unreleased
status. Session-source, password-work/profile and OTP changes are now included in 0.5.1; inclusion
does not rerun their historical tests, migrate storage or establish deployment acceptance.

### Unreleased atomic OTP consumption and source schema 2

On 2026-10-04, a real PostgreSQL regression against independent source instances first produced
**two successful logins for one code**, where exactly one was required. After integration, the same
case passed and **11 exact consumption cases plus four exact schema cases passed**. They cover
reopen/logout persistence, login versus rotation, separate accounts/recreated incarnations, suppressed
or tampered writes, deferred COMMIT failure, cancellation, real adjacent-counter collisions, actual
schema-1 empty/populated rejection, ten incompatible counter shapes and hit-proven registration/
bootstrap initialization faults. This is prepared-source atomicity evidence, not a live login rollout.

Fresh explicit source installation/bootstrap now activates schema 2 with watermark `-1`. Existing
schema 1 is refused without writes or repair; password rotation never resets consumption. Current
credentials/incarnation, the old watermark and source pins fence both transactions; post-write
readbacks and the final exact-counter proof precede COMMIT. Uncertain acknowledgement still does
not prove rollback or authorize counter reset. See [ADR-006](collaboration-auth-source.md).

**304 PostgreSQL cases passed** on the private PostgreSQL 16 instance: consumption 11, schema 4,
freshness 7, source 14, rotation 17, bootstrap 14, Session boundary 14, password profile 6, collaboration
commands 16, deletion 16, reconciliation 12, provisioning 9, Session Tasks 17, Artifacts 18, inputs 20,
Reviews 18, catalogue 16, revisions 17, content 25, standalone content HTTP 11 and unchanged legacy auth
22. The legacy subprocess alone explicitly enabled its fallback setting. Exact runner names
or nonempty target lists were checked before execution. Existing rotation tests use explicit synthetic
Sessions when login is not under test; fault-hit counters prevent premature OTP rejection from being
counted as rollback coverage. The freshness suite retains its real waits and now distinguishes the
reusable pure freshness predicate from composed counter consumption.

The backend gate passed **504 default Rust tests and 129 common script checks**, with **494 opt-in
cases ignored** by the default run. The SQL executions above were separate, not all ignored cases.
The ten OTP source/CI guards also passed directly. Bilingual contracts, security, testing and
architecture views were updated in place; the tensor now has **24 evidence items and 237 paths**,
still **57 cells / 73 edges / 16 chains**, with maturity scores and public connection gaps unchanged.
Earlier dated records below retain their original outcomes and limitations.

Tests use synthetic accounts, private temporary files and a 0700 Unix socket with no TCP listener.
Inherited deployment/database configuration was cleared. Provisioning's synthetic local socket tests
do not establish SCRAM/transport or deployed credential security. No frontend/browser, SDK runtime,
live advisories, kernel/LAN, deployment/migration/restore acceptance or remote CI run is claimed.
Product version remains 0.5.0; no commit, push, installation or live authentication switch was performed.
After local fixture-only unused-method warnings were corrected, the full backend gate passed again
with the same totals and no compiler warnings. Formatting, lengths, course/tensor/version consistency,
API/SDK compatibility and synthetic management/distribution checks passed. The database had no remaining
test namespaces, event hooks, extra roles or clients; its server was stopped and the exact 217 MiB data
directory, socket and 2.6 MiB generated fixtures removed. Diagnostic logs were retained; no live data,
dependency cache or unrelated checkout was deleted.

### Earlier pure OTP consumption stage

This records the initial pure-only stage on 2026-10-04, before the atomic schema-2 integration above;
its unchanged-file and unwired statements describe that earlier run, not the current source.

On 2026-10-04, **16 pure policy tests passed**, after the unimplemented policy seam failed 13 positive
cases (three rejection/state cases already passed). This was TDD for a new, unwired contract, not a
new reproduction or fix of runtime replay. Fixed public HMAC fixtures prove adjacent counters can
share one code and different keys can share a code/counter. Tests cover all-match watermark rejection,
highest selection, credential-bound exact-counter rechecks, normalization/byte bounds, epoch, negative
time, large counters and caller-supplied state. Preparing twice still proposes the same transition;
only a future atomic persistence layer can choose a winner.

**Three source guards passed**: the module remains private and dormant, takes explicit inputs with
no database/environment/current-time effects, and does not expose/copy/serialize its private binding.
Independent review confirmed the pure boundaries. Login, rotation, source schema/install/bootstrap,
reconciliation, old AuthService and migrations were left unchanged; runtime code reuse remains possible.
The [contract](collaboration-auth-source.md) records the necessary account-bound transaction, versioning
and existing fault-test adaptation before activation. No public clock, endpoint or runtime feature switch
was added. The tensor records a twenty-third evidence item for I04/I06 without changing scores or edges.

The backend-only gate passed **504 default Rust tests and 126 common script checks**, with **479
opt-in cases ignored** and no compiler warnings. The first full attempt stopped when the new test
parent directory inherited group-write permission and the unchanged Artifact root check refused it.
After restricting that disposable parent to 0700, the complete gate passed; no product check was weakened.
Formatting, file lengths, version/course/tensor consistency, API/SDK compatibility and synthetic
management/distribution checks passed. Inherited database/deployment settings were cleared.
Hashes of existing login/rotation, OTP freshness, account/install/bootstrap/reconciliation, legacy-auth
implementation and migration files stayed unchanged; the parent gained only a private module declaration.
PostgreSQL opt-in cases, frontend/
browser, SDK runtime, live advisories, kernel/LAN, deployment and remote CI were not rerun; the prior
day's SQL evidence below remains historical. Version stays 0.5.0, without commit, push or installation.
The final common/format gate passed all 126 checks again. The 2.6 MiB generated fixtures were removed,
with diagnostic logs retained; no database server was started or live data/dependency cache removed.

### Earlier prepared OTP freshness stage

This historical run preceded consumption; the current source now rejects committed counter reuse.

On 2026-10-03, a PostgreSQL regression observed login waiting on the exact source writer fence,
advanced only its private test clock beyond the accepted OTP window, and reproduced an incorrect
`Ok(())` result. After adding the missing checks, **all seven exact SQL cases passed**: login writer
and INSERT waits, rotation's post-revocation wait, unchanged-clock success at four corresponding
barriers, the existing rotation writer check, snapshot release while the password executor is held,
and same-window reuse across independent sources. Trigger counters prove post-write faults were reached; full source snapshots
and still-valid old Sessions prove rejected rotations restore credentials and revocations.
The held-executor case observes the read transaction's COMMIT, not exact dispatch into Tokio's
queue; a separate source guard ensures OTP is checked after awaiting password verification.

**11 pure tests passed**, covering the existing six-digit SHA-1 adaptation of RFC test vectors,
exact 30-second/one-step-window edges, input/secret normalization, malformed/empty secrets, negative
time, epoch, large counters and deliberate lack of consumption. Three source guards preserve the
three checks in each command, no awaited SQL between final OTP check and commit, and a test-only clock
without legacy changes. A separate CI guard keeps all seven library SQL names selected explicitly.

The contract is current application-UTC validity **before requesting commit**, after the last SQL
read/time check. It is not commit-acknowledgement freshness, clock-rollback protection or one-time
consumption; the same code can still create two Sessions inside the window. Session expiry remains
on the database clock. No machine clock, schema version, public route or old AuthService changed.
See [ADR-006](collaboration-auth-source.md) for this boundary and the remaining public-issuer gates.

**108 PostgreSQL cases passed** on the private PostgreSQL 16 test instance: OTP 7, password profile 6,
source 14, password rotation 17, bootstrap 14, source Session boundary 14, Session content 25 and
standalone HTTP 11. The seven OTP cases and 11 pure cases passed again after tightening the
held-executor test's name and evidence description; this rerun does not add distinct cases.

The backend-only gate passed **488 default Rust tests and 123 common script checks**, with **479
opt-in tests ignored** and no compiler warnings. Formatting, file lengths, version/course/tensor
consistency, public API/SDK compatibility and synthetic management/distribution checks passed.
After the final test/document changes, the common/format gate passed all 123 checks again.
Inherited deployment/database settings were cleared; tests used only synthetic data and private
temporary files. Frontend/browser, SDK runtime, provisioning CLI/transport, independent identity/
deletion suites, advisories, kernel/LAN, deployment/migration and remote CI were not rerun. These
results are scoped adjacent coverage, not all ignored tests or end-to-end deployment acceptance.
No test schemas, event triggers or other clients remained. The server was stopped and its 133 MiB
database, 2.6 MiB generated fixtures and private socket directory removed; diagnostic logs were
retained. No live data or dependency cache was removed.

The tensor adds a twenty-second evidence record for I04 and I06 only; scores and missing connections
remain unchanged. Version remains 0.5.0 with no commit, push, installation or live-auth cutover.

### Unreleased prepared password profile

On 2026-10-03, a safe low-cost PHC regression first returned `Ok(true)` for a valid but unsupported
Argon2 profile instead of `InvalidRecord`. The fixed prepared policy then passed **25 targeted unit
tests**: 13 profile cases and 12 worker cases, including the new rejection and updated malformed-record
classification. Extreme numeric costs are exercised only through pure validation, never expensive KDF
execution. Three source checks cover the prepared call sites, pre-admission/account validation and
separation from legacy crypto; a new exact-selection CI guard covers the six SQL cases.

The accepted profile is the former official writer output: Argon2id v19, m19456/t2/p1, 32-byte digest.
Duplicate, missing, extra and malformed parameters fail closed. Salt is fully decoded and bounded,
not newly required to be a UUID or match the separate column. Writers now use explicit constants.
This does not migrate credentials, silently repair storage or revoke existing Sessions; it adds no
public issuer or process RSS/wall-clock bound. [ADR-006](collaboration-auth-source.md) records the contract.

**101 PostgreSQL cases passed** on a fresh private PostgreSQL 16.15 instance: new profile 6, source 14,
password rotation 17, bootstrap 14, source Session boundary 14, Session content 25 and standalone HTTP
11. The new six use an exact-name runner; they cover rejected stored records without writes, existing
Session/logout behavior, supported writer compatibility and register/rotation post-write rollback.
The original source/rotation/bootstrap lists were verified before execution; the other suites use
their existing exact runners. No selected SQL assertion failed, and compiler output had no warnings.

The backend-only gate passed **477 default Rust tests and 119 common script checks**, with **472
opt-in tests ignored** and no compiler warnings. Formatting, file lengths, version/course/tensor
consistency, public API/SDK compatibility and synthetic management/distribution checks passed.
After documentation updates, the final common/format rerun also passed all 119 checks. A restricted
attempt first failed in two tool fixtures (including a confirmed loopback `listen EPERM`); rerunning
with the same scoped local-test permission as the full gate passed without a product change.

Inherited deployment/database settings were cleared. Tests used only synthetic accounts/content and
private temporary files; no test schemas, event triggers or other database clients remained after SQL.
Frontend/browser, SDK runtime, provisioning CLI/transport, independent identity/deletion suites,
advisories, kernel/LAN, deployment/migration and remote CI were not rerun. This is scoped adjacent
coverage, not every ignored test or a fresh end-to-end deployment acceptance.
The test server was stopped and its 133 MiB database plus 2.6 MiB generated fixtures removed; logs
were retained. No live data or dependency cache was removed.

The tensor adds a twenty-first evidence record for I04, I06 and O01; scores and missing edges remain
unchanged. Source version remains 0.5.0, without commit, push, installation or live-auth cutover.

### Unreleased prepared password work

On 2026-10-03, a deterministic regression against an unguarded worker stub returned `Ok(99)` when
cancellation should have left its occupied capacity unavailable. After implementing the gate,
**all 11 unit cases passed**: dispatched/total saturation, unpolled and queued cancellation, running
timeout/cancellation, cancellation while queued in Tokio, panic/closed recovery, deferred secret
copies, shared process identity and real Argon2 verification. Two source-wiring checks cover every
prepared hash/verification call site and preserve the separate legacy helpers.

The gate covers all five computation points across registration, login, password rotation and
bootstrap without changing SQL transaction order, credentials, public routes or configuration.
Both permits follow actual blocking work, not the request's lifetime; admission waits remain within
existing operation deadlines. It is not a single-job memory/time limit, OTP consumption policy or
protection against all live runtime work. [ADR-006](collaboration-auth-source.md) records the scope.

The backend-only gate passed **463 default Rust tests and 117 common script checks**, with **466
opt-in tests ignored** and no compiler warnings. Formatting, source lengths, version/document/tensor
synchronization, public API/SDK compatibility and synthetic management/distribution checks passed.

Separately, **95 PostgreSQL cases passed** on a fresh private PostgreSQL 16 instance: source 14,
password rotation 17, bootstrap 14, source Session boundary 14, Session content 25 and standalone HTTP
11. The first three targets had their ignored-case lists checked before running; the other three
used exact runners. An initial count assertion stopped before the password suite because 18 included
its one default test; the verified SQL count is 17, not 18. No product assertion failed in these runs.
This is selected adjacent coverage, not a rerun of all 240 SQL cases from the preceding slice.

Inherited deployment/database settings were cleared; only synthetic accounts/content and private
temporary files were used. The database had no remaining test schemas, event triggers or other
clients before shutdown. Frontend/browser, SDK runtime, provisioning CLI/transport, independent
identity/deletion suites, advisories, kernel/LAN, deployment/migration and remote CI were not rerun.
After the final common/format gate passed, its 133 MiB database and 2.6 MiB generated fixtures were
removed; diagnostic logs were retained. No live data or dependency cache was removed.

The tensor adds a twentieth evidence record for I04, I06 and O01 without changing any scores or
connecting a missing edge. Version remains 0.5.0; no commit, push, install or live-auth cutover.

### Unreleased source-only Session boundary

On 2026-10-03, fault injection reproduced a false logout acknowledgement: a trigger suppressed the
delete and redirected readback to an empty shadow table while the real Session remained usable.
The regression first failed against 0.5.0 and then passed with the source guard. Login now retains
namespace/table identity across password verification; all three Session methods check compatible
source relations and authority, with final expiry validation after guard waits. Source-only operation
still needs no registry. Installation, registration, password/deletion and live authentication are
not redesigned; [ADR-006](collaboration-auth-source.md) records the tighter connection rules.

The new exact runner passed **14 PostgreSQL cases**, and the original durable-source target passed
all **14 SQL cases** separately. Tests cover normal three-table operation, ambiguous/temporary paths,
RLS/views/incompatible storage, authority/schema drift, same-valued relation and namespace replacement,
logout ordering and expiry. The password-gap case holds the worker queue and observes the completed
first transaction before replacing the namespace; it does not rely on a sleep winning a race.
CI selection is guarded against omissions and zero matches; remote CI was not run.

All **212 preceding resource/content SQL cases** also passed through the twelve exact runners in the
[testing guide](testing-guide.md), giving **240 separate database passes** with the two source suites.
This covers the shared private-work guard through Task, Artifact, Review, schema-3 content and HTTP
composition, not all identity/lifecycle or ignored tests in the repository.

The backend-only gate passed **452 default Rust tests and 115 common script checks**, with **466
opt-in Rust cases ignored**. The SQL cases are executed separately, not counted as default passes.
Formatting, public API/SDK compatibility, synthetic management/distribution tooling, source lengths,
version metadata and document/tensor consistency checks passed; no compiler warnings were reported.

Execution cleared inherited deployment/database settings and used PostgreSQL 16 with a private 0700
Unix socket, no TCP listener and only synthetic accounts/content. No test schemas or other clients
remained; the exact server was stopped and its 152 MiB data plus 2.6 MiB generated fixtures removed,
retaining diagnostic logs. Frontend/browser and SDK runtime tests, independent lifecycle SQL,
dependency advisories, kernel/LAN, deployment/migration and browser-to-server acceptance were not rerun.

The capability tensor retains 57 coordinates, 73 edges and 16 paths, with a separate 19th evidence
record for I04. Its scores remain **3 / 1 / 3 / 1**: worker admission, OTP reuse/timing policy and
public issuer/installation composition remain pending. Version stays 0.5.0; no commit or push.

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

1. Define a durable-before-dispatch intent journal, retained-record trust and recovery identity before
   exposing C2-P or allowing retry. C2-R remains data; C2-S adds explicit current-authority inspection,
   not original-Session continuity, historical commit proof or a restored attempt. C2-Q's surviving
   original-attempt contract is unchanged. Unknown steps stay stopped; absence permits neither retry nor deletion.
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
