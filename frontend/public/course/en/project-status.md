# Project status

Reviewed **2026-10-25**. Product version remains **0.6.3**; the public API compatibility baseline is
**0.3.0**. HEAD also includes the Rust dependency follow-up `ab2b292` and frontend dependency
follow-up `be94db9`, after the `164d64a` version commit. Those fixes do not change the published
0.6.3 artifacts or turn the release report into validation of a later commit.

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
| Explicit platform host | Linux plain or native-TLS loopback hosting with limited Session issuance and private Task commands | Separately provisioned original storage; no normal-app/browser/SDK cutover or LAN listener |

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

Source 0.6.3 includes explicit `cyanrex-platform serve-loopback` and `serve-loopback-tls` modes.
They verify low-privilege process identity, private format 3 input, the declared original catalog,
five storage markers and the original Artifact root. They do not construct teaching AppState.

The host combines seven concrete routes over one original retained storage binding:

| Method and path | Implemented command |
|---|---|
| POST `/platform/v1/session/login` | Bounded existing-account password/TOTP login and confirmed dedicated Cookie projection |
| GET `/platform/v1/session` | Fresh current-Session status |
| POST `/platform/v1/session/logout` | Exact-token logout; clear Cookie after confirmed success |
| GET `/platform/v1/task-index` | Bounded current-owner Task metadata page |
| GET `/platform/v1/tasks/{task_id}` | Reopen an owned Task, including empty or verified ordered text content |
| POST `/platform/v1/tasks` | Create an empty manual Draft using a strict version/Task-ID/title request |
| PUT `/platform/v1/tasks/{task_id}/content` | Replace a manual Draft manifest with an expected revision and existing exact owned text references |

The private index is metadata, not proof that every content file remains readable. Reopening checks
the exact referenced content rather than trusting the index. Creation requires neither payload nor
code. Replacement can change a title, labels, order or references; it publishes no new Artifact bytes.
Task status, catalogue creation, assessment and execution are not mounted by these commands.

Six independent response-lifetime admission lanes cover login, Session lifecycle, index, content
read, empty creation and manifest replacement. Explicit construction preserves older narrow APIs;
the full pooled write router and file-distribution router are not silently merged into the host.

Native TLS adds private certificate/key checks, exact IP SAN matching, bounded handshake work and
Secure Cookies. Optional service notifications report verified Ready and pre-drain Stopping.
Neither mode supplies DNS/LAN hosting, an installed service, automatic restart, a proxy backend or
a browser deployment. Plain loopback availability alone is not a browser Cookie deployment plan.

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
metadata/events. Internal current-Session publication and reads, plus independent text HTTP
preparation, exist outside the retained host's mounted Task surface.

Task content storage atomically updates Task/manifest/outbox metadata. Existing text replacement
validates the complete old/new reference union, including removed content, around deferred writes.
It preserves immutable Artifact files and prior Review history.

Review supports revisioned private human opinions on exact Artifact targets/evidence. It does not
supply cross-user reviewer grants, generic Task acceptance, automatic assessment or AI review.
A complete browser save workflow still needs reviewed new-byte publication and client conflict/
uncertain-outcome handling, not just another editor button.

### Draft publication and recovery

Internal import/planning, create-only publication steps, checkpoints, immutable intents, journaled
dispatch and read-only observation are implemented as distinct capabilities. Manual and catalogue
paths preserve their own definition, identity and outcome rules.

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

1. Define an explicit new-text publication boundary for the original retained storage, including
   current authority, private file ownership and uncertain commit outcomes. Do not mount an old
   pooled router or infer permission from a local draft ID.
2. Connect browser authentication, private listing/reopening and explicit saving through reviewed
   contracts. Preserve unsaved edits, expected revisions, conflicts and cancel/unknown states.
   A local export and a successful metadata PUT must remain distinct operations.
3. Verify the real browser-to-server chain with actual authentication/storage, competing edits,
   rejected authority, restart behavior and uncertain delivery before calling private saving complete.
4. Add separately reviewed sharing/reviewer grants and version-specific acceptance. Demonstrate
   a non-teaching collaboration workflow that requires neither teacher/student roles nor a Run.
5. Advance protected-LAN operation, distribution UI and desktop exchange only with explicit trust,
   credential, deployment and compatibility decisions; loopback TLS is not their acceptance.

Automatic recovery/retry, storage migration, generic Run scheduling, AI delegation and executable
plugins need separately reviewed designs. They should not be incidental side effects of connecting
the private content workflow. No calendar or new version is promised by this ordering.

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
