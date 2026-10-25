# Teaching runtime and deployment reference

[简体中文](../zh-CN/runtime-reference.md) · [Documentation](README.md) · [Project home](../../README.md)

Use this reference for the existing teaching application: source startup, offline installation,
teacher accounts, Engine APIs and release validation. Commands run from the repository root unless
a section explicitly selects an extracted package directory.

The independent low-privilege platform host has its own [startup guide](platform-loopback-host.md).
Starting the teaching application does not mount every platform adapter, migrate collaboration
storage or connect browser task saving. For current capability boundaries, read [project status](project-status.md).

## Quick Start

Start with the [teacher guide](teacher-guide.md) or [student guide](student-guide.md) for the existing teaching workflow.

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
./start.sh start --debug      # safe debug notice; commands are not traced
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

Use `/ebpf` and `/learn` for the existing teaching workflow. In the current development tree,
`/tasks/new` opens the local task payload editor (`/editor` is its compatibility entry). Read the
[editor guide](editor.md) before relying on persistence: it does not save to the server.
Starting the application does not initialize or migrate the separate collaboration stores.

### 2.5) Build a distribution package

For classroom deployment or offline distribution, create a packaged artifact with prebuilt Docker images:

```bash
package_version="$(awk -F'"' '/^version = / { print $2; exit }' engine/Cargo.toml)"
./scripts/package-distribution.sh --version "$package_version"
```

This produces:

- `dist/cyanrex-lab-<version>-<timestamp>.tar.gz`
- `dist/cyanrex-lab-<version>-<timestamp>.tar.gz.sha256`

The archive contains the PostgreSQL, Engine, and frontend images. On a disposable Docker host,
change into the freshly extracted package directory and verify it end to end with `./install-smoke.sh`. It checks the package
manifest and `release-metadata.json`, loads the offline images with registry pulls disabled, generates
temporary secrets, starts every required service and the optional compiler Agent, runs authenticated
probes, then removes its temporary stack and volume. The release metadata records the source revision
and clean/dirty state, matching annotated version Tag when present, image references/content IDs/build
mode, and the image archive SHA-256 without exposing a build-host path. Installation acceptance pins
the packaged image references and verifies all three loaded Docker content IDs before probing services.
The metadata is checksum-bound but not signed. The smoke test refuses to overwrite an existing `.env`.
Installation acceptance uses the bundled Rust CLI for package validation, loaded-image identity checks,
and Engine health checks; its default packaged path no longer requires Python on the Docker host.
Set `CYANREX_SMOKE_RUN_LIVE_KERNEL=1` only on a disposable privileged Linux host to additionally run
the packaged `live-kernel-smoke.sh`: it attaches the built-in Aya `sched_switch` ring-buffer program,
requires a uniquely identified real kernel event, detaches the exact pin, and rejects residual attachments.
New packages run this flow through the adjacent native `cyanrex-release smoke live-kernel` client; the
script retains its shell/Python path for compatibility with older or manually assembled packages.
Set `CYANREX_KERNEL_SMOKE_REPORT=/safe/output/report.json` to atomically write machine-readable evidence;
the script refuses to overwrite an existing report.

In that same extracted package directory, verify the report against its exact package metadata:

```bash
./cyanrex-release evidence verify /safe/output/report.json \
  --release-metadata ./release-metadata.json
```

New offline packages carry this native Rust verifier without standalone Python release helpers. The
Python implementations remain in source checkouts as compatibility and cross-implementation references.

For an artifact downloaded from the Tag workflow, place its four files in a dedicated directory and
verify the complete candidate from a trusted checkout of the matching source Tag before extracting it:

```bash
release_tag="v<version>" # choose the exact annotated Tag you intend to install
release_version="${release_tag#v}"
release_revision="$(git rev-list -n 1 "$release_tag")"
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  candidate verify /path/to/downloaded-candidate \
  --expect-version "$release_version" --expect-revision "$release_revision" --expect-tag "$release_tag" \
  --extract-to /path/to/new-output-directory
```

This streams the archive without extracting it, rejects links, path traversal, duplicate members, and
unchecksummed payloads, then binds the exact archived metadata to the separately checksummed live-kernel
evidence. Extraction starts only after the complete candidate passes, writes only regular files into a
private temporary directory, preserves script execution bits, and refuses an existing output path.
Until release signing is configured, these checks prove bundle consistency rather than publisher
authenticity.

For a locally built two-file package that has no Tag acceptance report, use the same verified extraction
path without the evidence step from a trusted source checkout:

```bash
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  package extract /path/to/archive-and-checksum \
  --output /path/to/new-output-directory
```

The previous `scripts/release-package.py` entry point remains temporarily available for compatibility.

Usage on target machine:

```bash
cd /path/to/new-output-directory/cyanrex-lab-*
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

### 3) Private deployment teacher account

- username: `admin`
- role: `teacher` — personal use makes you the teacher; teaching and deployment share one identity
- password and TOTP secret: generated on first start in `docker/.env`
- `docker/.env` is mode `0600` and ignored by Git; do not publish it

You can override with environment variables before the first start:

- `CYANREX_ADMIN_USERNAME` (direct Engine startup; supported launchers retain the username `admin`)
- `CYANREX_ADMIN_PASSWORD`
- `CYANREX_ADMIN_TOTP_SECRET`
- `CYANREX_ADMIN_USERNAMES` (legacy optional teacher-authority allowlist, comma/space-separated)
- `CYANREX_TEACHER_USERNAMES` (optional full teaching/deployment authority for verified existing accounts)
- `CYANREX_ALLOW_MISSING_ORIGIN` (optional, default: disabled) — allow CSRF-protected state-changing routes without `Origin`/`Referer`
- Event persistence tuning (optional):
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default `true`)
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default `80`)
  - `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default `40`)
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default `10000`)

Registration and password-only TOTP bootstrap are disabled by default. Set
`CYANREX_ALLOW_REGISTRATION=true` only for a supervised lab. Core eBPF and
user scripts are available to authenticated users. Teachers manage modules, compiler settings, Runner
Agents, teaching and Terminal; students cannot use those management routes. Existing administrator
accounts/configuration remain compatible, but login and session roles now report `teacher`.

Single-person use needs only the seeded teacher account, not public registration or an administrator
switch. Public registration rejects reserved teacher/legacy-admin names and cannot choose a role.
The deployment teacher cannot delete itself. Before upgrading, review both allowlists: previous
teachers now gain full deployment authority. Upgrade frontend and Engine together; see the
[teacher guide](teacher-guide.md) for appointment, ownership and isolation limits.

Module directories opt into discovery with a v1 `module.json` manifest. Engine startup rejects
malformed manifests and lifecycle requests reject unknown names. Start/stop only update the
single-process control state; Cyanrex never executes files from a module directory. Override the
catalog root with `CYANREX_MODULES_DIR` when running outside the repository or packaged image.

## Auth API (Implemented)

- `POST /auth/register`
- `POST /auth/login`
  - response includes `role` (`teacher` / `student`; `admin` retained as a legacy schema value)
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
- `GET /runner/overview` — teacher-only active lease owners and deadlines
- `POST /runner/agent/register` — optional token-authenticated remote node registration
- `POST /runner/agent/heartbeat` — health and capacity heartbeat for a registered node
- `GET /runner/agents` — teacher-only remote node inventory
- `POST /runner/jobs/probe`, `/runner/jobs/compile-check`, `/runner/jobs/cancel` — teacher-only remote job lifecycle
- `POST /runner/agent/jobs/claim`, `/sync`, `/result` — signed Agent job protocol
- `GET /runner/jobs` — teacher-only job inventory
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
- Teachers can inspect Agent health/capacity and probe or cancel recent jobs from **Deployment & settings → Runner Agent Operations**

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
- `POST /learning/teacher/feedback` — teacher/admin saves a student-visible comment on an existing
  attempt; requires CSRF origin and `{ username, attempt_id, comment, expected_revision }`
- Feedback is plain text (1–2000 characters after trimming), with server-assigned reviewer and time.
  Use revision `0` for a first comment, then the last read revision; stale edits return `409`.
  Feedback is persisted with the attempt and never changes automatic completion or the submitted source.
- Students can review their own submissions and teacher feedback in **Learn → My lab history**.
- `/ebpf/run` records an attempt only when a known `lab_id` is supplied; completion is calculated
  from the real run, required template/source patterns, and attachment verification.

## API Contract

- `GET /openapi.json` serves the versioned OpenAPI 3.1 document without authentication.
- [`engine/openapi/openapi.json`](../../engine/openapi/openapi.json) is generated from the registered Axum
  routes plus maintained request/response schemas.
- The quality gate rejects route or access-tier drift between the Engine and OpenAPI document, plus
  unexpected JavaScript SDK coverage or generated-model drift.
- SDK consumers can import generated schemas from `@cyanrex/sdk-js/openapi` and operation metadata/types
  from `@cyanrex/sdk-js/operations` after packaging.
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

These tables belong to the existing runtime, not the explicit durable collaboration source. Its
namespace installation, account incarnations and current-Session transactions are documented in
[durable authentication](collaboration-auth-source.md); do not point preparatory installers
at an existing deployment or assume the stores are automatically synchronized.

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
./scripts/quality-gate.sh
```

`--format-only` runs common preflight and formatting, not application tests. See the
[testing guide](testing-guide.md) for default, disposable-database, browser, SDK and privileged
acceptance boundaries. The [historical network](functional-network.md) and its
[JSON snapshot](../functional-network.json) are preserved evidence, not today's complete inventory.

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

### Tag candidate acceptance

Pushing a new annotated `v<major>.<minor>.<patch>` Tag runs `Release Candidate Validation`. After
validating the immutable version tree, it builds the offline archive from that exact commit, requires
`release-metadata.json` to report the same Tag/revision with a clean source and locally built images,
runs `install-smoke.sh` against the extracted archive (including loaded-image content ID checks), and
enables its privileged live-kernel attach/event/detach acceptance before uploading the accepted archive
plus outer checksum as a 30-day workflow artifact. The artifact also retains a separately checksummed
live-kernel report bound to the candidate metadata, kernel environment, unique event, and exact cleanup;
verified extraction replaces direct archive expansion in CI and Tag acceptance. The unified candidate
verifier streams and checks the complete four-file artifact before upload and can safely extract it after
download. This does not create a GitHub Release or sign/publish packages.
