# Project Status

**2026-10-16, included in 0.6.2:** [bound Session connections](platform-session-connection.md) combine retained storage with query/logout and explicit HTTP state; [format 3 diagnostics](platform-marker-expectation.md) require original catalog/marker declarations before row reads. Neither starts a host/issuer or connects browser saving. Earlier snapshots below retain their dated scope.

Snapshot date: **2026-10-13**. Source version: **0.5.9**, adding reviewed local multi-file text import, complete five-language UI including Korean, bilingual guides, architecture/error-boundary cleanup and Source/HTTPS test coverage. Private task indexing and explicit file delivery remain included but unmounted; normal-app distribution, server-backed task saving and resumption remain absent. Earlier task decoupling, AI host adapters and authentication protections remain in place.

0.5.4 C2-T–W retains immutable intents, explicit journaled dispatch, private journal inspection and one-resource observation. The original C2-P path stays independent; metadata agreement is not a receipt,
restart recovery or retry permission. 0.5.5 adds shared HTTP admission and separate C2-X/Y text/catalogue
routers, both unmounted; C2-Z/AA independently publishes and observes a frozen catalogue draft without
entering the manual checkpoint/journal workflow. Neither connects browser saving or automatic assessment.
0.5.5 also separates frozen definitions from providers and portable language hints from editor profiles,
composes multi-domain/package catalogues within 256 definitions, and allows explicitly absent assessment
bindings. Catalogue/text content needs neither assessment nor a Run; typed assessment still requires both pins.

The October 11 [private metadata index](task-content-index.md), **included in 0.5.6**, lists bounded
owner-private schema 3 pages through trusted storage/current Session authorization. Explicit neutral
catalogue configuration preserves the old content modes. Its separate [HTTP adapter](task-content-index-http.md)
remains unmounted; no live HTTP/SDK/UI list or server-backed browser reopening is connected.

The same-day **[large-file core](large-file-core.md), included in 0.5.6**, following
[proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), adds transport-neutral manifests
and bounded synchronous IO, with centralized distribution prioritized over later desktop transfers.
It verifies supplied real bytes, not storage, publication, recipients, durable progress or delivery.
The separate [Unix chunk-staging follow-up](large-file-staging.md) adds exclusive private files,
sync/readback and an independent job-lifetime worker budget. Neither layer enables network transfer,
durable upload progress, disk reservations, publication or live authentication/client connections.
The separate [upload-intent ledger](large-file-upload-intent.md) adds immutable trusted metadata and
atomic logical quotas, not confirmed chunks, physical disk allocation, Session authority or transfer.
The [local upload adapter](large-file-upload-staging.md) adds original-file receipts; the independent [write-ahead journal](large-file-chunk-journal.md) records chunks, while [whole-byte verification](large-file-whole-verification.md) remains a current observation. Separate [persistent publication metadata](large-file-publication.md) combines complete confirmed chunks and fresh bytes. Separate [Session private metadata](large-file-session-metadata.md) derives owner/account for own reservations and upload/Published reads. Separate [pure publisher/recipient contracts](large-file-access-contract.md) match supplied exact accounts/versions, state and validity; they do not establish current audited permission. Recovery, durable grants and transport remain separate.
The [Unreleased publisher-grant ledger](large-file-publisher-grants.md), outside the 0.5.6 tag, adds
trusted revocable records and own reads. Later [Session management](large-file-grant-management.md) derives actors from an exact host-configured manager/account; no inherited roles, durable online manager revocation or publication consumption. Separate evidence does not refresh historical records.
Independent [Session publication](large-file-session-publication.md) now couples current exact publisher permission to real-byte verification and a final authorized source insert; recipients, transport, recovery and live mounting remain absent. Its new evidence stays in its own record.
Later [recipient grants](large-file-recipient-grants.md) add an independent exact-file ledger and current own-recipient metadata reader. October 12 [publisher-owned sharing](large-file-sharing.md) adds authenticated own-Published issuance and independent owner revocation; [download-manifest observation](large-file-download-manifest.md) checks exact Published metadata, not later byte authority. Separate [verified chunk snapshots](large-file-chunk-read.md) connect up to 4 MiB of private bytes between original-pinned source transactions. The independent [binary HTTP adapter](large-file-chunk-http.md) adds unmounted exact GET, byte-owner retention of both budgets and a ten-second first-frame window. A separate [HTTP receiver](large-file-chunk-receiver.md) checks supplied responses through explicit EOF/hash before private exclusive staging, retaining buffer/control through actual save work. An explicit [single-chunk client](large-file-chunk-client.md) adds fixed trusted HTTPS/opt-in literal-loopback requests with job-owned admission. Separate [private whole-file assembly](large-file-chunk-assembly.md) checks legacy inputs and an exclusive actual whole output with original-inode readback. Included in 0.5.8, [ordered private download](large-file-download-client.md) now joins sequential requests/stage ACKs to same-root assembly with real-job flow capacity; first failure/drop stops continuation. A separate [manifest HTTP adapter](large-file-manifest-http.md) now projects current authorized metadata; the new [exact manifest client](large-file-manifest-client.md) fetches the complete expected version and can compose metadata with ordered private download; even empty granted files require GET. Each later chunk remains independently authorized. The new [explicit private save](large-file-save.md) retains original assembly descriptors and exclusively copies to a host-selected new file in a private directory, not another matching inode. The new [authorized delivery call](large-file-delivery-client.md) now joins exact metadata, ordered chunks, original assembly and selected private saving under one actual-job-held flow lease. An [explicit owned-directory target](large-file-user-save.md) now allows 0700/0750/0755 with original directory UID/GID/mode continuity, keeping staging strictly 0700. A separate [live delivery observer](large-file-delivery-progress.md) reports latest per-call phases and actual chunk ACK prefixes, not saved-file confirmation from staged percentages or closure. An optional [cooperative cancellation entry](large-file-delivery-cancellation.md) stops one call's waiting on an explicit request, not handle drop, real-job termination or automatic cleanup. Explicit [whole-delivery HTTPS checks](large-file-delivery-https.md) exercise disposable native peers without changing production trust or grant authority; they are not deployed-LAN acceptance. An explicit [distribution router](large-file-distribution-router.md) now derives both endpoints from one supplied Source/chunk workspace with independent HTTP budgets; it is not a Session issuer or isolated service. User-facing save dialogs, normal-app delivery, resumption and recovery remain absent. [0.5.8 release validation](../../reports/releases/0.5.8/README.md) stays separate from dated implementation evidence. Unreleased after 0.5.8, [combined Source/HTTPS verification](large-file-source-https.md) puts current real grants, original bytes, the single router and native TLS selected saving on one test path; it does not mount a service or isolate the Engine.

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

The [capability tensor](capability-maturity.md) adds architecture/function/implementation coordinates,
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
| Large-file transfer | Independent [stream core](large-file-core.md), [private staging](large-file-staging.md), [logical quotas](large-file-upload-intent.md), [local receipts](large-file-upload-staging.md), [chunk journal](large-file-chunk-journal.md), [whole-byte verification](large-file-whole-verification.md), [publication metadata](large-file-publication.md), [Session private metadata](large-file-session-metadata.md) and [pure grant selectors](large-file-access-contract.md) | Central distribution and later desktop transfers still need reviewed receipt-loss/restart reconciliation, physical disk policy, current audited publisher/recipient grants, transport and safe client saves |
| Task content metadata | C2-K contracts, C2-L storage and C2-M Session/text composition; 0.5.5 explicit manual/catalogue modes; separate C2-N manual and C2-Y catalogue HTTP adapters, C2-Z catalogue publication and C2-AA target observation | HTTP adapters remain unmounted; catalogue publication/observation has no manual checkpoint/journal composition; no secure login/installation workflow, browser saving or migration |
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
The 0.5.5 draft accepts bounded ASCII language hints independently of the 14 installed profiles.
Unknown hints remain unchanged through import/export and use plain-text editing with no language tools;
resolving editor capabilities is not a payload edit or automatic language rewrite.

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
The 0.5.5 explicit catalogue mode adds exact frozen definitions to this text composition, keeping
title/filename/language labels independent of definition metadata and preserving revision/byte checks.
Its distinct workspace cannot enter the existing manual HTTP or draft-publication paths.
The independent 0.5.5 [C2-Z publisher](session-catalog-task-draft-publication.md) uses that
catalogue workspace without converting it into a manual attempt. It fixes the complete exact definition,
publishes one Artifact per step, then creates a catalogue Task through the existing Session command.
Confirmed prefixes remain on failure; this is not whole-draft atomicity, checkpoint recovery or browser saving.
The separate [C2-AA observer](session-catalog-task-draft-observation.md) reads that surviving attempt's
one unconfirmed target through existing current-Session commands. It compares the complete frozen
definition and ordered original content without adopting a receipt or changing the stopped state.
[C2-N](task-content-http.md) adds a standalone HTTP boundary but does not mount it in the existing
application, issue credentials, publish Artifacts or connect the browser.
The 0.5.5 [C2-X](text-artifact-http.md) router separately publishes one bounded text Artifact
and reads its exact revision. Its returned pin can feed a separate C2-N request; the routers do not
call or merge each other, and the sequence is not atomic or a connected browser save workflow.

0.5.3 [C2-O](task-draft-publication.md) supplies the pure import and publication-plan mapping:
blank titles are rejected without inventing text; checked local identities are discarded. Exact
supplied content may be bound to a manifest, but matching bytes do not prove publication or authorize
a later command. In that release, the server accepted valid extension language hints while the
browser still restricted them to installed profiles; the 0.5.5 local editor now preserves those
valid hints separately from its capabilities. The pure importer itself dispatches no publication; C2-P supplies separate internal
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

0.5.4 [C2-T](session-task-draft-intent.md) separately registers a real unstarted attempt's
canonical metadata under current Session authorization. A new Session may read the record only for
the same owner and exact account generation. The configured journal installation ID and namespace
names do not prove original resource incarnations or publication history. C2-P `advance` still needs
no journal: registration alone is not durable-before-dispatch execution, recovery or retry permission.

0.5.4 [C2-U](session-task-draft-dispatch.md) consumes that pristine attempt into a separate
journaled wrapper. Each advance first confirms an Unknown reservation, then reauthorizes and couples
the exact step marker with one Artifact or Task transaction. Only confirmed success advances memory;
uncertain outcomes stop it. The still-owned wrapper can complete a multi-item draft through repeated
successful steps, but intent/checkpoint reads cannot reconstruct it after drop or process loss.

0.5.4 [C2-V](session-task-draft-journal-inspection.md) separately reports recorded committed
steps, including the final Task step, and an optional Unknown target. A fresh Session must match the
intent's exact owner/account binding. The reader verifies the full prefix and current transaction
boundaries, not resource contents or publication history; absence is not rollback or retry evidence.
Its validation is separate from the historical C2-T/U records below.

0.5.4 [C2-W](session-task-draft-journal-observation.md) separately selects a recorded ordinal,
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

## Verification recorded so far

The C2-T–W changes and Next.js update are included in 0.5.4. Their dated implementation records below
retain their original 0.5.3 workspace version, Unreleased status, counts and failure history; those
records are not a new 0.5.4 release gate or evidence of publication or deployment.

The catalogue/editor/shared-admission follow-ups are now included in 0.5.5. Their dated 0.5.4
workspace records retain their original Unreleased labels, counts and SQL scope.

### 0.5.6 version validation · 2026-10-11

`bash scripts/quality-gate.sh --no-npm-install` passed for source **0.5.6**.
Default Rust: **832 passed, 0 failed; 742 ignored**, **75 result blocks**, no compiler warnings.
Common scripts: **248 passed**. Frontend: production build, full typecheck and **208 tests** passed.
SDK: generated/API/surface checks, build, **39 tests**, type tests and **3 consumer package tests** passed.
Fresh frontend and SDK production npm audits both reported **zero vulnerabilities**.
Version, course, OpenAPI, formatting and tooling checks passed; public API baseline remains **0.3.0**.
Frontend Node module-type warnings remain; this gate does not claim warning-free JavaScript execution.
Index/file preparation is included in 0.5.6; earlier 0.5.5/Unreleased execution records retain their scope.
Not rerun: opt-in SQL/file cases, real-kernel/distribution artifact acceptance, browser/LAN or deployment.
No preparation API mounting, network transfer, durable grant issuance, recovery or live-auth cutover.
Evidence: `/tmp/cyanrex-release-056-vh5118WO/quality-gate.log` (local log, not a release artifact).

### Unreleased upload-bound local receipts · 2026-10-11

Source **0.5.5**, outside the tag; [original upload/file pins and one-shot attempts](large-file-upload-staging.md).
Compiled Red: **0 passes / 1 expected failure**, seven temporary fixture warnings; Green **3 default / 9 SQL/file cases**.
Adjacent intent regression: **12 exact SQL cases**; full gate **798 default Rust / 229 common**, **700 ignored**, **70 result blocks**, no warnings.
Real lock-wait drift, same-byte inode replacement and running-worker cancellation are covered, not durable progress, Session, resume or transport.
All **102 prior dated blocks** and tensor evidence remain unchanged; detailed scope and the initial CI length-guard failure are in the linked record.
Private PostgreSQL stopped; **47 MiB** cluster and **3 MiB** regenerable cache removed, logs retained under `/tmp/cyanrex-upload-bound-wQ9u2Poq`.
No version, commit, push, release or deployment change; no disk-fault, power-loss, benchmark, browser, TLS/LAN or desktop acceptance.

### Unreleased durable upload intent and logical quotas · 2026-10-11

Source **0.5.5**, outside the tag. The separate [upload ledger](large-file-upload-intent.md) adds
trusted immutable owner/account-generation reservations, persisted logical limits, serialized
admission and bounded full-ledger post-write checks. It does not connect chunks or current Session.
Compiled Red: **0 passes / 1 expected behavior failure**, with six temporary unused-fixture warnings.
Green: **5 default cases / 12 exact real PostgreSQL cases** pass; the 64 GiB fixture is metadata only.
Full backend gate passes **795 default Rust / 225 common checks**, **691 opt-in cases ignored**,
**69 Rust result blocks**, no compiler warnings; tool fixtures are not live acceptance.
No lost-commit-ack, filesystem quota/fault/power-loss, benchmark, browser/SDK, Session, TLS/LAN,
deployment or desktop acceptance; earlier index SQL was not rerun. Details and early guard failures
are recorded in the upload document. All prior dated blocks and capability-tensor evidence remain unchanged.
The private PostgreSQL 16 instance had no TCP listener; every upload schema was dropped. Its stopped
47 MiB cluster was removed; regenerable cache/empty fixtures were cleaned, with logs retained under
`/tmp/cyanrex-upload-intent-7iIyc9OE`. No release/version, commit, push or deployment change.

### Unreleased private Unix chunk staging · 2026-10-11

Source **0.5.5**, outside the published tag. The separate [staging API](large-file-staging.md)
binds exclusive entries to the entire typed manifest/index, pins an existing private root, verifies
exact bytes, synchronizes file/directory and reads back the originally created inode. Clone-shared
worker permits belong to actual queued/running jobs; independent handles have additive budgets.
Existing entries conflict; uncertain output remains without overwrite, adoption, retry or cleanup.

The compiled baseline first ran **0 passes / 2 expected behavior failures**. Green passed **22
default integration cases** (two baseline plus twenty extensions) and **one private queued-worker
case**. Fixtures include a real 4 MiB + 17-byte file, complete-manifest aliases, retained corrupt/
short/long output, private-root/entry drift, FIFO with no writer, actual same-byte/different-inode
substitution retaining the original single-link inode, and running cancellation/timeout barriers.
Manual future polling with one blocking thread proves queued cancellation without a timing guess.

The backend-only gate passed **790 default Rust cases and 221 common script checks**, with **679
opt-in cases ignored**, 68 Rust result blocks and no compiler warnings. Formatting, file lengths,
version/API/SDK drift, generated course copies and synthetic tooling passed. All **98 preexisting
bilingual verification blocks** and the capability tensor were preserved without changing evidence.

Inherited deployment/database settings were removed. No real SQL, frontend/browser, SDK runtime,
advisory scan, disk-full/fsync fault, power-loss recovery, RSS/throughput, TLS/LAN, desktop or deployment
acceptance was rerun. Synthetic fixtures/cache were removed and logs retained. No upload journal,
reserved quota, durable progress/resume, publication/grants, transport/client or live composition
was added; current byte verification is not a historical receipt. No commit, push or version bump.

### Unreleased large-file manifest and streaming core · 2026-10-11

Source version **0.5.5**, outside the published tag. Following
[proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2), the independent
[core](large-file-core.md) implements typed file/version metadata, strict bounded decoding,
fixed-buffer inspection, exact chunk copying and ordered chunk/whole-file verification. Unix
preopened-file observations check source metadata, not paths, private roots or immutable storage.

Two behavior tests first compiled and ran with **0 passes / 2 expected failures** against the
replacement stubs. The completed target passed **37 default cases**, plus **two private Unix
snapshot-boundary cases** in the library. Fixtures include a real **4 MiB + 17-byte** file, independent
byte/hash oracles, short reads/writes, partial unverified output, EOF faults, hostile IO counts,
strict/duplicate metadata and a sparse over-limit descriptor rejected before reading. The real
same-length mutation test exercises the snapshot helper, not a concurrent in-read race.

The completed backend-only quality gate passed **767 default Rust cases and 220 common script
checks**, with **679 opt-in cases ignored**, 67 Rust result blocks and no compile warnings.
Formatting, file lengths, version/API/SDK drift, course copies and synthetic tooling passed.
Two earlier four-case inventory runs each had 3 passes/1 failure: an unfinished test-file inventory
and a wrong CI-command assumption in the new guard. Its corrected checks passed before the gate.
All **96 preexisting bilingual verification blocks** and the capability tensor remained unchanged.

Inherited deployment/database settings were removed. No PostgreSQL, frontend/browser, SDK runtime,
advisory scan, actual 64 GiB read, RSS/throughput benchmark, TLS/LAN, desktop or deployment acceptance
was rerun. Fixtures were removed; logs were retained. Copy success is not flush/fsync, publication
or delivery. Durable staging, worker/quota admission, resume, publisher/recipient authorization,
transport/client wiring and desktop pairing remain absent. No commit, push or version bump occurred.

### Unreleased private task metadata HTTP · 2026-10-11

Source version **0.5.5**, outside the published tag. The separate
[HTTP contract](task-content-index-http.md) projects one existing current-Session index command at
exact `GET /platform/v1/task-index`. Strict raw `after`/`limit` parsing grants no owner or namespace
selection. Dedicated credentials, empty-body checks and shared response-lifetime admission remain
independent of the old manual/catalogue collection contracts and live application.

The closed-source dispatch test first compiled and failed with **400 versus expected 503**,
then passed after the real command was connected. A temporary unused fixture import was corrected
without suppressing warnings. Review also strengthened browser-entry scanning and exact response
envelope/null assertions; all eight new SQL cases passed again after that assertion change.

Fresh backend verification passed **728 default Rust tests** and **216 common tests**, with
**66 successful Rust result blocks and zero compilation warnings**; **679 ignored Rust cases
are not passes**. The isolated PostgreSQL runners passed **51 distinct cases**: new index HTTP 8,
existing trusted/Session index 12, manual content HTTP 13, catalogue content HTTP 8 and text Artifact
HTTP 10. Real blockers and database expiry verify cancellation/reauthorization; snapshots protect
source/resource rows and file bytes/modes. The response is metadata, not content availability or a receipt.

The disposable database had no remaining fixture namespaces or clients before shutdown. Its temporary
cluster and synthetic files were removed; logs remain. Bilingual contracts and course copies are
synchronized. All **94 prior status sections** and the tensor snapshot/topology/scores/evidence remain
unchanged. No HTTP mounting, issuer, live OpenAPI/SDK/UI list, installation, migration or recovery was
added. Frontend/SDK runtime, cross-target builds, large-list benchmarks, remote CI, kernel, SSH/LAN
and deployment acceptance were not rerun. No version bump, commit or push is part of this slice.

### Unreleased owner-private task metadata index · 2026-10-11

Source version **0.5.5**, outside the published tag. The independent
[index contract](task-content-index.md) adds metadata-only trusted and current-Session pages:
default 20, limits 1–32, UUID keyset order, exact-owner filtering and validated lookahead.
Explicit neutral catalogue configuration admits manual and complete configured bound/unbound
definitions; the original content read/write modes and storage schemas remain unchanged.

Two closed-source tests first compiled and failed on placeholder results, then passed after real
dispatch. A later exact SQL test exposed missing content columns returning an empty success for
an absent owner. It failed with `Ok(empty)` versus `StorageUnavailable`, then passed after fixed
zero-row Task/outbox column checks. No other owner's rows are decoded for those probes.

Fresh verification passed **714 default Rust tests**, **212 common tests** and the backend gate;
**671 ignored Rust cases are not passes**, with **65 successful Rust result blocks and no warnings**.
The explicit isolated PostgreSQL runners passed **86 distinct cases**: new trusted index 4, new
Session index 8, existing content store 21, Session content 33 and catalogue tasks 20. After the
empty-page fix and bounded test-loop hardening, all **12 new index cases** passed again through
the exact runner. Real blockers and database time verify expiry/cancellation, not timing guesses.

The disposable database had no remaining fixture namespaces/clients before shutdown; the failed
test's synthetic schema and temporary cluster were removed, with logs retained. Current bilingual
contracts and course copies are synchronized. All **92 prior status sections** and the tensor's
snapshot, topology, scores and evidence remain unchanged. This slice adds no public HTTP/SDK/UI,
content-byte proof, installation, migration or recovery. Frontend/SDK runtime, cross-target builds,
large-inventory benchmarks, remote CI, kernel, SSH/LAN and deployment acceptance were not rerun.

### 0.5.5 version validation · 2026-10-11

Version metadata, current documentation and generated course copies are synchronized at **0.5.5**.
The fresh full local gate (`--no-npm-install`, unchanged verified dependency graphs) passed
**706 default Rust, 208 common, 208 frontend and 42 SDK tests**; **659 ignored Rust cases** are
not passes. Rust produced **63 successful result blocks** without compilation warnings.
Frontend/SDK production dependency audits each reported zero vulnerabilities; the existing Node
`MODULE_TYPELESS_PACKAGE_JSON` warnings remain.

The production-build browser fixtures additionally passed **23 task-payload and 18 multi-language
editor cases** with real pages/Monaco and mocked Engine reads. These verify local editing, not server
saving. This release validation does not rerun opt-in PostgreSQL, kernel, SSH/LAN, remote CI, Rust
advisory or deployed restart/recovery acceptance. Prior dated SQL records and the tensor snapshot,
scores and evidence remain unchanged. Source publication does not mount prepared APIs, migrate a
database or update a running deployment.

### Unreleased catalogue publication target observation · 2026-10-11

Source version remains **0.5.4**, outside the published tag. Independent C2-AA adds
`observe_unconfirmed(&self, token)` to the still-owned catalogue publication attempt. Ready and
Complete return `NoUnconfirmedStep` without I/O; an unconfirmed attempt rejects any token other
than its original canonical Session before dispatch. A matching fingerprint is continuity, not
authorization. One existing current-Session command reads the fixed exact Artifact revision or
the current catalogue Task and its ordered contents, retaining existing final authority checks.

The body-free report uses the existing generic step/outcome values, not a manual attempt or a
checkpoint/journal conversion. Matching requires full frozen `Some(definition)`, original manifest,
Draft revision 1 and all ordered original bytes. Another registered complete definition or a legal
later edit can differ. Same-reference metadata drift, manual-mode records and damaged bytes remain
typed reader errors; they are not converted into normal differences or invisibility.

An exact historical Artifact revision may match after its head changes. The observer does not scan
earlier confirmations. A missing Task read verifies source and Task scope without entering Artifact
namespace validation, while an existing empty Task does validate that namespace. Read locks and
commit boundaries remain those of the original commands, not SQL READ ONLY or a whole-workflow
transaction. Results may become stale; NotVisible proves neither rollback nor permanent absence.
Success, error and cancellation preserve the complete definition, target, stopped state and original
confirmation prefix. Even a later matching resource is not adopted as a receipt and cannot resume
`advance`.

TDD first compiled and executed one meaningful Red: the stub returned `NoUnconfirmedStep` instead
of the real closed-pool Artifact reader's typed storage error. The implemented reader then passed
that same exact case without warnings. This was an executed behavioral failure and subsequent Green,
not a fixture, missing-method or compilation failure.

The expanded public target initially passed four cases with one unused shared-fixture reexport
warning. A narrowly scoped allowance on that reused test-fixture module removed the warning without
changing the old fixture. Final public execution passed **four default cases** with **eight SQL cases
ignored**, and private comparison/admission execution passed **six cases**, without warnings. The six
new structural guards also passed. These scoped runs are not additive to a complete backend gate.

The independent disposable PostgreSQL runner passed **eight distinct exact cases**: state/original-
Session preflight; matching, valid different and exact historical Artifacts without prefix scans;
bound/unbound multi-package/version, empty and ordered Task contents and legitimate edits; missing/
foreign targets and missing versus existing-empty Task scope; effective deferred commit faults,
damaged/missing files and full-definition/manual-mode corruption; current logout/expiry/membership
and database-clock expiry during exact reader waits; observed cancellation; and separate pending
creators that commit or abort. Both whole-binding directions first match legitimately, then mutate
Task and outbox copies and return typed `InvalidRecord`. Fault counters and blocking barriers prove
the intended injection occurred; an invisible read is not used as rollback proof.

Adjacent exact runners separately passed **twelve old manual-observation**, **eight catalogue-
publication** and **thirty-three Session-content** cases: **61 distinct SQL cases** total including
the new eight. Repeat runs and name-list checks are not additional cases. Read-preservation checks
compare every table's sorted-row MD5 in the selected source/Artifact/Task schemas and private file
names, lengths, modes, SHA-256 and root device/inode/mode. These are scoped fingerprints, not raw-row
equality, an audit of every deployment schema or proof of historical publication provenance.

Capability validation passed **18 Node cases** and **412 existing distinct paths**. Only W16 gains
five source/documentation and five test references; scores, older evidence, snapshot fields and
topology remain identical to this round's baseline. All **88 earlier dated status sections** and
the published changelog tail remain unchanged. The ten old manual production files, shared pure
checks and catalogue confirmation file also retain their baseline hashes.

The complete backend gate passed **706 default Rust tests** and **208 common tests**, with **63
successful Rust result blocks** and no compiler warnings. **659 ignored Rust cases** are not counted
as default passes; the 61 exact SQL cases above ran separately. Rust formatting, generated contracts,
version synchronization, course copies and tooling checks passed. This round did not run a frontend/
SDK build, browser suite, real kernel/LAN acceptance, deployment, security audit or release gate.

Execution used an environment stripped of inherited deployment/database settings, a synthetic
private PostgreSQL 16 database with a Unix-only socket, and this round's owned private temporary
files. Before stopping the database, no fixture schema or other client remained. Its process was
confirmed stopped and its owned data/socket directories removed; project storage was not touched.
The final format/common gate again passed 208 checks without warnings; this repeat is not additional
test evidence. After it completed, the owned empty fixture/runtime directories and environment helper
were removed too. Cleanup removed **137,814,141 apparent bytes (about 131.4 MiB)** in total; this
round's logs remain in `/tmp/cyanrex-catalog-observation-xOASCU8O`. Only disposable fixtures were removed.

The [contract](session-catalog-task-draft-observation.md) is an internal read-only preparation step.
No HTTP listener, browser save, SDK, live authentication, installer or migration is connected.
The source version is unchanged; no commit, push or release is part of this development round.

### Unreleased independent catalogue draft publication · 2026-10-11

Source version remains **0.5.4**, outside the published tag. Independent C2-Z preparation returns
an opaque `SessionCatalogTaskDraftPublication` from the trusted catalogue-content workspace, an
exact reference and a C2-O plan. Structural token/source checks and complete exact definition lookup
precede target allocation; preparation performs no SQL, file or network I/O. Unknown coordinates
return `InvalidPreparation`, without provider discovery, manual fallback or inferred latest versions.

The frozen full definition may bind both assessment contracts or neither, but the final Task must
retain `Some` of that exact metadata. Each advance dispatches at most one existing current-Session
Artifact create, or the final catalogue-content Task create. It marks the exact target Unconfirmed
before awaiting; exact acknowledgement records progress without another await. Errors, mismatches
and dropped dispatched futures retain confirmed prefixes and stop. Wrong-token and unpolled calls
do not dispatch. Current authorization and actual owned text are independently rechecked by the
existing commands; no assessment, Review or acceptance is performed.

Only Artifact receipt matching and manifest construction are shared pure checks. Manifest binding
now rejects short or excess receipt prefixes before zip. The manual final matcher remains None-only;
the catalogue matcher requires the complete frozen Some definition. No old attempt conversion,
checkpoint/journal/observation API, durable receipt, recovery, retry, cleanup, whole-draft transaction,
HTTP/SDK/browser wiring, installer or migration was added. The eight protected old manual
checkpoint/dispatch/intent/inspection/observation source files remain byte-identical to this round's baseline.

The meaningful Red compiled and failed **0/1** on the temporary preparation stub's
`InvalidPreparation`, without compile warnings. The first attempt instead failed while opening its
fixture: a newly created parent directory was group-writable and was correctly rejected. That setup
failure is not feature Red. A premature Green build encountered not-yet-written test modules; a
later unit build found three missing test imports. Neither executed cases or established Green.
Both were corrected. Initial target Green passed four cases with one unused import warning; removing
that import and rerunning yielded **four default cases** and **eight ignored SQL cases**, without
warnings. The separate private receipt tests passed **eight cases**, including bound/unbound positive
controls, full-definition/owner/manifest negative controls, dropped futures and prefix cardinality.
The adjacent old manual unit filter passed **33 cases**; these default checks are not additive to the
complete backend gate recorded below.

The new disposable PostgreSQL runner passed **eight distinct exact cases**: mixed packages/versions/
bindings and ordered Unicode/same/empty text; unknown coordinates and Session substitution; second
Artifact failure; final definition/outbox/Session/membership/deferred-commit faults; damaged bytes and
preallocated Artifact/Task conflicts; current-Session revocation; observed cancellation waits; and
database-clock expiry while blocked. Counters prove injected faults were reached. Comparisons cover
full rows in seven Task/Artifact resource tables and private-directory names/bytes, not every identity
table. A failed Artifact step preserves the exact observed orphan as well as prior confirmed data;
a failed final Task step does not erase its Artifact prefix. Neither error nor cancellation implies
whole-draft rollback or permission to retry. Independent source/test review found no remaining
ineffective injection or new production defect.

All four adjacent PostgreSQL runners passed **67 distinct exact cases**: new catalogue publication
eight, old manual publication 12, Session content 33 and journaled dispatch 14. Each checked its exact
case name before execution; all completed without Rust compile warnings. These synthetic in-process
transactions do not establish a mounted service or live deployment.

The complete backend gate passed **696 default Rust and 202 common tests**, across **62 successful
Rust result blocks**, without compile warnings. **651 ignored Rust cases** are not counted as passed.
File/version/course synchronization, public API/SDK generation and compatibility, Rust formatting
and existing tool-fixture checks passed. This round did not rerun frontend/SDK functional suites,
browser or dependency audits, live kernel, SSH/LAN, remote CI or restart/recovery acceptance.
Tool-fixture checks do not validate a real deployment.

The tensor keeps **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 representative
chains**. Only W15 gains five source/document and five test/runner references, bringing distinct
paths to **404**. All other fields, scores, evidence, snapshot date and topology remain unchanged.
Tensor/reading-view checks passed **18 cases**; these are also included in the common gate, not
additional unique tests. Each locale's 43 prior dated sections remains byte-identical.

The disposable PostgreSQL 16 fixture used a private Unix socket only. Before stopping it, the
observed test-schema and other-client counts were both zero. Approximately **166 MiB** of this round's
owned database/test data, socket and launcher were removed; they are recreatable fixtures, not user
runtime data. Logs remain. The final format/common preflight also passed; its repeated 202 common
cases are not counted twice. Bilingual documents and built-in course copies are synchronized. This
round did not start the application service or change the source version, commit or push anything.

### Unreleased explicit catalogue Task-content HTTP · 2026-10-11

Source version remains **0.5.4**, outside the published tag. The independent C2-Y adapter takes
`CatalogTaskContentHttpState` with a frozen `SessionCatalogTaskContentWorkspace` and exposes four
create/read/replace/status commands only on its separately constructed `/platform/v1/catalog-tasks`
router. Creation requires an exact `definition_ref`, never a client definition, provider or mode
selector. Reference and package records use strict object decoding over the original JSON stream;
missing, unknown, duplicate, positional or invalid fields fail before Session dispatch.

Bound and explicitly unbound entries from composed package/version catalogues retain complete
definition metadata and private ownership. Existing Session transactions verify full metadata,
old/new exact text, revision fences and fresh authority. No assessment, Review or acceptance runs.
The old manual state, three-field creation JSON and prefix remain unchanged; wrong-mode records
fail closed. Unknown configured references map to generic `400 invalid_request / unconfirmed`
after the Session command was called, even when its internal lookup precedes SQL work.

The adapter reuses the dedicated Cookie/Origin/marker rules, 64 KiB/two-second body boundary,
private response projection and response-body-held admission. Clones share four default slots,
configurable from one to 16; separately constructed manual/catalogue/text budgets add. Saturation
does not poll the body or queue a Session command. Cancellation releases a slot, not a rollback
receipt. There is no normal-app mounting, issuer, discovery, automatic router merge, C2-P–W
publication/journal integration, live OpenAPI/SDK/browser connection or migration.

The focused Red test compiled and failed **0/1** because the temporary stub returned 404 instead of the
required missing-reference 400. Its minimal temporary harness emitted five unused-code/import
warnings. Green passed **28 default Rust cases**, with **eight ignored SQL cases** not counted as
passed and no compile warnings. The defaults comprise one missing-reference case, three exact
reference/mode cases, eight corrected catalogue-body cases and six transport plus ten admission
cases reused against the actual catalogue router. Duplicate-field injections assert their wire
changes, and valid nested-manifest controls prove rejection is not merely a missing reference.

The new disposable PostgreSQL runner passed **all eight distinct exact cases**. They cover mixed
packages/versions/bindings through all four commands, manual/catalogue and owner separation,
full metadata/binding drift and unknown coordinates, invalid/corrupt text, current Session and
account lifecycle, post-write definition/old/new input/session/membership and commit faults,
observed lock-wait expiry, post-write loss of an input removed by replacement, and response-body/
SQL-wait/cancellation admission. Fault counters prove mutations reached injected failures;
rejections compare full rows in **seven Task/Artifact resource tables** and all private-directory
file names/bytes, not every authentication/identity table. These observed fixture states, not
HTTP error outcomes, establish the tested nonchanges. Independent source/test review found no
new production defect or remaining ineffective injection.

All four relevant PostgreSQL runners passed **64 distinct exact cases**: new catalogue HTTP eight,
manual HTTP 13, Session content 33 and text HTTP ten. Each runner checked the exact case name before
execution; all completed without Rust compile warnings. These are in-process routers and synthetic
transactions, not a live listener, deployment or browser workflow.

The complete backend gate passed **684 default Rust and 196 common tests**; **643 ignored Rust
tests** are not counted as passed, across **61 successful Rust result blocks**, with no compile
warnings. File/version/course synchronization, public API/SDK generation and compatibility, Rust
formatting and existing tool-fixture checks passed. This round did not rerun frontend/SDK functional
suites, browser or dependency audits, live kernel, SSH/LAN, remote CI or restart/recovery acceptance.
Tool-fixture checks do not accept a real deployment.

The tensor keeps **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 representative
chains**. Nine new W13 source/test/document paths bring distinct paths to **394**; only W12's gap
wording additionally reflects the separate typed adapters. Scores, evidence, snapshot date and
topology remain unchanged. Tensor/reading-view checks passed **18 cases**. Each locale's 42 prior
dated sections remains byte-identical; the new adapter does not inherit their execution evidence.

The disposable PostgreSQL 16 fixture used only a private Unix socket; fixture schemas and other
client connections were zero before shutdown. Approximately **153 MiB** of this round's owned
database/test data, socket and launcher has been removed; these are regenerable fixtures, not user
runtime data. Logs are retained. No application service was started. The final format/common
preflight also passed; its repeated 196 common cases are not counted again. English/Chinese
documents and their built-in course copies are synchronized. No version, commit or push was made.

### Unreleased optional automatic-assessment bindings · 2026-10-11

Source version remains **0.5.4**, outside the published tag. `TaskDefinition` now has paired
`Option<VersionedName>` evidence-schema and assessment-policy fields. Definitions may supply both
exact pins or explicitly bind neither; half bindings fail both decoding and trusted registration.
Both JSON fields remain required: two explicit nulls mean no automatic assessment, while omission
is rejected. Old bound JSON retains its exact field order and values; existing records need no
migration. The Rust source fields require callers to adapt. New unbound records require the new
reader; old binaries fail closed, so backward reading/downgrade of those records is not promised.
Core and storage schema numbers remain unchanged.

Mixed metadata still feeds the existing bounded catalogue composition, Task drafts, schema-2
Session catalogue and schema-3 content-store/Session paths. An unbound definition remains
`Some(TaskDefinition)`, not a manual Task. Full saved-definition equality makes adding or removing
a binding under the same reference a metadata mismatch, not a policy upgrade. Typed provider
registration rejects unbound entries, and assessment checks both pins before calling the provider.
Actual `TaskAssessment` pins stay mandatory. There is no placeholder pass, rule Review, acceptance,
provider discovery, new authorization branch, public HTTP or browser-save connection.

Two focused Red tests compiled and failed separately (**0/1** each): trusted registration still
required an assessment binding, and ordinary optional-field decoding silently accepted omission.
This round adds **ten default Rust cases and two exact SQL cases**. Selected default regressions
passed **54 cases**: metadata 26, original typed catalogue nine, teaching compatibility six, Session
catalogue three, Session content four and content storage six. Pure cases cover required JSON keys,
duplicate/unknown fields, the exact old bound encoding, explicit null pairs, six-slot sequence
compatibility, half/truncated/extra values, namespace and byte/count limits, duplicate full references,
typed rejection before assessment and mandatory result pins.

The four adjacent PostgreSQL runners passed **86 distinct exact cases**: schema-2 Session catalogue
20, schema-3 Session content 33, content storage 21 and manual draft publication 12. The two new
cases are included in that total. They round-trip unbound catalogue definitions through drafts,
input/text replacement and Ready transitions, retain exact definition and private ownership, and
reject both adding and removing a binding under an unchanged reference. Each rejected operation
preserves all row values in seven business tables and every existing file name/byte; unaffected
definitions remain readable. Manual/catalogue separation and existing Session checks remain intact.
These new cases do not separately re-accept all lifecycle/authorization boundaries; adjacent suites
provide that coverage. Independent source review found no new production or boundary defect.

The complete backend gate passed **656 default Rust and 192 common tests**; **635 ignored Rust
tests** are not counted as passed, across **60 successful Rust result blocks**, with no Rust compile
warnings. File/version/course synchronization, public API/SDK generation and compatibility, Rust
formatting and existing tool-fixture checks passed. This round did not rerun frontend/SDK functional
suites, browser or dependency audits, kernel, SSH/LAN, remote CI or restart/recovery acceptance.
Tool-fixture checks are not evidence of a real deployment.

The tensor retains **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 representative
chains**. Four new test references bring distinct paths to **385**; scores, evidence, connections,
snapshot date and every non-implementation field are unchanged. Tensor/reading-view checks passed
**18 cases**. Each locale's 41 prior dated status sections remains byte-identical, including earlier
Unreleased rounds. English/Chinese documents and their built-in course copies are synchronized.

The disposable PostgreSQL 16 fixture used only a private Unix socket. Fixture schemas and other
client connections were both zero before shutdown. This round's approximately **138 MiB** of owned
database/test data, socket and launchers has been removed; these fixtures are regenerable and need
no migration, while execution logs are retained. No application service was started and user
runtime data was not changed. The final format/common preflight also passed, without counting its
repeated 192 common cases again.

### Unreleased bounded multi-package catalogue composition · 2026-10-11

Source version remains **0.5.4**, outside the published tag. Trusted
`TaskDefinitionCatalog::compose(&[catalogues])` combines frozen metadata for multiple domains,
package versions and task-definition versions. It accepts 1–256 input handles and checks the actual
flattened total of 1–256 definitions before allocation/copying. Nested composition has the same
global limit. Disjoint registrations of the same package/version are allowed; repeating a full
definition reference is rejected even for identical metadata, with no deduplication, overwrite or
latest-version fallback. Input and definition order remain intact in the owned immutable snapshot.

The existing Task draft, schema-2 Session catalogue and schema-3 content-store/Session constructors
accept this composed handle without new routing or authorization branches. Each saved definition
still must match in full. Old single-package registration and provider-bound typed assessment remain
strict and separate. Composition retains no provider, interprets no payload and adds no discovery,
publisher trust, public HTTP, browser saving, schema migration or publication/retry policy.

The first multi-package behavior test compiled and failed against the placeholder (**0/1**), then
passed after implementation. This round adds **eight default Rust cases and two exact SQL cases**.
Selected default regressions passed **44 tests**: 16 metadata, nine original typed catalogue, six
teaching compatibility, three Session catalogue, four Session content and six content-store cases.
Pure cases include all exact reference coordinates, source/entry order, frozen clone/drop behavior,
256/257 boundaries, nested limits, duplicate equal/drifted definitions and unchanged typed dispatch.

Both new database cases passed individually. In one workspace each creates four Tasks from definitions
across two packages, two package versions and two task versions; exact reads, input/text reordering and
state changes preserve full definitions and private ownership. Missing packages, missing exact
package versions and real metadata drift reject read/replace/transition, while another admitted
package remains readable. Seven business tables' complete row values and all existing file names/
bytes remain unchanged on rejection. During source review, a policy-version fault that originally
assigned its current value was changed to a genuinely different value; explicit inequality assertions
now prove both metadata injections alter the fixture. This required no production change.

The two new cases do not independently reaccept every authorization/lifecycle boundary; adjacent
existing suites supply that coverage. Independent production and test reviews found no new issue
or relaxation of manual, typed-provider or public boundaries. Execution counts and broader acceptance
remain scoped to this dated round, rather than inherited from earlier runs.

The four complete adjacent PostgreSQL runners passed **84 distinct exact cases**: 19 schema-2
Session catalogue, 32 schema-3 Session content, 21 content-store and 12 manual draft-publication
cases. The two new cases are included in those totals; individual reruns add no cases. The complete
backend gate passed **646 default Rust and 189 common tests**, with **633 ignored Rust cases**
excluded, **60 successful Rust result blocks** and no Rust compilation warnings. File/version/course,
public API/SDK generation and compatibility, Rust formatting and existing tool-fixture checks passed.
This round does not rerun frontend/SDK functional suites, browser or dependency audits, kernel,
SSH/LAN, remote CI or restart/recovery acceptance. Tool fixtures are not live deployment evidence.

The tensor retains **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 chains**.
Four new test references bring the distinct referenced paths to **381**; scores, evidence, edges,
snapshot dates and all non-implementation fields remain unchanged. Tensor/reading-view checks passed
**18 tests**. The 40 preceding dated status sections in each language remain unchanged, including the
prior Unreleased rounds. English/Chinese documentation and the built-in course copies are synchronized.

The disposable PostgreSQL 16 fixture used only a private Unix socket. Fixture schemas and other
client connections were zero before shutdown. Approximately **118 MiB** of this round's owned
database/test data, socket and launchers were removed; regenerable fixtures need no migration, and
execution logs remain. No application service was started and user runtime data was not changed.

### Unreleased catalogue and text-content composition · 2026-10-08

Source version remains **0.5.4**, outside the published tag. Explicit
`TaskContentStore::from_definitions` and `SessionCatalogTaskContentWorkspace::from_definitions`
combine frozen exact definitions with the existing schema-3 content manifest. The four new Session
commands create, read, replace and transition catalogue content without constructing a provider.
Definition admission and text-content metadata are independent choices: title/filename/language
labels do not replace definition display metadata, execute assessment or create a Run.

The original manual handles remain definition-free. Unknown creation coordinates are rejected before
database work; stored missing/unknown/drifted definitions and wrong modes fail closed. Full saved
definition equality, sorted old/new Artifact-byte checks, Task revision fences, atomic Task/manifest/
outbox records and final current-Session authorization plus confirmed commit remain shared. The
catalogue workspace cannot enter the manual HTTP or C2-P–W publication/observation entry points.
No schema change, migration, public mount, issuer, browser saving or recovery/retry policy is added.

The first Store and Session behavior tests each compiled and failed against their placeholders
(**0/1** each), then passed after implementation. Selected default regressions passed **10 tests**.
This round adds **eight default Rust cases and six exact SQL cases**; ignored tests are not counted
as default passes. **86 distinct exact PostgreSQL cases passed**: all 31 Session-content cases,
21 adjacent content-store cases, 18 schema-2 Session catalogue cases, 12 manual draft-publication
cases, two selected journaled-dispatch cases and two selected journal-observation cases. The last
two selections are not full reruns of their respective suites; repeat selections add no cases.

New database coverage includes exact-version/empty/32-item round trips, manual/catalogue/schema-2
isolation, six full-definition drifts, unknown coordinates, corrupt old or invalid new content,
owner/account lifecycle privacy, post-write record/event/file faults and Session expiry after an
observed storage wait. Rejected writes preserve complete business rows and existing files; fault
sequences prove the selected mutations reach their injection points. An independent source review
found no new production issue or public/manual boundary relaxation.

The first backend preflight passed **185/186 common tests**: one old journal-read source assertion
still matched the old helper signature. It now explicitly requires the manual-only `None` argument;
the production path needed no adjustment. The complete rerun passed **638 default Rust and 186
common tests**, with **631 ignored Rust cases** excluded, **60 successful Rust result blocks** and
no Rust compilation warnings. File/version/course, public API/SDK generation and compatibility,
Rust formatting and existing tool-fixture checks passed. This round does not rerun frontend/SDK
functional suites, browser or dependency audits, kernel, SSH/LAN, remote CI or restart/recovery
acceptance; earlier dated runs are not fresh evidence for these changes.

The tensor retains **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 chains**.
Source, test and evidence references total **377 distinct paths**, and the W11/W12 gap descriptions now reflect
explicit catalogue mode; scores, evidence, edges and earlier dated records remain unchanged.
English/Chinese documentation and bundled course copies are synchronized. The disposable PostgreSQL
16 fixture used only a private Unix socket; after verifying zero fixture schemas and other
connections it was stopped. Approximately 119 MiB of owned database/test data, socket and launchers
were removed, with logs retained. No application server was started in this round.

### Unreleased task definition and payload decoupling · 2026-10-08

Source version remains **0.5.4**, outside the published tag. Frozen `TaskDefinitionCatalog` metadata
can now admit Task drafts and Session catalogue commands without constructing or retaining a
`TaskProvider`. The existing typed `TaskCatalog<P>` still owns evidence and assessment. Exact
single-package/version lookup, mandatory evidence-schema/policy pins, full saved-definition equality,
private ownership, input-byte checks and transaction-time Session checks remain unchanged.

Portable payload hints now follow the existing server ASCII contract, independently of 14 installed
editor profiles. Unknown valid hints retain their raw values and use the existing plain-text profile
with an explicit notice and disabled language tools. Synchronization compares resolved profile IDs,
preserving the model and undo history; only an explicit language edit changes the saved hint. This
does not add a payload kind, format version, LSP transport or server connection.

The first metadata behavior test compiled and failed against the placeholder (**0/1**), then passed.
The frontend draft selection first passed **21/23**, with two new compatibility assertions failing,
then passed after implementation. Final selected Rust regressions passed **26 default tests**:
eight new metadata, nine original catalogue, three Session catalogue and six teaching compatibility.
The first Session selection failed all three cases because this round's temporary parent directory
was mode 0775; correcting that owned fixture to 0700 made the complete selection pass without
weakening production file checks.

**35 distinct exact PostgreSQL cases passed**: 18 Session catalogue (including two new provider-free
round-trip/metadata-drift cases) and 17 adjacent input-replacement cases. Coverage retains binary and
empty inputs, exact versions, old/new constructor interoperability, owner privacy and unchanged
business rows/files on rejected full-definition drift. Repeated selections are not extra cases.

The backend gate passed **630 default Rust and 183 common tests**, with **625 ignored Rust cases**
excluded, **60 successful Rust result blocks** and no Rust compilation warnings. The frontend gate
passed **208 functional tests**, production build/type checks and a zero-vulnerability production
dependency audit; existing Node module-type warnings remain. **41 real-browser cases** passed across
task payload, multilingual editor, filename-import and navigation suites, including unknown-hint
round-trip/edit/undo, rejected edits, model disposal and four-locale 320px layout. The browser used
mocked identity responses, not a live Engine or backend-save acceptance; the payload suite issued no
Engine writes. Public API/SDK generation and compatibility, file/version/course and tool checks passed.

The tensor keeps **70 coordinates, 69 functions, 39 evidence entries, 114 edges and 19 chains**;
only implementation source references expand to **367 paths**, without changing scores or evidence.
Earlier dated records remain unchanged. This round does not rerun SDK functional tests, Rust dependency
audit, kernel, SSH/LAN, remote CI or restart/recovery acceptance. No mount, issuer, storage migration,
automatic rule execution, save/retry policy or authorization relaxation is added.

The disposable PostgreSQL 16 fixture used only a private Unix socket. After verifying zero fixture
schemas and other connections, it and the owned loopback Next.js server were stopped; approximately
103 MiB of database/fixture data, socket and launcher were removed, while reproducible logs remain.

### Unreleased C2-X text Artifact HTTP · 2026-10-08

Source version remains **0.5.4**; this follow-up is outside the published tag. The independently
constructed text router creates one private fixed-profile Artifact and reads one exact historical
revision through existing current-Session commands. Its **2 MiB wire limit**, **256 KiB decoded-text
limit** and separate state-family admission budget do not expand C2-N's 64 KiB manifest limit.
Returned references can feed a separate Task request; this is not an atomic batch or router merge.

The first real behavior test failed against the placeholder: a valid 70 KiB text request returned
`service_unavailable / not_attempted` instead of reaching the closed source with `unconfirmed`.
After implementation, **12 default transport/contract tests, 10 exact PostgreSQL cases and three
new common guards passed**. Test review then strengthened two existing assertions: valid JSON
precedes trailers/stream errors, and rejected-input preflight holds an exact source-schema lock.
The full twelve-default/ten-SQL selection passed again, with unchanged case counts and no production
change required for those assertion improvements.

Coverage includes empty/Unicode/exact-limit text, strict JSON and client-authority rejection,
historical revision pins, foreign/missing privacy, current Session/membership and expiry behind
observed storage waits, duplicate conflicts, outbox/deferred-commit and file/schema faults, retained
response capacity and cancellation. Trigger counters prove selected write faults execute; retained
orphan files are neither adopted nor deleted. The bridge is an independent in-process fixture HTTP
sequence: text publication, then a separate C2-N Task create/read using exact returned references.

**56 adjacent exact SQL cases also passed**: 13 C2-N HTTP, 18 Session Artifact and 25 C2-M content
cases. Including C2-X, this round ran **66 distinct SQL cases**; targeted/full reruns do not increase
that number. The final backend-only gate passed **622 default Rust and 180 common tests**, with
**623 ignored Rust cases** excluded and **59 successful Rust result blocks**, without Rust compilation
warnings. File/version/course, public API/SDK generation and compatibility checks and existing tool
fixtures passed. The capability map now records **70 coordinates, 69 functions, 39 evidence entries,
114 edges and 19 representative chains**; W23 alone adds scoped **3/1/3/1** evidence, while all older
coordinates, scores, evidence and dated records remain unchanged.

PostgreSQL 16 used a disposable private Unix socket with no TCP listener. After confirming zero
fixture schemas and other connections, the server was stopped and approximately 120 MiB of owned
database/fixture data, socket and launcher removed; reproducible test logs remain.
This round does not rerun
frontend/SDK functional suites, dependency audits, actual browser/proxy/TLS, kernel, SSH/LAN, remote
CI or restart/recovery acceptance. Generic Artifact reads may materialize 1 MiB before the stricter
text projection. No live mount, issuer, browser saving, installation/migration, journal enforcement,
automatic retry/adoption/cleanup or global worker/memory quota is added. Cancellation is not rollback
proof and does not stop dispatched blocking I/O; earlier verification records remain historical.

### Unreleased HTTP shared admission 2026-10-08

Source version remains **0.5.4**; this follow-up is not in the published tag. The standalone C2-N
adapter now shares a fail-fast budget across cloned states and their routers: four slots by default,
trusted configuration from one to sixteen. Slots cover body collection, C2-M, serialization and
returned response bodies through their terminal frame/end, error or drop. Excess requests return
private `503 / capacity_exhausted / not_attempted` before body polling or SQL.

The first real Rust behavior test failed against the original implementation: with four body reads
held, a fifth request still reached the closed source and returned `service_unavailable / unconfirmed`.
It passed after implementation, followed by **24 HTTP default tests** (14 preceding, 10 new),
**one new outgoing-body error unit** and **three new common wiring guards**. The unit separately
keeps a failed body alive while checking that its slot is already released.

**13 exact HTTP PostgreSQL cases and 25 adjacent C2-M cases passed**, totaling **38 distinct SQL
cases**. Two new HTTP cases retain committed response bodies, reach an observed event-write blocker,
reject overflow without business changes and verify completion or cancellation with source-pool
restoration. The cancellation fixture observes unchanged Task/outbox/files; cancellation alone is
not rollback proof. A test-only abort-on-drop guard also prevents an early assertion failure from
detaching the pending request. The HTTP runner and CI inventory now enumerate all thirteen cases.

The backend-only local gate passed **610 default Rust and 177 common tests**, with **613 ignored
Rust cases** excluded and **58 successful Rust result blocks**, without compilation warnings.
File/version/course checks, public API/SDK generation and compatibility checks, and existing tool
fixtures passed. The current capability tensor remains 69 coordinates, 38 evidence entries, 111
edges and 18 representative chains, with unchanged scores. The historical 0.3.8 network checker
reports source/version drift as expected for that preserved snapshot; its hashes were not refreshed.

This round does not rerun frontend/SDK functional suites or dependency audits, actual browsers,
kernel, SSH/LAN, remote CI or restart/recovery acceptance. No listener, issuer, public save connection,
schema/journal change, global process/worker/memory quota or retry policy is added. Cancellation does
not terminate dispatched blocking I/O. The earlier dated evidence and published version record stay unchanged.

### 0.5.4 version validation · 2026-10-08

Version metadata, current documentation and generated course copies are synchronized at **0.5.4**.
The fresh full local gate passed **599 default Rust, 174 common, 206 frontend and 42 SDK tests**;
**611 ignored Rust cases** are not passes. Rust produced **58 successful result blocks** without
compilation warnings. Frontend/SDK production dependency audits each reported zero vulnerabilities;
the existing Node `MODULE_TYPELESS_PACKAGE_JSON` warnings remain.

This version-only validation does not rerun opt-in PostgreSQL, actual browser, kernel, SSH/LAN,
remote CI, Rust advisory or deployed restart/recovery acceptance. The separately dated C2-T–W
implementation evidence below keeps its original version, counts and scope. Version publication
does not mount prepared APIs, migrate a database or update a running deployment.

### Unreleased C2-W journal step observation · 2026-10-08

Source version remains **0.5.3**. **Two default Rust tests, twelve exact PostgreSQL cases and three new
common guards passed** for the separate journal-bound resource observer. The twelve ignored SQL cases
in the default target are not default passes; the final Rust runs emitted no compilation warnings.
A real behavior test failed against the stub before the implementation passed.

The first SQL runner reached six passes before the seventh case failed during fixture setup: it tried
to revise an Artifact's kind, which the existing command correctly rejects as `InvalidInput`.
The fixture now retains `cyanrex.task-text` while changing title/bytes to create a later head.
That exact case passed, followed by a fresh complete **12/12** runner. This fixes test preparation,
not production behavior, and neither relaxes the kind rule nor adds a distinct case.

Coverage distinguishes missing/filtered intents, unrecorded ordinals and invisible resources; it
checks exact historical Artifact revisions, current Task edits and all referenced contents, typed
blob faults, missing versus existing-empty Tasks, final authorization, waiting and cancellation.
Fresh Sessions are seeded fixtures, not a public issuer or real OTP-login test. Account-generation
rejection changes a stored binding field, not an actual account deletion/recreation cycle.

Concurrency checks show external journal metadata/nonce/DDL writers blocked during resource waiting;
after reader cancellation the writer can proceed and a later read rejects the damaged record.
This is not in-transaction DML-trigger or OID-replacement fault injection. First-snapshot/full-prefix
reuse is constrained by source guards alongside normal read execution. The observer still performs
no business write and returns no body, nonce, receipt, restored attempt or retry permission.

All **61 adjacent exact PostgreSQL cases passed**: 25 C2-M content, 12 C2-S checkpoint inspection,
10 C2-V journal inspection and 14 C2-U dispatch cases. Together with C2-W, this round covers
**73 distinct SQL cases**; targeted reruns do not add distinct cases. The isolated PostgreSQL 16
used a private Unix socket without TCP. Cleanup confirmed zero custom fixture schemas and zero other
connections before stopping the server and removing approximately **185 MiB** of owned database data.

The full quality gate exited successfully: **599 default Rust tests passed**, with **611 ignored**
tests excluded from that count, across **58 successful result blocks**. **174 common, 206 frontend
and 42 SDK tests passed**. Next.js **15.5.27** production build, frontend type checking, SDK build/type/package
checks, generated contracts, API compatibility against **0.3.0**, course/version checks and tools passed.
Fresh frontend and SDK production dependency audits each reported zero findings at every severity.
Rust emitted no compilation warnings; the existing non-failing Node `MODULE_TYPELESS_PACKAGE_JSON`
warnings remain.

This round did not rerun actual browser, kernel, SSH/LAN, remote CI, Rust advisory audit or deployed
restart/recovery acceptance. Source version remains **0.5.3**, with this work unreleased and not committed or pushed.

### Unreleased C2-V journal inspection · 2026-10-08

Source version remains **0.5.3**. **Two default Rust tests, ten exact PostgreSQL cases and three new
common guards passed** for the independent current-Session journal reader. The ten ignored cases in
the default target are not counted as default passes. Final Rust runs emitted no compilation warnings.
A real behavior test first failed against the `UnsupportedSchema` stub, then passed with implementation.

The first ten-case SQL run passed nine cases; its concurrency fixture failed while trying to corrupt
an owned synthetic file installed with mode `0400`. Only that fault-injection fixture changed its file
to `0600`; production permissions were unchanged. The exact case then passed, followed by a fresh
complete **10/10** runner. The reruns are not extra distinct cases or a production permission fix.

Coverage includes an empty plan's zero records versus its one final Task, a closed journal pool with
the source-owned transaction, primary-key/nullability/scope metadata checks, invalid prefixes and
plan-specific row bounds. Duplicate rows behind invalid uniqueness are rejected as `UnsupportedSchema`,
not evidence of reaching a duplicate-row decoder branch. Synthetic rows test the 32-item/33-step limit
and rejection of a 34th row; they are not 33 real resource writes. Account-generation tests alter the
stored binding, not an actual account deletion/recreation cycle.

Present and absent intent reads recheck expiry after waiting. Readers wait for B to commit or roll back;
cancellation preserves business state, and damaged current resources do not trigger a resource/file
probe. Results remain journal metadata, without nonces, restored authority or permission to retry.
All **26 adjacent exact PostgreSQL cases passed**: fourteen C2-U dispatch and twelve C2-T intent
cases, for **36 distinct SQL cases** in this round. Tests used an independent PostgreSQL 16 server
on a private Unix socket with no TCP listener or deployment URL. No custom fixture schemas or other
connections remained; the server was stopped and its roughly 133 MiB temporary data and remaining
synthetic files removed. Diagnostic logs were retained; project data was not deleted.

The fresh full local gate passed **597 default Rust, 171 common, 206 frontend and 42 SDK tests**.
Rust produced 57 successful result blocks without compilation warnings; **599 ignored cases are not
counted as default passes**. Next.js 15.5.27 production build/TypeScript, SDK build/type/package,
generated contracts, compatibility against baseline 0.3.0, version metadata, course mirrors and tool
fixtures passed. Fresh frontend and SDK production audits each reported zero findings. Existing Node
module-type warnings remain. These results do not establish actual browser/kernel acceptance,
remote CI, Rust advisory audit, host crash, backup replay or deployed recovery; those were not rerun.
Source remains based on `fa15fe35c492eb1b61114c8430d67eb7936f11c4` plus uncommitted C2-T/U/V work.

### Unreleased C2-U journaled dispatch · 2026-10-08

The final fresh full local gate passed **595 default Rust, 168 common, 206 frontend and 42 SDK tests**.
Rust produced 56 successful result blocks with no compiler warnings; **589 ignored cases are not
counted as default-suite passes**. Next.js 15.5.27 production build/TypeScript, SDK build/type/package,
generated contracts, compatibility against baseline 0.3.0, version metadata, course mirrors and tool
fixtures passed. Fresh frontend and SDK production audits each reported zero findings.

Source version remains **0.5.3**, based on `fa15fe35c492eb1b61114c8430d67eb7936f11c4` plus the
uncommitted C2-T/U work. **Three new default Rust tests, fourteen exact PostgreSQL cases and three
new common guards passed**. Real fresh-install and dispatch behavior Reds preceded the implementation.
The consumed original attempt first confirms a unique Unknown reservation; a separately authorized
transaction then verifies its exact binding and commits one resource's SQL with its marker. Existing
C2-M policy is shared rather than bypassed. The original C2-P path remains independently callable.

The new cases cover fresh schema 2 and unchanged schema 1 rejection, empty and Unicode drafts,
exact owner/account/nonce/prefix checks, actual Session removal between A and B, wait expiry,
trigger faults, deferred commit failure and cancellation in each phase. Post-resource intent/marker
changes and equivalent step-table replacement roll back the business SQL; a 34-row overflow is
rejected without truncation. A final targeted rerun also proved rejection when a resource trigger
changes only a previous confirmed nonce. Account-generation tests mutate stored binding fields,
not genuine account deletion/recreation. Definite B rollback retains A's Unknown and may retain files;
an uncertain B COMMIT can already have committed its marker and is not rollback or retry evidence.

All **84 adjacent exact PostgreSQL cases passed**: 25 C2-M, 11 standalone content HTTP, and 12 each
from C2-P/Q/S/T, for **98 distinct SQL cases** including C2-U. These ran on a private PostgreSQL 16
Unix socket with no TCP listener or deployment URL. No custom fixture schemas or other database
connections remained; the server was stopped and its roughly 168 MiB temporary data removed.
Diagnostic logs were retained; project data was not deleted. Reopened handles and controlled faults
do not test host crash, backup replay, physical source continuity across A/B, or restart recovery.

The first full gate stopped at a fresh Next.js 15.5.24 production audit: one moderate dependency
finding covered GHSA-4jqv-mc3x-m676 and GHSA-mcj8-r9mp-w47p. The manifest and lock now select 15.5.27;
three offline security checks passed after a real Red, including refusal to adopt the unsafe 16.0.0
branch merely because its major version is higher. Historical 0.5.3 audit results remain unchanged;
dependency patching does not update or prove exploitability of a running deployment.

Separately, **four task-navigation browser cases passed** on real production Next.js 15.5.27 pages
and Monaco using a fixture-only Engine URL: same-page draft preservation, expired-Session recheck,
pending logout and fragment-preserving login redirects. Engine replies were mocked and unapproved
network requests blocked; these are neither real server saving nor cache-poisoning exploit acceptance.
The temporary loopback frontend was stopped. Remote CI, Rust advisory audit, deployed restart,
backup replay, kernel, SSH and LAN acceptance were not rerun.

### Unreleased C2-T intent registration/read boundaries · 2026-10-07

Source version remains **0.5.3**, based on `fa15fe35c492eb1b61114c8430d67eb7936f11c4` plus this
uncommitted slice. The fresh full local gate passed **592 default Rust, 165 common, 206 frontend and
42 SDK tests**. **575 ignored Rust cases are not counted as passes**; no Rust compilation warnings
were emitted. Next production build/TypeScript, SDK build/type/package checks, generated contracts,
the 0.3.0 compatibility baseline, version metadata, course mirrors and tool fixtures passed. Frontend
and SDK production audits each reported zero vulnerabilities.

Separately, **48 exact PostgreSQL cases passed**: 12 new C2-T cases and the 12 cases in each existing
C2-P, C2-Q and C2-S runner. The private disposable server had no TCP listener; deployment/database
variables were cleared before selecting the explicit test URL. A real schema-damage Red preceded the
fix. Checks cover registration/reopening, conflicts, current authorization and wait expiry, bounded
canonical metadata, structural primary keys/columns, completed metadata-table replacement, synchronous
commit settings, injected rollback and controlled cancellation. Account-generation rejection uses
a changed journal binding plus a fresh Session, not actual account deletion/recreation. Reopening
handles is not server restart or host crash recovery. These tests neither connect browser saving nor
enforce write-ahead resource dispatch; unknown commit still does not authorize retry. No real browser,
backup-replay, remote CI, Rust advisory audit, deployment, kernel, SSH or LAN acceptance was rerun.

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

1. Define uncertain-outcome handling and any authorized restart/recovery contract before public
   dispatch or retry. C2-U provides per-resource write-ahead and bound-identity gates only through a
   surviving wrapper; C2-P remains independently callable. Intent/checkpoint reads do not restore the
   wrapper, prove original storage incarnations or authorize adoption. C2-Q/S/AA observations still
   permit neither retry nor deletion based on absence.
2. Build reviewed Session issuance and an explicit authenticated installation/mounting path for the
   C2-N router, then public content publication. Preserve its Origin/request limits and conflict/outcome
   semantics; do not silently switch live authentication or migrate existing storage.
3. Connect explicit browser save, private task listing, reload and conflict handling. The internal
   [metadata index](task-content-index.md) and its [unmounted HTTP adapter](task-content-index-http.md)
   supply bounded owner-private pages only; mounting and content reopening still need current authorization.
   Preserve unsaved work and distinguish
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
