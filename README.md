# cyanrex-lab

Version: `0.3.1`

Cyanrex monorepo for eBPF experiments: Axum engine + Next.js dashboard + module utilities.

Architecture: [English](docs/en/architecture.md) · [简体中文](docs/zh-CN/architecture.md)
· Project status: [English](docs/en/project-status.md) · [简体中文](docs/zh-CN/project-status.md)

cyanrex-lab is free and open source under the [Apache License 2.0](LICENSE).
Contributions are welcome; start with [CONTRIBUTING.md](CONTRIBUTING.md), follow the
[community code of conduct](CODE_OF_CONDUCT.md), and report vulnerabilities through the
private process in [SECURITY.md](SECURITY.md).

## Repository Layout

```text
cyanrex-lab/
├ frontend/        # Next.js UI
├ engine/          # Axum backend
├ sdk-js/          # typed browser/Node.js API client
├ modules/         # versioned module catalog and manifest contract
│  ├ module-ebpf
│  ├ module-network
│  └ module-protocol
├ scripts/         # saved experiment scripts
├ docs/zh-CN/      # Chinese course and lab manual
├ docker/          # compose and container assets
└ start.sh         # unified launcher
```

The browser is the control plane, while the privileged Rust Engine is the execution plane. The
Engine owns authentication, authorization, persistence, compilation, eBPF loading, and event
streaming. See the architecture document before adding a service, route, or deployment mode.

## Current Capabilities

- TDD-first backend workflow (`engine/tests/routes_tdd.rs`)
- Axum API server with:
  - versioned module discovery and catalog-backed control endpoints
  - eBPF run pipeline endpoint (`/ebpf/run`)
  - compile-only clang diagnostics (`/ebpf/check`)
  - semantic C/eBPF completion (`/ebpf/complete`)
  - eBPF template catalog (`/ebpf/templates`)
  - eBPF attachment endpoints (`/ebpf/attachments`, `/ebpf/attachments/details`, `/ebpf/detach`)
  - eBPF kernel trace stream (`/ws/events`, plus `/events` snapshot)
  - eBPF attach diagnostics events (`ebpf.attach_verified / ebpf.attach_missing / ebpf.attach_not_applicable`)
  - helper environment check endpoint (`/helper/environment`)
  - user script endpoints (`/scripts`, `/scripts/save`, `/scripts/delete`)
  - event settings endpoint (`/settings/events`)
  - C header module endpoints (catalog/download/delete/select/inject metadata)
  - learning progress endpoints for student labs and teacher overview
  - structured admin command bus (`/command`) for module lifecycle and experiment handoff
  - generated OpenAPI 3.1 contract (`/openapi.json`) with Engine route/access and SDK coverage checks
- Auth system:
  - register/login/logout/session (`HTTP cookie`)
  - OTP/TOTP verification
  - OTP bootstrap QR flow
  - password change via `current_password + otp`
  - account deletion via `password + otp`
- Password security:
  - no plaintext storage
  - Argon2 password hashing with legacy-hash verification compatibility
  - persisted session tokens are stored as SHA-256 digests
- Auth persistence:
  - PostgreSQL-backed `users` + `sessions`
  - fallback to in-memory if DB temporarily unavailable
- Frontend pages:
  - `/dashboard`, `/ebpf`, `/learn`, `/teaching`, `/helper`, `/modules`, `/events`, `/terminal`
  - `/login`, `/register`, `/otp-setup`, `/account`
  - administrator-only Terminal with structured results, module snapshots, and session history
- JavaScript SDK:
  - typed ESM package covering the browser-facing Engine API
  - public request/response models generated from 76 OpenAPI component schemas
  - browser credentials, Node session-cookie capture, Origin-based CSRF support, cancellation, and typed errors
  - coverage checked against every non-Agent Engine operation in the generated API contract
  - dry-run package manifest, declaration-closure, and ESM consumer-import acceptance checks
- Frontend i18n:
  - supported languages: Simplified Chinese (`zh-CN`), English (`en`), Spanish (`es`), Japanese (`ja`)
  - sidebar + auth pages + core runtime pages integrated
  - language preference persisted in browser local storage
- Event center:
  - user-scoped persistent event storage
  - category split: `kernel` / `platform`
  - severity + color: success=green, warning=yellow, error=red
  - sidebar unread badge (red dot + count)
  - export by filters (`/events/export`)
  - delete by same filters (`/events/delete`)
  - per-user event retention settings:
    - max retained records
    - overflow policy: `drop_oldest` / `drop_new`
- Page state persistence (sessionStorage):
  - helper report cache
  - events filter state
  - eBPF editor and runtime controls

## Quick Start

中文教程入口：[Cyanrex eBPF 教学手册](docs/zh-CN/README.md)。教师建议先阅读
[教师快速开始](docs/zh-CN/teacher-guide.md)，学生从
[学生快速开始](docs/zh-CN/student-guide.md)进入课程。

### 1) Start services

```bash
./start.sh
```

Useful commands:

```bash
./start.sh start --mode auto   # WSL2-native inside WSL; Docker elsewhere
./start.sh start --mode wsl    # Engine targets the WSL2 kernel
./start.sh start --mode docker # Engine targets the Docker host/VM kernel
./start.sh start --mode native # Engine targets the native Linux kernel
./start.sh start --local
./start.sh start              # fast-start (default, no forced rebuild)
./start.sh start --debug      # enable detailed startup tracing
./start.sh start --rebuild    # force rebuild when deps/Dockerfile changed
./start.sh start --pull       # pull base images before start
./start.sh start --skip-conflict-check  # skip preflight port/project conflict check
./start.sh start --instance-id room-a    # same instance starts are auto-locked by instance ID
./start.sh start --skip-start-lock       # bypass start-lock when debugging only
./start.sh diagnose          # run system-level diagnostic checks
./start.sh status
./start.sh logs
./start.sh stop
```

The eBPF Engine always runs on a Linux kernel. On Windows, use WSL2-native mode for direct access
to the WSL kernel, or Docker mode for a disposable teaching sandbox. Docker mode is intentionally
privileged and must not be exposed to untrusted users.

### 2) Open UI

- Frontend: `http://localhost:3000`
- Engine health: `http://localhost:8080/health`
- Postgres: `127.0.0.1:15432` (not exposed to the LAN)

### 2.5) Build a distribution package

For classroom deployment or offline distribution, create a packaged artifact with prebuilt Docker images:

```bash
./scripts/package-distribution.sh --version 0.3.1
```

This produces:
- `dist/cyanrex-lab-0.3.1-<timestamp>.tar.gz`
- `dist/cyanrex-lab-0.3.1-<timestamp>.tar.gz.sha256`

The archive contains the PostgreSQL, Engine, and frontend images. On a disposable Docker host,
verify the freshly extracted package end to end with `./install-smoke.sh`. It checks the package
manifest, loads the offline images with registry pulls disabled, generates temporary secrets, starts
every required service and the optional compiler Agent, runs authenticated probes, then removes its
temporary stack and volume. It refuses to overwrite an existing `.env`.

Usage on target machine:

```bash
tar -xzf cyanrex-lab-*.tar.gz
cd cyanrex-lab-*
cp .env.example .env   # update credentials/bind settings as needed
./run.sh               # start
./stop.sh              # stop
```

The package now includes a richer `deploy.sh` helper:

```bash
./deploy.sh up     # start (default behavior)
./deploy.sh status # show service status
./deploy.sh logs   # follow logs
./deploy.sh down   # stop services
```

`run.sh` and `stop.sh` remain compatibility wrappers.

If `.env` is missing or still contains placeholder values, startup will fail with an explicit
prompt to initialize secrets first.

### 3) Private development account

- username: `admin`
- password and TOTP secret: generated on first start in `docker/.env`
- `docker/.env` is mode `0600` and ignored by Git; do not publish it

You can override with environment variables before the first start:

- `CYANREX_ADMIN_USERNAME`
- `CYANREX_ADMIN_PASSWORD`
- `CYANREX_ADMIN_TOTP_SECRET`
- `CYANREX_ADMIN_USERNAMES` (optional, comma/space-separated; defaults to `CYANREX_ADMIN_USERNAME`)
- `CYANREX_TEACHER_USERNAMES` (optional, comma/space-separated)
- `CYANREX_ALLOW_MISSING_ORIGIN` (optional, default: disabled) — allow CSRF-protected state-changing routes without `Origin`/`Referer`
- Event persistence tuning (optional):
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default `true`)
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default `80`)
  - `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default `40`)
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default `10000`)

Registration and password-only TOTP bootstrap are disabled by default. Set
`CYANREX_ALLOW_REGISTRATION=true` only for a supervised lab. Core eBPF and
user scripts are available to authenticated users. Module browsing is available
to admin/teacher roles, while module modification and system settings remain
administrator-only.

Module directories opt into discovery with a v1 `module.json` manifest. Engine startup rejects
malformed manifests and lifecycle requests reject unknown names. Start/stop only update the
single-process control state; Cyanrex never executes files from a module directory. Override the
catalog root with `CYANREX_MODULES_DIR` when running outside the repository or packaged image.

## Auth API (Implemented)

- `POST /auth/register`
- `POST /auth/login`
  - response includes `role` (`admin` / `teacher` / `student`) in addition to username/session fields
- `POST /auth/totp/bootstrap`
- `GET /auth/me`
  - response includes current authenticated `role`
- `POST /auth/logout`
- `POST /auth/password/change` (requires auth session + OTP)
- `POST /auth/delete` (requires auth session + OTP)

## eBPF APIs (Implemented)

- `POST /ebpf/check`
  - compile-only clang syntax check; never loads code into the kernel
  - returns structured line/column diagnostics
- `POST /ebpf/complete`
  - clang semantic completion at a one-based cursor position
  - returns header symbols, macros, types, functions, and structure fields
- `POST /ebpf/run`
  - accepts optional `program_name`, `template_id`, and `lab_id`
  - accepts optional `runtime_backend` (`bpftool` | `aya`)
  - supports `sampling_per_sec` to control kernel event sampling rate
  - supports `stream_seconds` to control stream duration
  - supports `enable_kernel_stream` toggle
  - kernel stream prefers `ringbuf event_pipe` and falls back to `tracelog`
  - `aya` backend currently targets tracepoint attach path (first-class for sched switch sampler)
- `GET /ebpf/attachments`
- `GET /ebpf/attachments/details`
- `POST /ebpf/detach`
- `GET /ebpf/templates`
  - includes typical templates: `xdp`, `tracepoint`, `ringbuf skeleton`, `ringbuf high-freq sampler`
- `GET /events`
  - event snapshot
- `GET /ws/events`
  - realtime event stream
- `GET /settings/events`
  - get current user event retention settings
- `POST /settings/events`
  - update user event retention settings (`max_records`, `overflow_policy`)
- `GET /helper/environment`
  - runtime checks include `bpftool_autoattach`, `bpftool_link_show`, `btf_dump`, `bpffs_mount_type`, `runtime_context`

## Runner APIs (Implemented)

- `GET /runner/status` — authenticated capacity and explicit isolation level
- `GET /runner/overview` — administrator-only active lease owners and deadlines
- `POST /runner/agent/register` — optional token-authenticated remote node registration
- `POST /runner/agent/heartbeat` — health and capacity heartbeat for a registered node
- `GET /runner/agents` — administrator-only remote node inventory
- `POST /runner/jobs/probe`, `/runner/jobs/compile-check`, `/runner/jobs/cancel` — administrator-only remote job lifecycle
- `POST /runner/agent/jobs/claim`, `/sync`, `/result` — signed Agent job protocol
- `GET /runner/jobs` — administrator-only job inventory
- `GET /ebpf/check/backends` — authenticated, sanitized local and eligible Agent compiler inventory
- `POST`, `GET /ebpf/check/remote` and `POST /ebpf/check/remote/cancel` — user-scoped asynchronous remote diagnostics
- `/ebpf/run` executes through the replaceable `RunnerDriver` boundary
- Local Runner defaults to two global jobs, one job per user, and a 45-second execution timeout
- `shared_kernel` is reported explicitly; quotas do not replace per-student VM isolation
- Agent jobs use per-node HMAC credentials, replay protection, leases, capability matching, cancellation, and bounded results
- Remote diagnostics require an explicit editor selection and never silently fall back; local checking remains the default
- Optional compile jobs use restricted Clang and never load or return eBPF objects; `/ebpf/run` remote dispatch remains disabled
- `cyanrex-runner-agent` connects trusted Linux, WSL2, and unprivileged container nodes to this protocol
- `scripts/runner-agent.sh start` enables the optional hardened Compose Agent; the matching smoke script verifies the authenticated editor path
- Administrators can inspect Agent health/capacity and probe or cancel recent jobs from **Settings → Runner Agent Operations**

## Scripts APIs (Implemented)

- `GET /scripts` (user-scoped list)
- `POST /scripts/save` (user-scoped create)
- `POST /scripts/delete` (user-scoped delete)

## Learning APIs (Implemented)

- `GET /learning/labs` — current user's five-lab catalog and progress
- `GET /learning/attempts` — current user's backend-recorded run attempts
- `GET /learning/teacher/overview` — teacher/admin classroom progress summary
- `GET /learning/teacher/attempts?username=...&limit=...` — staff-only bounded attempt,
  feedback, and submitted-source review for one student
- `/ebpf/run` records an attempt only when a known `lab_id` is supplied; completion is calculated
  from the real run, required template/source patterns, and attachment verification.

## API Contract

- `GET /openapi.json` serves the versioned OpenAPI 3.1 document without authentication.
- [`engine/openapi/openapi.json`](engine/openapi/openapi.json) is generated from the registered Axum
  routes plus maintained request/response schemas.
- The quality gate rejects route or access-tier drift between the Engine and OpenAPI document, plus
  unexpected JavaScript SDK coverage or generated-model drift.
- SDK consumers can import the full generated map from `@cyanrex/sdk-js/openapi` after packaging.
- A frozen `0.3.0` compatibility baseline rejects removed operations, access changes, narrowed
  requests, and weakened successful-response guarantees while allowing additive evolution.

Regenerate and validate it after changing routes or wire models:

```bash
node scripts/generate-openapi.mjs
node scripts/openapi-contract.mjs
node scripts/api-compatibility.mjs
node scripts/generate-sdk-types.mjs
```

## Auth Persistence

- Tables: `users`, `sessions`
- Migration template: `engine/migrations/0001_auth_users_sessions.sql`

## Data Persistence

- `event_records` for event center (`engine/migrations/0002_event_records.sql`)
- `user_scripts` for script storage (`engine/migrations/0003_user_scripts.sql`)
- `learning_attempts` for source snapshots, automated feedback, and lab progress
  (`engine/migrations/0004_learning_attempts.sql`)

## Engineering Rule

- This repo is TDD-first.
- Flow: `Red -> Green -> Refactor`.
- For backend route changes, update tests in `engine/tests/` first.
- For API changes, regenerate the OpenAPI document and keep its component schemas current.
- Replace the compatibility baseline only as part of an explicitly reviewed compatibility reset.
- Maintained source files are limited to 600 lines; documentation files are limited to 2000 lines.
- See `TDD.md` for team conventions.

Run the same checks used by CI before submitting changes:

```bash
./scripts/quality-gate.sh --format-only
```

### CI Gate (`ci-gate`)

- Workflow `CI` now runs checks in parallel and then uses a final `ci-gate` job to aggregate results:
  - `security-audit`
  - `file-lengths`
  - `engine`
  - `frontend`
  - `sdk`
  - `permissions`
  - `distribution`
- Set branch protection to require only `ci-gate` (instead of each matrix-like job), then any failed job will fail the required gate.
- Recommended GitHub branch rule (GitHub UI):
  - Settings → Branches → Branch protection rules
  - Add rule for default branch (for example, `main`)
  - Enable **Require status checks to pass before merging**
  - Add required status check: `CI gate`
  - (Optional) keep each sub-job unchecked to reduce noise; `ci-gate` already aggregates failures.
