# Cyanrex project documentation

Cyanrex is moving from an eBPF teaching application to a self-hosted collaboration platform for
people, AI Agents and compute resources. The existing teaching runtime remains usable. General
identity, content, task and review services are explicitly configured preparation layers; the task
payload editor is local-only, and autonomous AI and general Run orchestration are not implemented.

This index describes the whole project, not only its first teaching domain. The source version is
**0.5.7** (**2026-10-12**), including private task metadata indexing, its unmounted HTTP adapter and
large-file preparation through current grants/sharing, authorized chunk transport and private assembly.
Earlier task decoupling, C2-O–AA, Next.js 15.5.27, AI host adapters and security boundaries remain in place.
Source inclusion does not connect generic browser saving, switch live authentication, migrate storage or
validate a deployment. Dated `Unreleased` descriptions retain their original implementation stage,
source versions and verification scope; they are not fresh 0.5.7 execution records.
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
projects that current-Session page at exact `GET /platform/v1/task-index`. It remains unmounted,
with strict raw pagination queries and private response-lifetime admission; existing content routers,
Session issuance, SDK/OpenAPI and browser behavior do not change.

The **0.5.6 [large-file preparation](large-file-distribution.md)** provides independently composed
streaming, staging, upload quotas, journaling, verification, publication and private metadata services.
Publisher/recipient contracts compare supplied snapshots; durable grant issuance, recovery and both
network transfer modes remain unconnected. Source inclusion is not a deployed distribution service.

## Start with your question

| What you need | Read first |
|---|---|
| Understand the project and its boundaries | [Architecture](architecture.md), [platform and teaching concepts](concepts.md) |
| See all current modules and connections | [Current platform feature map](platform-network.md) |
| Compare architecture, capabilities, implementations and maturity | [Capability tensor and evidence-backed scores](capability-maturity.md) |
| Know what is implemented and what comes next | [Project status](project-status.md), [target architecture](../zh-CN/next-architecture.md) |
| Review centralized large-file distribution and later desktop transfers | [Distribution proposal](large-file-distribution.md), [streaming core](large-file-core.md), [private chunk staging](large-file-staging.md), [upload intent and logical quotas](large-file-upload-intent.md), [local receipts](large-file-upload-staging.md), [write-ahead chunk journal](large-file-chunk-journal.md), [whole-byte observation](large-file-whole-verification.md), [persistent publication metadata](large-file-publication.md), [Session private metadata](large-file-session-metadata.md), [pure publisher/recipient contracts](large-file-access-contract.md); network transfers not connected |
| Inspect the Unreleased publisher-grant ledger and own Session reads | [Persistent publisher grants](large-file-publisher-grants.md); outside the 0.5.6 tag, not grant-management authorization or publication permission consumption |
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
