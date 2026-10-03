# Cyanrex project documentation

Cyanrex is moving from an eBPF teaching application to a self-hosted collaboration platform for
people, AI Agents and compute resources. The existing teaching runtime remains usable. General
identity, content, task and review services are explicitly configured preparation layers; the task
payload editor is local-only, and autonomous AI and general Run orchestration are not implemented.

This index describes the whole project, not only its first teaching domain. The source version is
0.5.0, including C2-K–N content preparation; source-release contents, live runtime capabilities and target architecture are distinguished
in the documents below. A built module or passing test does not imply an online deployment.

## Start with your question

| What you need | Read first |
|---|---|
| Understand the project and its boundaries | [Architecture](architecture.md), [platform and teaching concepts](concepts.md) |
| See all current modules and connections | [Current platform feature map](platform-network.md) |
| Compare architecture, capabilities, implementations and maturity | [Capability tensor and evidence-backed scores](capability-maturity.md) |
| Know what is implemented and what comes next | [Project status](project-status.md), [target architecture](../zh-CN/next-architecture.md) |
| Edit task contents such as notes, code or configuration | [Task payload editor](editor.md) |
| Inspect the internal server content shape and exact snapshot checks | [Task content manifest](task-content-manifest.md) |
| Inspect separate content persistence and atomic editing | [Task content storage](task-content-store.md) |
| Inspect current Session and content-byte checks in one transaction | [Session task content](session-task-content.md) |
| Inspect the explicitly constructed, unmounted HTTP boundary | [Task content HTTP adapter](task-content-http.md) |
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
