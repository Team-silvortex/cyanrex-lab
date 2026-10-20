# Cyanrex project documentation

[Translation coverage and boundaries](localization.md) describe the complete dictionaries and
bilingual public guides included in 0.5.9, including Korean UI support and explicit course fallback.

Cyanrex is moving from an eBPF teaching application to a self-hosted collaboration platform for
people, AI Agents and compute resources. The existing teaching runtime remains usable. General
identity, content, task and review services are explicitly configured preparation layers; the task
payload editor is local-only, and autonomous AI and general Run orchestration are not implemented.

This index describes the whole project, not only its first teaching domain. The source version is
**0.6.3**, including explicit loopback/TLS hosting, limited Session issuance and private Task
create/list/reopen/manifest replacement. The **2026-10-16 0.6.2** bound Session, format 3 and internal startup/HTTP/Task-controller separation, plus the 0.6.1 startup/storage/Session privilege work, remain.
The 0.6.0 layouts, independent Session lifecycle/stock and original read-only storage gates remain.
Local multi-file authoring, private task metadata indexing and large-file preparation remain included.
Earlier task decoupling, C2-O–AA, Next.js 15.5.27, AI host adapters and security boundaries remain in place.
Source inclusion does not connect browser/SDK saving, publish new Artifact bytes, switch live authentication,
add LAN access, migrate storage or validate a deployment. Dated `Unreleased` descriptions retain their original
implementation stage, source versions and verification scope; they are not fresh 0.6.3 execution records.
The documents distinguish source-release contents, live runtime capabilities
and target architecture; a built module or passing test does not imply an online deployment.

0.5.4 [C2-T intent records](session-task-draft-intent.md) add separate Session-authorized
registration and reading. [C2-U journaled publication](session-task-draft-dispatch.md) adds an explicit
live wrapper with per-resource write-ahead steps; it does not recover attempts or connect browser saving.
[C2-V journal inspection](session-task-draft-journal-inspection.md) separately reads recorded steps under
current authority, without checking resource contents, restoring the wrapper or authorizing retry.
[C2-W journal step observation](session-task-draft-journal-observation.md) separately reads one recorded
target's actual content in the same authorized transaction; its body-free comparison is not a receipt or recovery.

The October 8 [C2-X text Artifact router](text-artifact-http.md), included in 0.5.5, separately publishes text
and reads exact revisions. It remains unmounted; caller-controlled composition with the Task router
does not connect browser saving or create a whole-draft transaction.

The independent **[C2-Y catalogue-content router](catalog-task-content-http.md), included in 0.5.5,**
adds exact definition selection to a separate typed HTTP adapter. The old manual router stays
manual-only; both remain unmounted and independent of publication/journal workflows. This October 11
follow-up adds no Session issuer, assessment or browser saving.

The independent **[C2-Z catalogue publication attempt](session-catalog-task-draft-publication.md), included in 0.5.5,**
freezes an exact full definition without I/O and
explicitly publishes one Artifact or creates the final catalogue Draft per advance. Manual C2-P–W
types remain separate; its in-memory prefix is not a journal, recovery, HTTP route or browser save.

The independent **[C2-AA catalogue observation](session-catalog-task-draft-observation.md), included in 0.5.5,**
reads one unknown C2-Z target through current Session
authority. It compares complete definition/text metadata without returning bodies or adopting
progress. Matching and invisibility neither prove an earlier commit/rollback nor enable retry.

The independent **[task-content metadata index](task-content-index.md), included in 0.5.6,**
lists private schema 3 metadata under trusted ownership or current Session authority.
It returns bounded pages without Artifact reads, HTTP/SDK/UI, writes, migration or recovery; this
follow-up is separate from the preceding release and verification records.

A separate **[index HTTP adapter](task-content-index-http.md), included in 0.5.6,**
projects that current-Session page at exact `GET /platform/v1/task-index`. The later retained-host
[index connection](platform-task-index-connection.md) mounts it only in the explicit workspace host,
with strict raw pagination queries and private response-lifetime admission; existing content routers,
Session issuance, SDK/OpenAPI and browser behavior do not change.

The **0.5.6 [large-file preparation](large-file-distribution.md)** provided independently composed
streaming, staging, upload quotas, journaling, verification, publication and private metadata services.
At that release, publisher/recipient contracts only compared supplied snapshots; durable grant issuance,
recovery and both network modes were unconnected. 0.5.7 added separate grants and chunk transport;
the [ordered download follow-up](large-file-download-client.md) is now included in 0.5.8.
Normal-app distribution, user-facing save dialogs, resume and desktop transfers remain unconnected.

## Start with your question

For local authoring, [add multiple text files](local-text-file-import.md) in one reviewed batch,
without replacing the draft or connecting server saving (Unreleased after 0.5.8).

| What you need | Read first |
|---|---|
| Understand the project and its boundaries | [Current architecture map](architecture-map.md), [detailed architecture](architecture.md), [platform and teaching concepts](concepts.md) |
| See all current modules and connections | [Current platform feature map](platform-network.md) |
| Compare architecture, capabilities, implementations and maturity | [Capability tensor and evidence-backed scores](capability-maturity.md) |
| Know what is implemented and what comes next | [Project status](project-status.md), [target architecture](next-architecture.md) |
| Inspect the new independent platform Session boundary | [Session lifecycle](platform-session-lifecycle.md); 0.6.0, current-session query and confirmed logout only, no issuer or normal-app mounting |
| Run existing Session query/logout without password or TOTP reads | [Session database privileges](platform-session-permissions.md); Unreleased after 0.6.0, tested column permissions and nonexecuting relation locks, not a mounted host or read-only role |
| Bound persistent Session stock during explicitly selected login | [Session stock limits](durable-session-stock.md); 0.6.0 trusted preparation, not a public issuer or automatic expiry cleanup |
| Check explicit host configuration and database catalog identity | [Host catalog preflight](platform-host-preflight.md); 0.6.0 read-only preparation, not storage health or a mounted host |
| Check an explicitly selected Linux service process before workers | [Process startup check](platform-process-startup.md); Unreleased after 0.6.0, exact identity/capabilities and no-new-privileges, not a sandbox or running host |
| Open one explicit local database connection against an original catalog | [Database connector](platform-database-connection.md); Unreleased after 0.6.0, no automatic retry, pool or peer-authentication claim |
| Declare original catalog identities before the first connection | [Catalog declaration](platform-catalog-expectation.md); Unreleased after 0.6.0, strict private version 2 and immutable comparisons, not authenticated discovery or a running host |
| Check a private startup configuration after the Linux process gate | [Private startup input](platform-startup-input.md); Unreleased after 0.6.0, strict JSON and file checks without connecting or starting a host |
| Run one read-only storage diagnostic from that private input | [Storage check](platform-storage-check.md); Unreleased after 0.6.0, actual root/catalog/marker checks and connection close, not a retained runtime baseline |
| Open fresh connections against retained catalog, marker and root observations | [Retained storage connections](platform-storage-connection.md); Unreleased after 0.6.0, explicit caller-selected bindings, not a pool, current authorization or running host |
| Open a retained binding from complete trusted declarations | [Expected-storage opener](platform-storage-open.md); Unreleased, declared checks and confirmed close before capability handoff; the factory itself starts no listener |
| Query or end an existing Session through retained platform storage | [Bound Session connections](platform-session-connection.md); Included in 0.6.2, same-transaction original storage/Source guards, no pool, issuer or mounted host |
| Log in an existing account against that original binding | [Bound login](platform-login-connection.md); Unreleased after 0.6.2, explicit limited internal issuance, not public HTTP login, Cookie delivery or a running host |
| Project confirmed login into a dedicated Cookie | [Login HTTP adapter](platform-login-http.md); Unreleased, independently constructed and unmounted; bounded request admission, not deployed TLS or browser authentication |
| Combine login, Session query and logout over one original binding | [Shared Session router](platform-session-router.md); Unreleased, independent admission lanes and one private fallback; no normal application or browser mounting |
| List owner-private Task metadata without code or execution | [Retained Task index](platform-task-index-connection.md); Unreleased, original storage and final current-Session authorization; no content reopening or browser saving |
| Reopen your saved empty or text-payload Task through the independent host | [Retained content reads](platform-task-content-connection.md); Unreleased, original root descriptor and verified exact revisions, GET-only; no writes or browser integration |
| Create your own manual Draft without payload, code or a Run | [Retained empty Task creation](platform-task-create-connection.md); Unreleased, strict ID/title POST and final current-Session authorization; no payload publication, browser saving or SDK/OpenAPI |
| Revise a manual Draft title or its existing exact text references | [Retained manifest replacement](platform-task-replace-connection.md); Unreleased, expected-revision PUT and verified old/new owned bytes; no Artifact publication, status writes or browser/SDK integration |
| Run Session routes and private Task creation/reads/replacement in a low-privilege Linux process | [Loopback host](platform-loopback-host.md); Unreleased, format 3 and exact Origin/bind, bounded HTTP/1.1 and signal drain; no LAN/TLS, proxy backend or browser integration |
| Run the same routes directly over native HTTPS loopback | [TLS loopback host](platform-loopback-tls-host.md); Unreleased, private certificate/key and IP SAN checks, bounded handshake and Secure Cookie; no CA installation, LAN/browser or Cookie-port isolation |
| Report host startup and shutdown to an explicitly selected service manager | [Service notifications](platform-host-notifications.md); Unreleased, opt-in Ready/Stopping over Unix datagrams, not installed supervision, continuous health or recovery |
| Declare original catalog and five-marker identities before the first inspection | [Marker identity declarations](platform-marker-expectation.md); Included in 0.6.2, strict format 3 diagnostics, no baseline adoption or host startup |
| Recheck an explicitly selected original private Artifact directory | [Artifact root preflight](platform-artifact-root.md); 0.6.0 metadata-only observation, not content health or runtime handoff |
| Bind five storage version declarations and their scope to an original catalog | [Storage marker observation](platform-storage-markers.md); 0.6.0 marker-only preparation, not complete schema compatibility or authority health |
| Review centralized large-file distribution and later desktop transfers | [Distribution proposal](large-file-distribution.md), [streaming core](large-file-core.md), [private chunk staging](large-file-staging.md), [upload intent and logical quotas](large-file-upload-intent.md), [local receipts](large-file-upload-staging.md), [write-ahead chunk journal](large-file-chunk-journal.md), [whole-byte observation](large-file-whole-verification.md), [persistent publication metadata](large-file-publication.md), [Session private metadata](large-file-session-metadata.md), [pure publisher/recipient contracts](large-file-access-contract.md); normal-app distribution and desktop transfers not connected |
| Inspect the persistent publisher-grant ledger and own Session reads | [Persistent publisher grants](large-file-publisher-grants.md); included in 0.5.7, outside the 0.5.6 tag, not grant-management authorization or publication permission consumption |
| Issue or revoke grants through an explicitly configured manager | [Session grant management](large-file-grant-management.md); host-configured exact account, not inherited roles, durable online manager revocation or publication permission consumption |
| Publish an already staged upload with a current publisher grant | [Session publication](large-file-session-publication.md); final authorized source commit plus original-byte checks, not downloads, network transfer or recovery |
| Inspect exact-file recipient grants through the current recipient | [Persistent recipient grants](large-file-recipient-grants.md); independent revocation and own metadata reads, not recipient management or download permission |
| Share an own Published version or stop its old shares | [Publisher-owned file sharing](large-file-sharing.md); current publisher permission for issuance, exact owner for revocation, not download admission |
| Observe a granted exact Published download manifest | [Recipient manifest observation](large-file-download-manifest.md); current recipient and final grant/Session expiry, not byte health or a reusable download permit |
| Read one currently authorized verified file chunk | [Chunk snapshots](large-file-chunk-read.md); private up-to-4-MiB verification and original-authority reentry, not HTTP or client delivery |
| Inspect the separate unmounted binary chunk route | [Single-chunk HTTP adapter](large-file-chunk-http.md); strict exact-version GET and byte-owner budgets, not mounted LAN delivery or client saving |
| Validate and privately stage a supplied binary response | [HTTP chunk receiver](large-file-chunk-receiver.md); explicit EOF/hash before exclusive local staging, not active fetching, whole-file saving or resumption |
| Explicitly fetch one chunk from a fixed trusted origin | [Controlled HTTP client](large-file-chunk-client.md); HTTPS/opt-in literal loopback and job-owned request admission, not runtime mounting or whole-file save/resumption |
| Assemble existing legacy chunks into one private file | [Private whole-file assembly](large-file-chunk-assembly.md); actual ordered/whole bytes and original-inode readback, not user-selected saving, resume or publication |
| Download ordered chunks and privately assemble them | [Ordered download client](large-file-download-client.md); Included in 0.5.8, shared flow/job ownership, not UI saving, resume or mounted delivery |
| Observe an authorized full manifest through an independent JSON GET | [Recipient manifest HTTP](large-file-manifest-http.md); Included in 0.5.8, exact current recipient, not byte permission |
| Fetch exact metadata and compose granted private download | [Exact manifest client](large-file-manifest-client.md); Included in 0.5.8, empty files still require GET, no UI save/resume or live mounting |
| Save the original acknowledged output to an explicitly selected new private file | [Explicit private saving](large-file-save.md); Included in 0.5.8, original descriptor proof and exclusive copy/readback, not a UI dialog or resume |
| Download an exact granted version and save one explicitly selected private copy | [Authorized download and saving](large-file-delivery-client.md); one whole-flow lease through actual file jobs, not UI saving, resume or a mounted service |
| Explicitly select an owned ordinary directory without weakening private staging | [Owned user-directory saving](large-file-user-save.md); 0700/0750/0755 with original UID/GID/mode checks, not shared-writable folders or UI picking |
| Observe current delivery phase and chunk ACKs without claiming save completion | [Live delivery progress](large-file-delivery-progress.md); one latest per-call snapshot, not durable progress, receipt, resume or UI |
| Explicitly cancel one delivery wait without claiming a file-job abort | [Cooperative cancellation](large-file-delivery-cancellation.md); handle drop does not cancel, result decides races, existing jobs keep resources, no cleanup |
| Verify the full controlled delivery path over actual HTTPS | [Native HTTPS delivery checks](large-file-delivery-https.md); disposable exact peers, independent outputs and cancellation, not deployed LAN or grant authority |
| Compose manifest and chunk endpoints from one trusted configuration | [Distribution router](large-file-distribution-router.md); one Source/workspace, independent budgets and private fallback, not live mounting or privilege isolation |
| Verify actual Source delivery through HTTPS | [Real Source HTTPS](large-file-source-https.md); explicit combined SQL/files/TLS cases, not deployed LAN, live mounting or Session issuance |
| Edit task contents such as notes, code or configuration | [Task payload editor](editor.md) |
| Configure an AI Agent host and adapt explicitly selected tools | [AI Agent SDK integration](ai-agent-integration.md) |
| Inspect the internal server content shape and exact snapshot checks | [Task content manifest](task-content-manifest.md) |
| Inspect separate content persistence and atomic editing | [Task content storage](task-content-store.md) |
| List private task metadata without reading its contents | [Task content metadata index](task-content-index.md) |
| Inspect the separate unmounted HTTP metadata list | [Task content index HTTP adapter](task-content-index-http.md) |
| Inspect current Session and content-byte checks in one transaction | [Session task content](session-task-content.md) |
| Inspect the explicitly constructed, unmounted manual HTTP boundary | [Task content HTTP adapter](task-content-http.md) |
| Select an exact configured definition through a separate unmounted HTTP adapter | [Catalogue task content HTTP adapter](catalog-task-content-http.md) |
| Publish text or read an exact revision through a separate unmounted router | [Text Artifact HTTP adapter](text-artifact-http.md) |
| Inspect strict draft import and pure publication mapping | [Task draft publication plan](task-draft-publication.md) |
| Inspect manual one-step Session publication and partial outcomes | [Session draft publication](session-task-draft-publication.md) |
| Publish a catalogue draft with a frozen definition and independent attempt | [Session catalogue draft publication](session-catalog-task-draft-publication.md) |
| Observe an unconfirmed attempt target without resuming it | [Draft target observation](session-task-draft-observation.md) |
| Observe a catalogue attempt target with its frozen complete definition | [Catalogue draft target observation](session-catalog-task-draft-observation.md) |
| Export reported progress as data without restoring an attempt | [Draft metadata checkpoints](session-task-draft-checkpoint.md) |
| Inspect one checkpoint target with separately supplied current authority | [Explicit checkpoint target inspection](session-task-draft-inspection.md) |
| Register and read an immutable intent without executing publication | [Session draft intent records](session-task-draft-intent.md) |
| Publish one resource with a prior journal step and a shared completion transaction | [Journaled Session draft publication](session-task-draft-dispatch.md) |
| Inspect recorded journal progress without recovering an attempt | [Session draft journal inspection](session-task-draft-journal-inspection.md) |
| Observe one recorded step's resource while retaining the original journal pins | [Journal step observation](session-task-draft-journal-observation.md) |
| Run or teach the existing eBPF workflow | [Student guide](student-guide.md), [teacher guide](teacher-guide.md), labs below |
| Deploy or operate a trusted instance | [Security](security.md), [classroom connection and SSH](classroom-connection.md), [Runner Agent](runner-agent.md), [troubleshooting](troubleshooting.md) |
| Develop or verify a change | [Contributor guide](../../CONTRIBUTING.md), [current testing guide](testing-guide.md), [SDK](../../sdk-js/README.md), [tools](../../scripts/README.md) |
| Inspect past evidence | [Development history](development-history.md), [historical acceptance](acceptance.md), [historical test network](testing-network.md), [0.3.8 functional baseline](functional-network.md) |

## Platform contracts and implementation references

Read the conceptual model before selecting an adapter. Owner filtering in a direct store is not
authentication. Public teaching APIs still use the existing runtime; general Session commands are
not automatically HTTP endpoints or a replacement for live login.

| Boundary | Design and implementation references |
|---|---|
| Foundation and scoped identity | [Foundation](collaboration-foundation.md), [identity registry](collaboration-identity-store.md), [membership and policy](collaboration-access-store.md) |
| Audited lifecycle and commands | [Policy audit](collaboration-policy-audit.md), [identity lifecycle](collaboration-identity-lifecycle.md), [current-Session commands](collaboration-session-commands.md) |
| Durable authentication | [Account and Session source](collaboration-auth-source.md), [account deletion](collaboration-account-deletion.md), [password change](collaboration-password-change.md) |
| Explicit provisioning and observation | [Fresh bootstrap](collaboration-bootstrap.md), [local provisioning CLI](collaboration-provisioning.md), [read-only reconciliation](collaboration-reconciliation.md) |
| Tasks and domain definitions | [Task/domain separation](task-domain-boundary.md), [task instances](task-instance-store.md), [catalogue admission](session-catalog-tasks.md) |
| Immutable content | [Artifact revisions](artifact-revision-store.md), [private Artifact commands](session-artifact-commands.md) |
| Task content access and changes | [Private manual tasks](session-task-commands.md), [exact inputs](session-task-inputs.md), [Draft input replacement](session-task-revisions.md) |
| Judgment and history | [Review records](review-record-store.md), [authorized private human Reviews](session-review-commands.md) |

The preparation layers do not migrate existing teaching records. Task schema 2 is fresh-install-only;
a storage version is not the software version. Code is optional task content, not a mandatory Task
field. Publishing content, replacing a Task input, executing a Run and recording a Review are different
operations. None implies the others or grants cross-user access.

## Existing runtime reference

The teaching package is the first active domain, with its established teacher-managed deployment,
student learning history and kernel execution boundaries. Keep these operational guides separate from
general-platform commands that are not connected to the live runtime.

- [Learning storage and feedback](learning-storage.md) and [event streams and recovery](event-stream.md).
- [Desktop and LAN isolation target](classroom-isolation.md): a design boundary, not completed VM provisioning.
- [Root quick start and runtime API reference](../../README.md#quick-start).
- [Module manifests](../../modules/README.md): a declarative catalogue, not executable domain packages.
- [Troubleshooting](troubleshooting.md): distinguish local drafts, live runtime failures and preparatory-store failures.

## Recommended Reading Order

### Teacher

1. [Teacher Quick Start](teacher-guide.md)
2. [Project Status](project-status.md)
3. [System Architecture](architecture.md)
4. [Runner Agent Guide](runner-agent.md)
5. [Concept Map](concepts.md)
6. [Security and Classroom Deployment](security.md)
7. Browse all labs and perform a dry run first
   - Template path in the eBPF page:
     - `learning/foundations/beginner/fundamentals`
     - `learning/foundations/intermediate/protocols`
     - `learning-plus/cases/advanced/forensics`
     - `learning-plus/track/practice/operators`

### Student

1. [Student Quick Start](student-guide.md)
2. [Concept Map](concepts.md)
3. Complete labs in order:
   - [Lab 1: The Execution Pipeline](labs/01-first-program.md)
   - [Lab 2: Observing `execve`](labs/02-trace-execve.md)
   - [Lab 3: Counting with Maps](labs/03-map-counter.md)
   - [Lab 4: Passing Events via Ring Buffer](labs/04-ring-buffer.md)
   - [Lab 5: Verifier and Debugging](labs/05-verifier-debugging.md)
4. Read [Troubleshooting](troubleshooting.md) when needed

## Learning Objectives

After finishing this curriculum, learners should be able to:

- Explain the relationship among userspace, eBPF programs, kernel hooks, and verifier
- Choose hooks such as XDP and tracepoint according to scenario
- Use Maps to keep state and Ring Buffer to report events
- Understand why boundary checks, null checks, and bounded loops are required
- Locate common errors from clang and verifier output
- Safely detach programs and verify the environment is clean

## Run Modes

| Mode | Actual Kernel | Recommended Scene |
|---|---|---|
| WSL2 | WSL2 Linux kernel | Personal learning on Windows |
| Docker | Host Linux kernel or Docker Desktop VM kernel | Fast start, unified classroom environment |
| Native Linux | Local Linux kernel | Advanced labs, best compatibility |

eBPF always runs on a Linux kernel. In Docker on Windows/macOS, you observe the VM/host kernel,
not the desktop OS itself.

## Optional Runtime Tuning

For high event traffic classes, you can tune persistence queue alerting in `docker/.env` to control noise and debugging sensitivity:

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default: `true`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default: `80`)
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default: `40`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default: `10000`)

## CI and Merge Gate

- CI workflow now includes an aggregate gate job `ci-gate` in `.github/workflows/ci.yml`.
- `ci-gate` requires `security-audit`, `file-lengths`, `engine`, `frontend`, `sdk`, `permissions`, and
  `distribution`, and fails if any required job fails.
- For branch protection, enable required status check for **`CI gate`** on your main branch.
- An annotated version Tag triggers `Release Candidate Validation`, which binds the clean Tag commit to
  a newly built offline archive, runs exact-image installation plus live Aya attach/event/detach
  acceptance, and retains the result plus separately checksummed, candidate-bound kernel evidence as a
  30-day workflow artifact. A unified offline verifier streams the archive without extraction, rejects
  unsafe members, checks every package file, binds its metadata to that evidence, and only then performs
  non-overwriting manual extraction when requested. CI and Tag acceptance use this path instead of direct
  `tar` expansion. The workflow does not create or sign a GitHub Release.
