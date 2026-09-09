# Linux Desktop and LAN Classroom Isolation

Decision date: 2026-09-06. This is the target architecture, not a deployment feature already shipped.
The current release is still a privileged local Engine with an optional compile-only Agent.

Authority clarification (2026-09-09): the teacher owns teaching policy and deployment management.
Personal use seeds the owner as a teacher; classroom use keeps the teacher-managed control instance as
the authority for identity, enrollment, records and trusted assessment. Students' personal teacher
accounts confer no authority on that classroom. This is not a peer-to-peer or student-elected control plane.

## Deployment assumptions

- No public Internet service is required. Multiple authenticated students submit their own eBPF source;
  authentication does not make that source trustworthy.
- Linux desktops may host the entire classroom: a teacher desktop can host the teaching service and
  student desktops can host their own experiment VMs. A dedicated server is optional.
- Desktop means a deployment location here; the existing frontend is reused. This decision does not
  introduce a new native desktop shell or require a second UI implementation.
- Linux and hardware virtualization are the planning baseline. Future installation must still check
  virtualization access, guest kernel/toolchain requirements, and available resources; failed checks
  must not silently select privileged host execution.

## Separate teaching from execution

The target control service owns identity, courses, saved source, learning records, and teacher feedback.
It must run without eBPF-loading privileges or host kernel mounts. PostgreSQL stays private to that
service; an execution VM receives no database credentials, browser session tokens, or shared enrollment
secret. A teacher-managed host supervisor provisions and resets VMs outside the student guest. It is a
separate technical privilege boundary, not a requirement for another human administrator or login role.

Each active student gets an exclusive Linux experiment VM. All untrusted compilation, completion,
program loading, attachment verification, and kernel event collection belong inside that VM. The
browser and control service must not interpret a guest attachment identifier as a local filesystem path.
Native host execution remains an explicit trusted personal-lab option, not classroom isolation.

A Linux desktop and a centralized virtualization host use the same ownership contract. Ordinary
containers do not supply a separate kernel, and one VM shared by the whole class does not separate
students from one another. Isolation still requires maintained host/guest kernels and a constrained
hypervisor configuration; a VM is not a guarantee against every escape.

If students administer their own desktop hosts, they can alter reported results. Such results are
learning records, not anti-cheating evidence. Trusted automated assessment must rerun the submission
on teacher-managed execution infrastructure.

## Environment ownership and recovery

1. Bind an environment to a server-authorized user, a unique environment ID, and a generation. A
   student's submitted ID or an Agent's self-reported `virtual_machine` label is not authorization.
2. Keep the environment assigned throughout the experiment, including after a run request finishes.
   Execution-capacity leases do not own the lifetime of attachments, events, or the VM itself.
3. Scope every command and result to the owner, environment generation, and operation. Reject stale
   credentials/results after reassignment, and persist ownership before allowing remote loading.
4. On disconnect, timeout, ambiguous cleanup, or lost control state, stop scheduling new work and
   quarantine the environment. Do not treat a missing inventory or a guest-reported successful detach
   as proof that the VM is safe for another student.
5. Before reassignment, rebuild/reset through the host supervisor, remove old writable state and
   credentials, and verify readiness. When no clean environment is available, queue or reject work;
   never fall back to the control host.

The managed supervisor must enforce CPU, memory, process, disk, event/output, and lifetime limits.
Its network policy permits only the necessary authenticated control channel and bounded lab traffic;
guests must not reach PostgreSQL, peer students, or unrelated LAN devices. Same-origin HTTPS for the
classroom frontend/API/WebSocket and authenticated encrypted node transport remain requirements on a
LAN. Per-node enrollment and revocation must not expose the current global Agent bootstrap secret to
student-controlled machines.

## Incremental implementation

The first preparatory increment routes run, attachment inventory, and detach through `RunnerDriver`.
The manager rejects an execution request whose owner differs from its lease owner. Inventory and
detach retain session-derived ownership, run independently of execution-slot leases, and have bounded
deadlines. A detach report comes from the selected driver; detach routes do not inspect pins on the control
host. Unavailable/unsupported operations return `503`, timeouts return `408`, and invalid detach
targets keep `400`; successful HTTP response shapes are unchanged.

The compiler increment adds owner-scoped checks/completion to the same driver. Two check slots and three
completion slots belong to each manager, with operation-wide 15/8-second deadlines (or a shorter Runner
timeout). Cancellation releases capacity and in-flight metrics. The local compiler uses owner-scoped
caches and private source workspaces with cleanup guards; no new remote compiler transport is enabled.

This is not a complete remote execution contract. Attach verification, kernel event streaming,
environment discovery, and compiler settings still have local paths. Selected-header metadata needs a
validated remote bundle contract before guest use. Next steps are to move those
behind the execution boundary, implement durable environment ownership and scoped node enrollment,
then add a VM supervisor/driver with disconnect, restart, stale-message, quota, and reset tests.

There is no selectable VM execution mode or automatic VM creation yet. Unknown Runner modes still
fail startup, `/ebpf/run` remains local, and the existing Agent must stay unprivileged and compile-only.
LAN ingress hardening is also pending: current Compose shares a bind setting across published services,
and the native/WSL launcher still explicitly listens on all interfaces. Do not expose the existing stack
as the isolated classroom design merely by changing its bind address.

See [System Architecture](architecture.md), [Security](security.md), and [Runner Agent](runner-agent.md).
