# Platform and eBPF Concepts

Cyanrex Lab now has two vocabularies: domain-neutral collaboration concepts and the eBPF concepts
used by its first working domain. This guide keeps them separate so that code editing, execution and
approval are not treated as the same action. Platform contracts and explicit backend preparation exist,
but the running teaching application still uses its established APIs and storage. The local task editor
is not connected to those backend stores. See [architecture](architecture.md) and the
[platform network](platform-network.md) for the implementation boundaries.

## Platform concepts

### Authority and workspace

An **Authority** identifies a durable instance authority. It is not a hostname, an HTTPS identity,
the `CYANREX_INSTANCE_ID` deployment label or permission on another instance. A **Workspace** groups
work and membership within an Authority; its reference carries both identities. Matching a local ID
across two instances does not make their objects or permissions equivalent.

These are platform preparation concepts. The existing teaching application has not become a public
multi-workspace service merely because the types and stores exist.

### Principal and Session

A **Principal** is the identity of a participant, not a username or a role. The contract distinguishes
human, agent, service and machine identities. Teacher/student are authority roles, not Principal kinds.
The `agent` kind reserves space for future AI participants; it does not mean AI behavior is implemented.

A **Session** proves a currently authenticated account context. The prepared durable source tracks
account incarnations, so deleting and recreating the same username must not inherit the old identity
or grants. Its command adapters verify Session, binding, membership and policy in one transaction.
Supplying a Principal ID, serialized role or old permission preview is not a substitute for that check.

### Membership and deployment authority

**Membership** connects a Principal to a Workspace, with active/suspended state and local role presets.
It is separate from an instance deployment grant. Being a workspace owner does not automatically grant
SSH credentials, privileged execution or deployment management.

The running teaching application continues to make the teacher both teaching and deployment authority;
the personal deployment account is a teacher by default. That convenience does not make another
user's private content public. The prepared private-work adapters also remain owner-only, including
when the caller is a teacher. Classroom review of teaching attempts is a separate, existing permission.

### Task definition and domain pack

A **TaskDefinition** describes an exact package/task version, evidence schema and assessment policy.
A built-in **domain pack** supplies definitions and domain-specific rules through a typed TaskProvider.
The shared TaskCatalog validates and looks up exact definitions; it does not assume all tasks contain
code, require a compiler or need a Run.

The eBPF teaching pack owns lab templates, source evidence and kernel-related rules. Other-domain test
fixtures demonstrate the shared boundary, not a shipped document-review workflow. A registered
definition is neither an executable plugin nor permission to execute. The `modules/` manifest catalogue
is a different feature and does not dynamically install TaskProviders.

### Task and payload

A **Task** is a work item: who owns it, what definition it uses if any, which exact inputs it refers to,
its current status and revision. Code can be part of its **payload**, but code is not the Task itself.
The current local editor lets a draft contain zero or more text items. The backend Task stores
Artifact references rather than embedding editor models or source text.

There are three distinct objects named draft: a browser TaskDraft, the backend creation input TaskDraft,
and a persisted TaskSnapshot in Draft status. Local IDs and item revisions cannot be sent as server
Artifact/Task identities. Exporting a local JSON draft is not publication or a server save.

Current stored statuses are Draft, Ready, InProgress, Blocked, InReview and Cancelled. There is no
Accepted/Done state. A revision is an optimistic concurrency check: a command must name the version
it expects, and a competing change makes that expectation stale. Task cancellation ends the work item,
not a running process or kernel attachment.

### Artifact and exact reference

An **Artifact** carries content independently of task type. Its stored revisions are immutable; changing
bytes creates a new revision, rather than overwriting the old one. An **ArtifactRef** pins workspace,
artifact ID, revision ID and SHA-256 digest. There is no floating `latest` reference. Identical digests
do not merge ownership or grant access to another artifact.

Stored metadata or successful decoding is not enough: authorized reads verify the referenced bytes.
Artifact publication and Draft Task input replacement are separate operations. A replacement checks
the expected Task revision and old/new inputs; it does not delete superseded content. A failed Task
update must not be treated as permission to remove a published revision. See [input replacement](session-task-revisions.md).

### Assessment and Review

An **Assessment** is a provider's in-process rule result, with Passed or NotPassed and feedback. It is
not a human signature, a durable Review or permission to accept a Task. Catalogue metadata admission
does not run those rules or turn arbitrary content into valid typed evidence.

A **ReviewRecord** is a judgment about pinned Artifact targets and evidence. Human judgments use
Approved, ChangesRequested or Commented; rule judgments are a separate form. Amendments preserve
earlier opinion revisions. The current Session path supports private human Review of owned artifacts,
not general cross-user review or AI-authored judgments. Content digest checks establish byte identity,
not that evidence is relevant, an assessment policy executed, or a Task should be accepted.

### Run and Runner Agent

A **Run** is the intended durable representation of an execution attempt, separate from the Task that
requested it. The platform currently has Run identity/reference contracts, not a general durable Run
store or scheduler. Existing eBPF runs, runtime reports, Runner leases and Agent jobs retain their
teaching-specific implementation; they are not automatically converted into platform Run records.

A **Runner Agent** is a remote computing node, not an AI participant. The current Agent can probe its
environment and optionally perform isolated compile-only checks; `/ebpf/run` remains local. A healthy
Agent registration or a lease does not prove a general task has completed, authorize review or add
kernel isolation to the local Engine.

### Business events and telemetry

An **EventEnvelope** describes a business fact using event identity, aggregate/revision, actor and
correlation fields. Task, Artifact and Review stores write their state and outbox records atomically.
This is storage-side preparation, not an implemented outbox delivery/replay service or an exactly-once
execution guarantee.

The existing **EventBus** carries bounded runtime telemetry and browser events, with asynchronous
persistence and explicit gap/fallback limits. Those events are not authenticated audit records and
cannot be promoted into business facts just because their text describes a successful operation.

### Editor language services and execution

The controlled text editor supplies local language support to a selected payload item. Its 14 language
profiles include richer bundled JavaScript/TypeScript, JSON, CSS and HTML workers, alongside basic
profiles for other languages. This is not a connected rust-analyzer, Pyright or clangd server.
Language assistance neither runs content nor grants runtime authority. The separate eBPF editor's
clang diagnostics and semantic services still use the authenticated teaching endpoints.

### Versions

The product release number, core contract schema, storage schema, definition/policy versions and
individual object revisions are independent. The source release is 0.4.9; the current
Task storage requires fresh schema 2 while its core contract schema remains 1. An old Task schema 1
namespace is rejected, not automatically migrated. A higher product version does not upgrade a
Task's frozen definition or move a Review to new content.

## eBPF teaching concepts

The following concepts describe the existing eBPF teaching domain. They remain useful for the live
editor, but do not become requirements of every platform Task or payload.

## 1. How an eBPF Program Runs

```text
C source
  -> clang compiles to BPF bytecode
  -> kernel verifier checks safety
  -> loader loads programs and maps
  -> attach to hook
  -> kernel event triggers the program
  -> Map/Ring Buffer/trace outputs data
  -> userspace reads and displays
```

In Cyanrex, the result panel splits this flow into compile, load, and attach stages.
When troubleshooting, identify which stage failed first.

## 2. Hook

A hook is where an eBPF program is invoked. `SEC("...")` declares program type and attach point.

- `SEC("xdp")`: early path in NIC receive path.
- `SEC("tracepoint/category/name")`: stable kernel tracepoint.
- `SEC("kprobe/function")`: dynamic kernel function probe, higher compatibility requirements.
- `SEC(".maps")`: map declaration, not an executable program.
- `SEC("license")`: program license.

## 3. Context

The kernel passes a context pointer when calling an eBPF program, for example `struct xdp_md *ctx` in XDP.
Allowed fields depend on program type. When you type `ctx->`, Cyanrex asks clang for actual fields.

## 4. Helper

eBPF cannot call arbitrary kernel functions and can only use helpers allowed by program type.

- `bpf_ktime_get_ns()`: read monotonic clock
- `bpf_get_current_pid_tgid()`: get process/thread ID
- `bpf_map_lookup_elem()`: map lookup
- `bpf_ringbuf_reserve()`: reserve ring buffer space

Some helper calls require GPL-compatible programs, therefore examples may declare GPL license.

## 5. Map

Maps are shared containers that connect kernel eBPF state and userspace.

- Hash: key-value storage
- Array: fixed index, predictable access cost
- Per-CPU Array: one copy per CPU to reduce lock contention
- Ring Buffer: ordered transfer of variable-size events

Map lookup can return `NULL`, so check pointers before dereference.

## 6. Verifier

The verifier performs static analysis to prove safety constraints. It does not “guess” whether code is probably safe.
Common requirements include:

- pointer provenance is known
- memory access ranges are provable
- map lookup and ring buffer reserve results are null-checked
- loops have explicit bounds
- all paths terminate
- helper signatures/types are correct

Writing eBPF is not about making code look correct; it is about proving correctness to the verifier.

## 7. CO-RE and BTF

BTF describes kernel types. `vmlinux.h` can be generated from the current kernel's BTF.
CO-RE relies on type and field metadata to reduce adaptation work across kernel versions,
but it does not guarantee every program runs on every kernel.
