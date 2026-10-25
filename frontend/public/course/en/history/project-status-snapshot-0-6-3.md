# Project status snapshot for source 0.6.3

Archived on **2026-10-25** from `docs/en/project-status.md`, original lines **1–238** at source commit `be94db9`.
Snapshot text through the 0.6.3 inclusion note, including the older 0.5.9 snapshot and capability summaries.

These are historical statements, not current capability or fresh acceptance claims. Original versions, dates,
Unreleased labels, hashes, failure counts and verification limits are retained. Only relative Markdown
link targets are rebased for this directory. Use [current project status](../project-status.md),
the [reference index](../reference-index.md) and [development history](../development-history.md) for navigation.

<!-- original-project-status:start -->
# Project Status

**Included in source 0.6.3:** the explicit [loopback](../platform-loopback-host.md)/[TLS](../platform-loopback-tls-host.md) host provides limited [Session issuance](../platform-login-http.md), the [retained Task index](../platform-task-index-connection.md), [read-only reopening](../platform-task-content-connection.md), [empty manual Draft creation](../platform-task-create-connection.md) and [expected-revision manifest replacement](../platform-task-replace-connection.md). Owner-private Tasks need no payload, code or Run; replacement changes a Draft title or existing exact owned text references, without publishing new Artifact bytes. Status writes, browser/SDK saving, LAN access and migration remain absent. **2026-10-16, included in 0.6.2:** [bound Session connections](../platform-session-connection.md) combine retained storage with query/logout and explicit HTTP state; [format 3 diagnostics](../platform-marker-expectation.md) require original catalog/marker declarations before row reads. Those dated changes alone start no host/issuer. Earlier snapshots and implementation-stage labels retain their original scope and evidence.

Snapshot date: **2026-10-13**. Source version: **0.5.9**, adding reviewed local multi-file text import, complete five-language UI including Korean, bilingual guides, architecture/error-boundary cleanup and Source/HTTPS test coverage. Private task indexing and explicit file delivery remain included but unmounted; normal-app distribution, server-backed task saving and resumption remain absent. Earlier task decoupling, AI host adapters and authentication protections remain in place.

0.5.4 C2-T–W retains immutable intents, explicit journaled dispatch, private journal inspection and one-resource observation. The original C2-P path stays independent; metadata agreement is not a receipt,
restart recovery or retry permission. 0.5.5 adds shared HTTP admission and separate C2-X/Y text/catalogue
routers, both unmounted; C2-Z/AA independently publishes and observes a frozen catalogue draft without
entering the manual checkpoint/journal workflow. Neither connects browser saving or automatic assessment.
0.5.5 also separates frozen definitions from providers and portable language hints from editor profiles,
composes multi-domain/package catalogues within 256 definitions, and allows explicitly absent assessment
bindings. Catalogue/text content needs neither assessment nor a Run; typed assessment still requires both pins.

The October 11 [private metadata index](../task-content-index.md), **included in 0.5.6**, lists bounded
owner-private schema 3 pages through trusted storage/current Session authorization. Explicit neutral
catalogue configuration preserves the old content modes. Its separate [HTTP adapter](../task-content-index-http.md)
remains unmounted; no live HTTP/SDK/UI list or server-backed browser reopening is connected.

The same-day **[large-file core](../large-file-core.md), included in 0.5.6**, following
[proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), adds transport-neutral manifests
and bounded synchronous IO, with centralized distribution prioritized over later desktop transfers.
It verifies supplied real bytes, not storage, publication, recipients, durable progress or delivery.
The separate [Unix chunk-staging follow-up](../large-file-staging.md) adds exclusive private files,
sync/readback and an independent job-lifetime worker budget. Neither layer enables network transfer,
durable upload progress, disk reservations, publication or live authentication/client connections.
The separate [upload-intent ledger](../large-file-upload-intent.md) adds immutable trusted metadata and
atomic logical quotas, not confirmed chunks, physical disk allocation, Session authority or transfer.
The [local upload adapter](../large-file-upload-staging.md) adds original-file receipts; the independent [write-ahead journal](../large-file-chunk-journal.md) records chunks, while [whole-byte verification](../large-file-whole-verification.md) remains a current observation. Separate [persistent publication metadata](../large-file-publication.md) combines complete confirmed chunks and fresh bytes. Separate [Session private metadata](../large-file-session-metadata.md) derives owner/account for own reservations and upload/Published reads. Separate [pure publisher/recipient contracts](../large-file-access-contract.md) match supplied exact accounts/versions, state and validity; they do not establish current audited permission. Recovery, durable grants and transport remain separate.
The [Unreleased publisher-grant ledger](../large-file-publisher-grants.md), outside the 0.5.6 tag, adds
trusted revocable records and own reads. Later [Session management](../large-file-grant-management.md) derives actors from an exact host-configured manager/account; no inherited roles, durable online manager revocation or publication consumption. Separate evidence does not refresh historical records.
Independent [Session publication](../large-file-session-publication.md) now couples current exact publisher permission to real-byte verification and a final authorized source insert; recipients, transport, recovery and live mounting remain absent. Its new evidence stays in its own record.
Later [recipient grants](../large-file-recipient-grants.md) add an independent exact-file ledger and current own-recipient metadata reader. October 12 [publisher-owned sharing](../large-file-sharing.md) adds authenticated own-Published issuance and independent owner revocation; [download-manifest observation](../large-file-download-manifest.md) checks exact Published metadata, not later byte authority. Separate [verified chunk snapshots](../large-file-chunk-read.md) connect up to 4 MiB of private bytes between original-pinned source transactions. The independent [binary HTTP adapter](../large-file-chunk-http.md) adds unmounted exact GET, byte-owner retention of both budgets and a ten-second first-frame window. A separate [HTTP receiver](../large-file-chunk-receiver.md) checks supplied responses through explicit EOF/hash before private exclusive staging, retaining buffer/control through actual save work. An explicit [single-chunk client](../large-file-chunk-client.md) adds fixed trusted HTTPS/opt-in literal-loopback requests with job-owned admission. Separate [private whole-file assembly](../large-file-chunk-assembly.md) checks legacy inputs and an exclusive actual whole output with original-inode readback. Included in 0.5.8, [ordered private download](../large-file-download-client.md) now joins sequential requests/stage ACKs to same-root assembly with real-job flow capacity; first failure/drop stops continuation. A separate [manifest HTTP adapter](../large-file-manifest-http.md) now projects current authorized metadata; the new [exact manifest client](../large-file-manifest-client.md) fetches the complete expected version and can compose metadata with ordered private download; even empty granted files require GET. Each later chunk remains independently authorized. The new [explicit private save](../large-file-save.md) retains original assembly descriptors and exclusively copies to a host-selected new file in a private directory, not another matching inode. The new [authorized delivery call](../large-file-delivery-client.md) now joins exact metadata, ordered chunks, original assembly and selected private saving under one actual-job-held flow lease. An [explicit owned-directory target](../large-file-user-save.md) now allows 0700/0750/0755 with original directory UID/GID/mode continuity, keeping staging strictly 0700. A separate [live delivery observer](../large-file-delivery-progress.md) reports latest per-call phases and actual chunk ACK prefixes, not saved-file confirmation from staged percentages or closure. An optional [cooperative cancellation entry](../large-file-delivery-cancellation.md) stops one call's waiting on an explicit request, not handle drop, real-job termination or automatic cleanup. Explicit [whole-delivery HTTPS checks](../large-file-delivery-https.md) exercise disposable native peers without changing production trust or grant authority; they are not deployed-LAN acceptance. An explicit [distribution router](../large-file-distribution-router.md) now derives both endpoints from one supplied Source/chunk workspace with independent HTTP budgets; it is not a Session issuer or isolated service. User-facing save dialogs, normal-app delivery, resumption and recovery remain absent. [0.5.8 release validation](../../../reports/releases/0.5.8/README.md) stays separate from dated implementation evidence. Unreleased after 0.5.8, [combined Source/HTTPS verification](../large-file-source-https.md) puts current real grants, original bytes, the single router and native TLS selected saving on one test path; it does not mount a service or isolate the Engine.

Cyanrex is becoming a domain-neutral collaboration platform, with eBPF teaching retained as its first
domain rather than the definition of all work. The transition has produced a durable collaboration
preparation layer and a local task-payload editor. It has **not** yet produced a server-backed generic
task workflow for users or replaced the existing teaching runtime. This page describes the whole project now;
dated implementation and verification details are preserved in [Development History](../development-history.md).

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
| 0.5.4 · C2-T preparation | Immutable Ready/zero-progress intent registration and reading, bound to an exact owner and account generation; compatible with separate journal schemas 1 and 2 | Registration/read itself does not dispatch, restore an attempt, authorize retry, install automatically or connect a public/browser API |
| 0.5.4 · C2-U preparation | Consumed live wrapper; confirmed Unknown reservation followed by exact-identity resource/marker commit in a fresh-only schema 2 journal | No whole-draft transaction, schema 1 migration, unjournaled-path enforcement, restart recovery, automatic retry or public/browser connection |
| 0.5.4 · C2-V preparation | Current-Session read of the schema-2 intent and complete recorded step prefix, with exact owner/account filtering | No resource/file read, nonce exposure, checkpoint mutation, recovered wrapper, retry, cleanup or public/browser connection |
| 0.5.4 · C2-W preparation | One recorded resource read plus original full journal/pin recheck and fresh authorization in one transaction | Body-free output still reads files; no receipt, business writes, resumed attempt, retry, cleanup or public/browser connection |
| 0.5.5 · C2-X preparation | Independent private text Artifact HTTP create/exact-read adapter, using current-Session commands and a separate bounded admission budget | Included in 0.5.5 source; no mounted listener, issuer, automatic Task dispatch, browser saving, journal enforcement or retry |
| 0.5.5 · C2-Y preparation | Independent private catalogue Task-content HTTP create/read/replace/status adapter with exact definition references and its own shared admission budget | Not mounted; no client-supplied full definition, provider execution, draft-publication integration, issuer, browser saving or migration |
| 0.5.5 · C2-Z preparation | Independent create-only catalogue draft publication with a frozen complete definition, fixed targets and exact in-memory confirmations | No manual attempt conversion, checkpoint/journal support, recovery, retry, whole-draft transaction, public route or browser saving |
| 0.5.5 · C2-AA preparation | Independent current-Session observation of one unconfirmed catalogue publication target, comparing the frozen complete definition and ordered original bytes | No recovered receipt, state adoption, prefix audit, checkpoint/journal conversion, retry, public route or browser saving |
| 0.5.5 · Task decoupling | Provider-free definition catalogue with bounded multi-package composition, complete or explicitly absent assessment bindings, portable language hints independent of editor profiles, and explicit catalogue/text-content composition | No half-bound/default policies, evaluator discovery, new payload kind, public API, migration or browser saving |
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
| 0.5.6 · private metadata index preparation | Independent bounded schema 3 metadata listing with strict lookahead and current Session owner filtering | No public task-list edge, content-byte proof, browser reopening, installer, migration or recovery; original content modes remain unchanged |
| 0.5.6 · private metadata HTTP preparation | Explicit standalone GET adapter over the existing index, with strict positional query parsing and response-lifetime admission | Not mounted; no issuer, live OpenAPI/SDK/browser list, content-byte proof, migration or recovery |
| 0.5.6 · large-file core preparation | Independent strict manifest, ordered chunk/whole-file checks, bounded stream copy and preopened Unix descriptor checks | No durable staging, quota/work admission, publication/grants, resume, network/client or desktop pairing |
| 0.5.6 · Unix chunk staging preparation | Full manifest/index-bound exclusive private chunks, sync/readback and clone-shared worker admission retained through queued/running jobs | Trusted-only; no upload journal, reserved quota, resume, publication/grants, cleanup, transport/client or desktop pairing |
| 0.5.6 · upload intent preparation | Separate trusted PostgreSQL immutable reservations, persisted logical limits and bounded full-ledger validation | No chunk confirmation, physical allocation, release/cleanup, current Session, resume, publication, network/client or desktop pairing |
| 0.5.6 · upload-bound staging preparation | Original source/reservation pins, one-shot in-memory attempts and original-file local receipts | No durable chunk journal, Session authority, recovery/retry, publication, transport or live/client connection |
| 0.5.6 · chunk journal preparation | Independent fresh namespace, acknowledged Unknown before file dispatch and exact recorded confirmations | No recovered attempt, current-byte proof from status, Session authority, retry, publication, transport or live/client connection |
| 0.5.6 · whole-upload verification preparation | Ordered current bound chunk/whole digests and original source/file snapshot rechecks | Read-only metadata observation; no historical receipt, journal-state adoption, durable publication, recovery, authority or transport |
| 0.5.6 · file publication and private metadata preparation | Separate trusted publication after confirmed journal/fresh bytes; independent current-Session own reservations and upload/Published metadata reads with original pins and final policy/expiry checks | No actual Session publication, publisher/recipient grants, physical sealing, recovery or transport; a trusted publisher may remain Unknown with a stored row |
| 0.5.6 · file grant contract preparation | Pure exact workspace, account-generation, file-version, state and validity matching | No authenticated issuer, persisted grant, fresh policy/revocation lookup, Session integration or byte-access authority |

Product version, API compatibility baseline and individual storage schema versions are independent.
The source version is 0.5.8 and the public API compatibility baseline remains 0.3.0. The current Task
store requires schema 2 in a fresh dedicated namespace and rejects schema 1 without migration. Neither
the normal startup path nor this source release upgrades a database.
The content store included in 0.5.0 separately requires schema 3; the schema 2 handle and existing Session
adapters reject it. Both formats require their own explicitly installed fresh namespace.
Prepared authentication now separately requires source schema 2 for atomic OTP consumption. Its
installer and readers reject existing source schema 1, including empty instances, without migration.

## Whole project capability map

The [capability tensor](../capability-maturity.md) adds architecture/function/implementation coordinates,
four independent maturity scores, evidence references and missing edges. These source-review judgments
do not supersede the dated test records below or turn prepared components into a connected product.

“Existing workflow” means the current application implements the path, not that a particular installed
instance or newly built release artifact has been accepted. “Preparation layer” means tested Rust
contracts/services that are not connected to the live application's authentication and storage.

| Module | Available implementation | Remaining boundary |
|---|---|---|
| Application shell and task editing | Existing four-language navigation and action confirmations; local task draft with optional text payload and 14 editor capability profiles; 0.5.5 valid extension hints retain plain-text editing | No server save, browser persistence, autosave, binary attachment or generic task list |
| Live identity and classroom access | Password/TOTP, cookie Sessions, CSRF checks, teacher management, student guards and link-based invitation onboarding | Still the existing instance authority; new Workspace ownership is not a deployment grant |
| Collaboration identity and policy | Stable identities, audited membership/deployment policy, durable Sessions, lifecycle commands, explicit provisioning and read-only reconciliation | No live AuthService cutover, reviewed data migration or complete recovery flow |
| Domain definitions | 0.5.5 provider-free TaskDefinitionCatalog with explicit bounded multi-domain/package-version composition and an explicit choice of assessment binding; typed TaskCatalog keeps fully bound single-provider assessment; the five teaching labs use the eBPF pack | Catalogue metadata admission is not typed input validation, policy execution or dynamic provider discovery |
| Task instances | Private manual/catalogue tasks, fixed definitions, revision-fenced states, atomic task/outbox records; Draft input replacement included in 0.4.9 | No mounted public task API, cross-user assignment, dependencies or acceptance policy |
| Artifact content | Private immutable revision files, exact digest-bound references, transactional metadata/events, Session-authorized publication/reads and 0.5.5 C2-X text HTTP preparation | Text router remains unmounted; no UI metadata mapping, public content workflow, retention/garbage collection or cross-user sharing |
| Large-file transfer | Independent [stream core](../large-file-core.md), [private staging](../large-file-staging.md), [logical quotas](../large-file-upload-intent.md), [local receipts](../large-file-upload-staging.md), [chunk journal](../large-file-chunk-journal.md), [whole-byte verification](../large-file-whole-verification.md), [publication metadata](../large-file-publication.md), [Session private metadata](../large-file-session-metadata.md) and [pure grant selectors](../large-file-access-contract.md) | Central distribution and later desktop transfers still need reviewed receipt-loss/restart reconciliation, physical disk policy, current audited publisher/recipient grants, transport and safe client saves |
| Task content metadata | C2-K contracts, C2-L storage and C2-M Session/text composition; 0.5.5 explicit manual/catalogue modes; separate C2-N manual and C2-Y catalogue HTTP adapters, C2-Z catalogue publication and C2-AA target observation | HTTP adapters remain unmounted; catalogue publication/observation has no manual checkpoint/journal composition; no secure login/installation workflow, browser saving or migration |
| Whole-draft publication mapping | 0.5.3 C2-O strictly parses portable draft JSON, prepares text publication values and binds supplied exact content | Pure values only; no dispatch, confirmed publication history, current authorization or persisted draft |
| Review | Immutable revision history; Session-authorized private human opinions on exact Artifact targets/evidence included in 0.4.9 | No cross-user reviewer grants, generic Task approval, automatic rule execution or AI review |
| Teaching and eBPF workbench | Five labs, attempts/progress, teacher feedback, historical resume, Clang assistance, bpftool and supported Aya tracepoint execution | Remains a separate compatible workflow; old attempts have not been migrated into generic Tasks/Runs/Reviews |
| Execution and resources | Local leases/quotas/timeouts; signed remote Agent registration, jobs, probes and optional compile-only diagnostics | Local kernel remains shared; no durable generic Run service, remote eBPF loading or VM lifecycle |
| Events and persistence | Owner-scoped telemetry, bounded history/reconnect recovery and service-specific persistence; preparation stores commit their own outbox records | No generic durable business-event dispatcher, cursor replay or multi-Engine coordination |
| Extensions and integrations | State-only module manifests, structured teacher commands, generated OpenAPI/internal SDK; 0.5.3 AI connection metadata and explicit SDK tool adapters | No provider transport, executable plugin runtime, AI Agent planning/delegation or implemented Viento/Lese/Nuis integration |
| Operations and distribution | Linux/WSL2/Docker paths, explicit SSH management of preinstalled packages, offline tooling and artifact verification | No automatic bare-host bootstrap, deployment cutover or current-candidate LAN/kernel/artifact acceptance claim |

The [system architecture](../architecture.md) explains ownership and trust boundaries. The
[current platform network](../platform-network.md) distinguishes connected workflows from unfinished links;
the older functional/testing maps retain their original teaching-era evidence.

## Task content is not a code requirement

A Task describes work. Its optional payload may contain notes, code or configuration; editing one
text item does not turn that Task into a Run. The new `/tasks/new` page owns the draft, and `/editor`
opens the same container. The original `/ebpf` workbench remains separate.

The local draft supports up to 32 text items, at most 256 KiB each, with bounded whole-draft JSON
import/export. Filename, language and text belong to the parent draft. Confirmations and revision
checks prevent obsolete imports or callbacks from replacing newer work. Drafts live only in page
memory; a download is a local export, not a server save. See [Task payload editing](../editor.md).
The 0.5.5 draft accepts bounded ASCII language hints independently of the 14 installed profiles.
Unknown hints remain unchanged through import/export and use plain-text editing with no language tools;
resolving editor capabilities is not a payload edit or automatic language rewrite.

JS/TS provide browser-local semantic assistance; JSON/HTML/CSS use their configured local providers.
Rust, Python, C/C++ and the other basic profiles provide highlighting/snippets, not external language
servers. There is no rust-analyzer, Pyright, clangd, project filesystem or LSP transport. Language
feedback is neither execution nor assessment, and shared browser workers are not a security sandbox.

The server has a different identity contract: exact owned Artifact revisions, current Session
authorization and a Task revision check. [Draft replacement](../session-task-revisions.md) validates old
and new content around the task/outbox write while preserving prior content and Reviews. Artifact
publication remains a separate operation. Filenames/languages, title changes, import semantics and
browser save/conflict handling still need an explicit public contract; local IDs cannot supply one.
The [C2-K manifest](../task-content-manifest.md), included in 0.5.0, defines metadata and pure consistency
checks. [C2-L](../task-content-store.md) adds independent trusted storage and atomic edits, without
changing existing schema 2 records or Session commands. [C2-M](../session-task-content.md) now composes
current Session and exact Artifact-text checks internally. The authorized browser workflow remains absent.
The 0.5.5 explicit catalogue mode adds exact frozen definitions to this text composition, keeping
title/filename/language labels independent of definition metadata and preserving revision/byte checks.
Its distinct workspace cannot enter the existing manual HTTP or draft-publication paths.
The independent 0.5.5 [C2-Z publisher](../session-catalog-task-draft-publication.md) uses that
catalogue workspace without converting it into a manual attempt. It fixes the complete exact definition,
publishes one Artifact per step, then creates a catalogue Task through the existing Session command.
Confirmed prefixes remain on failure; this is not whole-draft atomicity, checkpoint recovery or browser saving.
The separate [C2-AA observer](../session-catalog-task-draft-observation.md) reads that surviving attempt's
one unconfirmed target through existing current-Session commands. It compares the complete frozen
definition and ordered original content without adopting a receipt or changing the stopped state.
[C2-N](../task-content-http.md) adds a standalone HTTP boundary but does not mount it in the existing
application, issue credentials, publish Artifacts or connect the browser.
The 0.5.5 [C2-X](../text-artifact-http.md) router separately publishes one bounded text Artifact
and reads its exact revision. Its returned pin can feed a separate C2-N request; the routers do not
call or merge each other, and the sequence is not atomic or a connected browser save workflow.

0.5.3 [C2-O](../task-draft-publication.md) supplies the pure import and publication-plan mapping:
blank titles are rejected without inventing text; checked local identities are discarded. Exact
supplied content may be bound to a manifest, but matching bytes do not prove publication or authorize
a later command. In that release, the server accepted valid extension language hints while the
browser still restricted them to installed profiles; the 0.5.5 local editor now preserves those
valid hints separately from its capabilities. The pure importer itself dispatches no publication; C2-P supplies separate internal
composition, while browser saving remains absent.

0.5.3 [C2-P](../session-task-draft-publication.md) separately composes current-Session publication
and C2-M Task creation one write at a time. A fixed caller-owned attempt retains confirmed progress
and stops at an unconfirmed step after dispatch failure or cancellation. It supplies no persistent
receipt, automatic recovery or browser/public connection; each command still reauthorizes independently.

0.5.3 [C2-Q](../session-task-draft-observation.md) separately observes a fixed unconfirmed target
with the original Session. Matching data does not confirm the earlier write; invisibility does not
prove rollback. Observation leaves the attempt stopped and provides no new-login or restart recovery.

0.5.3 [C2-R](../session-task-draft-checkpoint.md) exports a 64 KiB-bounded metadata checkpoint without
text or authentication material. Parsing validates structure and consistency, not the reported history.
It performs no save or read and cannot restore C2-P or invoke C2-Q; durable outcomes remain separate.

0.5.3 [C2-S](../session-task-draft-inspection.md) adds a separate explicit read of one checkpoint
target under supplied current authority, without needing the original attempt. Matching metadata is
not original-byte or historical-caller proof; parsing still does no I/O, and C2-Q's original Session
requirement remains unchanged. No confirmation, journal, state restoration or retry follows.

0.5.4 [C2-T](../session-task-draft-intent.md) separately registers a real unstarted attempt's
canonical metadata under current Session authorization. A new Session may read the record only for
the same owner and exact account generation. The configured journal installation ID and namespace
names do not prove original resource incarnations or publication history. C2-P `advance` still needs
no journal: registration alone is not durable-before-dispatch execution, recovery or retry permission.

0.5.4 [C2-U](../session-task-draft-dispatch.md) consumes that pristine attempt into a separate
journaled wrapper. Each advance first confirms an Unknown reservation, then reauthorizes and couples
the exact step marker with one Artifact or Task transaction. Only confirmed success advances memory;
uncertain outcomes stop it. The still-owned wrapper can complete a multi-item draft through repeated
successful steps, but intent/checkpoint reads cannot reconstruct it after drop or process loss.

0.5.4 [C2-V](../session-task-draft-journal-inspection.md) separately reports recorded committed
steps, including the final Task step, and an optional Unknown target. A fresh Session must match the
intent's exact owner/account binding. The reader verifies the full prefix and current transaction
boundaries, not resource contents or publication history; absence is not rollback or retry evidence.
Its validation is separate from the historical C2-T/U records below.

0.5.4 [C2-W](../session-task-draft-journal-observation.md) separately selects a recorded ordinal,
reads its exact Artifact revision or current Task and referenced contents, then rechecks the original
full journal and first pins before returning. Recorded Unknown/Committed and current metadata agreement
are separate values, neither a receipt nor a recovery permit. Its execution evidence is independent;
C2-V continues to read no resources or files.

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

<!-- original-project-status:end -->
