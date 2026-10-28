# Project status

Reviewed **2026-10-26**. Product version is **0.6.5**; the public API compatibility baseline remains
**0.3.0**. Task-bound publication/strict HTTP and default-off `--task-text-publication` extend the
0.6.4 text/Session-page metadata paths, not browser raw-text saving or browser TLS acceptance. See the
[0.6.5 report](../../reports/releases/0.6.5/README.md); earlier artifacts and dated evidence retain their scope.

Cyanrex is a self-hosted **collaboration and teaching platform**. A Task describes work; code is
one optional payload, not the organizing center. Notes, configuration and code can use the text
editor without requiring execution, automatic assessment or a Run. eBPF teaching remains the
first supported domain and its existing workflow stays compatible.

The mainline now has an explicit low-privilege platform host and a bounded private Task backend.
The normal browser still edits local drafts rather than saving generic Tasks to that backend.
Shared collaboration, reliable generic execution and deployed-LAN acceptance remain later work.

## Current operating boundaries

Three application paths coexist. They are not interchangeable authentication or storage modes.

| Path | What works in source | Boundary |
|---|---|---|
| Teaching application | Existing browser authentication, classrooms, five labs, feedback, execution and teacher deployment management | Uses the existing AuthService and teaching storage; old attempts are not generic Tasks or Runs |
| Local Task editor | Optional text payload, controlled editors, reviewed import and local JSON export | Page-memory drafts; no generic server save, autosave or browser persistence |
| Explicit platform host | Linux plain/native-TLS loopback hosting, limited Sessions, private Tasks, metadata saves and optional Task-bound publication | Original provisioned storage; no normal-app/text-save/SDK cutover or LAN listener |

Additional Rust stores, HTTP adapters, draft-publication helpers and file-delivery components can be
composed explicitly. Their availability does not mean that the normal application mounts them.
The [architecture map](architecture-map.md) owns module responsibilities; the
[platform network](platform-network.md) identifies connected and missing feature edges.

### Teaching and deployment

Existing teaching remains teacher-authoritative. In single-user use, the user is also the teacher
and manages deployment. Password/TOTP, Cookie Sessions, CSRF, student restrictions and invitation
onboarding belong to that existing instance authority.

Linux, WSL2 and Docker launch paths remain supported. SSH tooling manages explicitly selected,
preinstalled packages; it is not automatic bare-host installation. Launcher targeting, health
observations, package-control checks and page-local deployment diagnostics are implemented.

The local Engine still combines control with privileged Linux execution. Shared-kernel limits do
not isolate students, and registration of a remote compiler worker does not grant remote eBPF loading.
Use the [security guide](security.md), [runtime reference](runtime-reference.md) and
[testing guide](testing-guide.md) for current operating and validation requirements.
The [historical acceptance record](acceptance.md) preserves the 2026-09-09 checkpoint only.

### Local Task payload editing

`/tasks/new` owns a Task draft; `/editor` opens the same container. The eBPF workbench remains
separate. A draft may have no payload or up to 32 text items, each at most 256 KiB.

The parent draft owns filenames, language hints and text. Import previews, confirmations and
revision/generation checks protect newer edits against obsolete callbacks. Local multi-file import
is reviewed before an atomic draft change; JSON import/export is bounded. Exporting a file is not
a server commit, and leaving the page can lose unsaved work.

Fourteen installed editor profiles are independent of valid portable language hints. Unknown valid
hints survive import/export and use plain-text editing. JS/TS and configured JSON/HTML/CSS providers
offer browser-local assistance; Rust, Python and C/C++ do not imply rust-analyzer, Pyright or clangd.
There is no project filesystem or external LSP transport.

The UI supports English, Simplified Chinese, Spanish, Japanese and Korean. English/Chinese project
and course sources are maintained separately from interface translations; Korean UI support does
not imply Korean course bodies. See [Task editing](editor.md) and [local file import](local-text-file-import.md).

## Private platform host

Source 0.6.5 retains `cyanrex-platform serve-loopback` and `serve-loopback-tls`, verifying low-privilege
identity, private format 3 input, the original catalog/five markers/Artifact root, without teaching AppState.

The 0.6.4 unbound text route remains enabled in both CLI modes; 0.6.5 adds one default-off Task-bound
route. Both use the same original retained storage binding:

| Method and path | Implemented command |
|---|---|
| POST `/platform/v1/session/login` | Bounded existing-account password/TOTP login and confirmed dedicated Cookie projection |
| GET `/platform/v1/session` | Fresh current-Session status |
| POST `/platform/v1/session/logout` | Exact-token logout; clear Cookie after confirmed success |
| GET `/platform/v1/task-index` | Bounded current-owner Task metadata page |
| GET `/platform/v1/tasks/{task_id}` | Reopen an owned Task, including empty or verified ordered text content |
| POST `/platform/v1/tasks` | Create an empty manual Draft using a strict version/Task-ID/title request |
| PUT `/platform/v1/tasks/{task_id}/content` | Replace a manual Draft manifest with an expected revision and existing exact owned text references |
| POST `/platform/v1/text-artifacts` | Create a private fixed-profile text Artifact; return only confirmed metadata |
| POST `/platform/v1/tasks/{task_id}/text-artifacts` | Explicit opt-in: publish against a complete owned manual-Draft baseline; do not update or attach to the Task |

The private index is metadata, not proof that every content file remains readable. Reopening checks
the exact referenced content rather than trusting the index. Creation requires neither payload nor
code. Replacement can change a title, labels, order or references; it publishes no new Artifact bytes.
Task status, catalogue creation, assessment and execution are not mounted by these commands.

Default CLI construction stays eight-route/seven-lane; old constructors stay seven-route/six-lane
without publication. Only `with_task_text_publication_capacities` or `--task-text-publication` opts into
nine routes/eight response-lifetime lanes. All eight capacities are checked before one original storage
Arc split. Strict HTTP pins the full Task/manifest; current ownership is rechecked before any new file work.
Each text connector has four actual workers (eight together); global admission is still 16 connections,
not 32 guaranteed requests. Pooled write/read and file-distribution routers remain separate.

Native TLS adds private certificate/key checks, exact IP SAN matching, bounded handshake work and
Secure Cookies. Optional service notifications report verified Ready and pre-drain Stopping.
Neither mode supplies DNS/LAN hosting, an installed service, automatic restart, a proxy backend or
a complete browser Task workflow. Plain loopback availability alone is not a Cookie-safe deployment plan.

Included in 0.6.4, `--session-ui` is default-off: seven fixed assets/five languages, dedicated Sessions,
explicit 20-entry private pages and inert text/empty-payload reopening. Strict scope/revision/pins/hash
checks use 128 KiB index/50 MiB content wire, 16,384 frames and 32 × 256 KiB decoded limits. A narrow
manual-Draft title/display-label save preserves pins/order/bytes/count: one 64 KiB PUT, strict HTTP 200
metadata ACK (128 KiB/4,096 frames), not a fresh text read. Save/dirty/unknown leave dialogs default Cancel.
Ten seconds bounds waiting, not parsing/server work. Denial/context changes/pagehide clear private DOM
and quarantine local work without cross-Session restore/replay; explicit discard is required to continue.
No raw-text save, new Artifact, Task creation/Run, normal Next draft/SDK, new API or grant is connected.
Unknown is not rollback/retry authority. Cookies are not port-isolated; legacy NSS still blocks browser TLS.

For setup and exact contracts, start at the [platform references](reference-index.md).
The [loopback guide](platform-loopback-host.md), [TLS guide](platform-loopback-tls-host.md) and
[replacement guide](platform-task-replace-connection.md) describe operational and transaction limits.

## Collaboration foundations

### Identity and current authority

The separately composed backend has stable identities, audited membership/deployment policy,
durable Sessions, account lifecycle commands, explicit provisioning and read-only reconciliation.
Prepared authentication includes bounded password work, fixed Argon2 profiles and atomic per-account
OTP consumption.

Untrusted resource commands derive the owner from the current Session. Account incarnation,
audited active membership, original source/storage bindings and final fresh Session expiry remain
inside the command transaction. Trusted direct stores accept supplied owners but do not authenticate
them; they are not public permission APIs.

The explicit host does not replace teaching login or migrate existing users. New Workspace
ownership does not grant access to other owners' content or other instances' deployment controls.

### Tasks and domain definitions

Manual Tasks need no domain definition. Catalogue Tasks retain the complete exact frozen definition;
definitions and payload are separate choices. Provider-free metadata catalogues can combine bounded
domain/package versions and explicitly have both assessment bindings or neither.

Typed evaluation still requires its exact evidence-schema and policy pins. A definition lookup,
valid payload or successful Run does not approve a Task. Generic assignment, dependencies,
cross-user acceptance and automatic rule execution remain absent.

### Artifacts and Review

Artifact storage supports private immutable revisions, digest-bound references and transactional
metadata/events. Generic pooled publication/read APIs remain separate from the retained host;
its independently selected text commands use the original storage binding instead.

[Retained text publication](platform-text-artifact-connection.md) began in 0.6.4; 0.6.5 adds a Task-bound
connector/strict POST and explicit host opt-in. Both use fixed text profiles and deferred exact-head
checks, without browser text saving or Task updates. Existing combined login/Task/lock grants and
three Artifact INSERTs suffice; the Task-bound opt-in adds no grant or old-payload byte verification.

Task content storage atomically updates Task/manifest/outbox metadata. Existing text replacement
validates the complete old/new reference union, including removed content, around deferred writes.
It preserves immutable Artifact files and prior Review history.

Review supports revisioned private human opinions on exact Artifact targets/evidence. It does not
supply cross-user reviewer grants, generic Task acceptance, automatic assessment or AI review.
A complete browser text-save workflow still needs reviewed browser authentication/transport integration
and client conflict/uncertain-outcome handling, not just another editor button.

### Draft publication and recovery

Internal import/planning, create-only publication steps, checkpoints, immutable intents, journaled
dispatch and read-only observation are implemented as distinct capabilities. Manual and catalogue
paths preserve their own definition, identity and outcome rules.

An Unreleased [browser-side content-save attempt](task-draft-publication.md#unreleased-browser-side-existing-draft-save-attempt)
freezes a draft and existing manual Task baseline, hashes text locally, then hands out single-use
Artifact/manifest descriptors with separate pending outcomes. Supplied confirmations establish only
consistency; no fetch, Session/Origin configuration, UI saving or persistent recovery is connected.
Completion covers the frozen attempt only and does not clear the current local editor's dirty state.

These helpers do not create a whole-draft transaction. Confirmed prefixes can survive a failed later
step; metadata agreement is not a historical receipt. Reading an intent or checkpoint does not
reconstruct a dropped attempt, authorize retry or permit deletion based on absence.

### Large-file distribution

The file chain includes strict manifests, private exclusive staging, logical upload quotas, local
receipts, chunk journals, whole-byte verification and Published metadata. Current publisher and
independent exact-recipient grants gate authorized sharing and access.

Explicit manifest/chunk HTTP composition and controlled clients connect metadata, ordered chunks,
verified assembly and host-selected saving. Progress and cooperative cancellation are per-call
observations. Real Source/HTTPS fixtures exercise this chain with disposable native peers.

This remains separate from normal-app delivery and the loopback Task host. User-facing save dialogs,
SDK wiring, deployed-LAN distribution, durable resume/recovery, physical disk policy and desktop
peer transfer are not complete. Centralized distribution remains the first delivery target.
See [file distribution](large-file-distribution.md) and the [reference index](reference-index.md).

### Execution and integrations

The teaching runtime has local leases/quotas/timeouts and a signed Runner Agent protocol with
registration, jobs, probes and compile-only diagnostics. Runner Agents are compute workers, not AI
identities, general Task orchestration or remote kernel sandboxes.

AI connection profiles and explicitly selected SDK tool adapters assist a trusted host. The host
still owns model transport, secrets, approvals and disclosure. There is no autonomous AI participant,
delegation service, provider transport or executable plugin runtime.

Resource outboxes exist, but a generic durable business-event dispatcher, cursor replay,
multi-Engine coordination, durable Runs and reviewed partner integrations remain open.

## Security and compatibility rules

- Product version, API baseline and storage schemas evolve independently. The current version does
  not install a schema or certify a running deployment.
- Task schema 2 and content schema 3 require their own explicitly installed namespaces. Prepared
  authentication requires Source schema 2; existing incompatible formats are refused, not migrated.
- Retained operations verify original storage and current authority, without pool fallback or
  first-target adoption. Metadata matching does not prove physical or historical continuity.
- Mutation success requires confirmed COMMIT. Cancellation, timeout, disconnect or lost delivery
  can leave an unconfirmed outcome; none proves rollback or authorizes blind retry/compensation.
- Filesystem and SQL checks are not one atomic transaction, a hard process deadline or protection
  against the same Unix identity/root. Dispatched blocking work retains its real worker budget.
- Teacher authority in the teaching product is not universal private-content authority in the
  generic platform. TLS does not add sharing policy, tenant isolation or LAN acceptance.
- Existing teaching contracts and the public 0.3.0 API baseline remain compatible. Dedicated
  platform routes are not added to the generated public SDK/OpenAPI surface.
- Updating source dependencies does not patch already-running binaries, images or generated assets;
  operators must rebuild and redeploy the affected components.

These rules also constrain the next steps. Detailed threat boundaries belong in
[security](security.md); current implementation maturity is recorded in [capability maturity](capability-maturity.md),
without one whole-project completion percentage.

## Next Decision Points

The immediate mainline is a **private, payload-neutral Task workflow**. The retained host supplies
a bounded server edge, not the completed browser product. Work should proceed in this order:

1. Connect the reviewed Task-bound edge to a browser contract with Cookie-safe Origin/client trust
   and separate publication/Task-update outcomes. Preserve current authority/private file ownership;
   do not mount pooled routers or infer permission from a local draft ID.
2. Extend the opt-in metadata-save page to text publication only through separately reviewed
   contracts. Preserve separate local drafts, unsaved edits, expected revisions, conflicts and cancel/unknown states.
   A local export and a successful metadata PUT must remain distinct operations.
3. Verify the real browser-to-server chain with actual authentication/storage, competing edits,
   rejected authority, restart behavior and uncertain delivery before calling private saving complete.
4. Add separately reviewed sharing/reviewer grants and version-specific acceptance. Demonstrate
   a non-teaching collaboration workflow that requires neither teacher/student roles nor a Run.
5. Advance protected-LAN operation, distribution UI and desktop exchange only with explicit trust,
   credential, deployment and compatibility decisions; loopback TLS is not their acceptance.

Automatic recovery/retry, migration, generic Run scheduling, AI delegation and executable plugins
need separate review, not incidental inclusion in private saving. This ordering promises no date or version.

## Verification and evidence

Verification is scoped by commit, command and boundary, not by the number of exported modules.

Read test coverage in three layers:

- Default gates cover their selected source, contracts, tooling and build checks; ignored cases are
  not passes.
- Explicit SQL/TLS runners exercise disposable external boundaries and only their enumerated cases.
- Browser fixtures with mocked Engine replies establish UI behavior, not actual server authorization
  or saved Task persistence. Real deployed acceptance needs its own path.

| Evidence entry | What it establishes |
|---|---|
| [0.6.3 release validation](../../reports/releases/0.6.3/README.md) | The dated `164d64a` source validation: full local default gate, 208 exact SQL cases and 35 mocked-backend browser cases; not later HEAD or deployed acceptance |
| [Testing guide](testing-guide.md) | Current default, opt-in SQL, browser, SDK, TLS and privileged selections, with their limits |
| [Security guide](security.md) | Current dependency floors and threat/permission boundaries; audits do not establish complete dependency safety |
| [Capability maturity](capability-maturity.md) | Dated architecture/function/implementation scores and missing edges; not an aggregate release gate |
| [Historical acceptance record](acceptance.md) | The 2026-09-09 checkpoint and its separate live kernel, network and distribution limits; not current acceptance |
| [Development history](development-history.md) and archives below | Original implementation evidence, including failures, ignored selections and then-current scope |

After the version commit, `ab2b292` updates the Rust TLS/certificate dependency floors without new
audit exceptions; `be94db9` updates Next.js to 15.5.28 and DOMPurify to 3.4.17, including the actual
Monaco embedded sanitizer. Their local regression evidence is separate from the published release.
For the sanitizer, the actual old 3.4.16 asset produced 20 failures in 33 cases; the regenerated
3.4.17 asset passed all 33, including the original ten. These opt-in component/browser fixtures
block external networking and do not establish Next.js exploitability or a deployed asset version.

Remote CI, release artifacts and deployed services require their own verified results. This status
does not claim that the post-release commits passed remote CI, upgraded installed instances or
re-ran every optional suite. Documentation reorganization is not another product acceptance run.

### 2026-10-26 Unreleased Session-page validation

On the uncommitted working tree, locked default Cargo tests passed **1,451**, with **1,028 ignored**
and no warnings. The common/format gate passed **584** source/contract tests and its tooling checks.
The new route/CLI tests are included in that Cargo count, not additional passes.
Explicit real Chrome UI fixtures passed **12** with mock APIs; the real plain SQL/browser selector
passed separately, including the failed-logout/manual-check/explicit-confirmation path. Its OTP is
handed off only after the browser is ready, with no login retry. Existing plain/TLS host and text
publication regressions passed **22** exact SQL selectors. The adjacent pure content-save attempt
passed **36** units; this is not Task saving through the new page. Desktop/360px screenshots were
checked. TLS-browser acceptance remains blocked by local legacy NSS; no CA/HOME/verification bypass
was used, and CI wiring is not a remote pass. The owned socket-only PostgreSQL 18 fixture was stopped
and its 56 MiB generated data removed; logs/screenshots remain outside the repository. No version,
commit or push was made for this slice.

The same-day read-only Task follow-up passed 1,451 default Rust tests (1,029 ignored), 610 common
tests (including 20 pure read-contract units), 12 new and 12 original mocked-API browser cases,
the real plain Task and Session browser selectors, and 16 existing index/content SQL regressions.
The malformed-401/403-body active-state bug had a browser Red then Green; no old content survives denial.
Desktop/360px screenshots were checked; this is not Task saving or TLS-browser acceptance. The temporary database was stopped and its generated data removed.

## Development history

Detailed former status content is archived with its original versions, dates, hashes, failure
counts and Unreleased labels. The labels describe their implementation stage; later source inclusion
does not rewrite those runs as new release checks.

| Archive | Original content |
|---|---|
| [Status snapshot](history/project-status-snapshot-0-6-3.md) | Former English lines 1–238: 0.6.3 inclusion note, older snapshots, capability tables and boundaries |
| [October 8–11 validation](history/project-status-validation-2026-10-08-11.md) | Former lines 239–1046: dated release and implementation records |
| [October 7 and earlier validation](history/project-status-validation-2026-10-07-and-earlier.md) | Former lines 1047–2000: earlier evidence and the then-current roadmap |

The [reference index](reference-index.md) lists the full topical catalogue. Use the
[documentation index](README.md) for role-based entry points and the current architecture/network
views for implementation navigation rather than treating a historical snapshot as current scope.
