# Cyanrex eBPF Learning Handbook

Cyanrex is an eBPF teaching system for beginners. It brings source editing, clang diagnostics,
semantic completion, kernel loading, event observation, and program detach into one Web interface.

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
- `ci-gate` requires `security-audit`, `file-lengths`, `engine`, `frontend`, `permissions`, and
  `distribution`, and fails if any required job fails.
- For branch protection, enable required status check for **`CI gate`** on your main branch.
- An annotated version Tag triggers `Release Candidate Validation`, which binds the clean Tag commit to
  a newly built offline archive, runs exact-image installation plus live Aya attach/event/detach
  acceptance, and retains the result plus separately checksummed, candidate-bound kernel evidence as a
  30-day workflow artifact. A unified offline verifier streams the archive without extraction, rejects
  unsafe members, checks every package file, and binds its metadata to that evidence. The workflow does
  not create or sign a GitHub Release.
