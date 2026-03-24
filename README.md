# cyanrex-lab

Version: `0.06`

Cyanrex monorepo for eBPF experiments: Axum engine + Next.js dashboard + module utilities.

## Repository Layout

```text
cyanrex-lab/
├ frontend/        # Next.js UI
├ engine/          # Axum backend
├ sdk-js/          # cyanrex-js SDK scaffold
├ modules/         # module examples
│  ├ module-ebpf
│  ├ module-network
│  └ module-protocol
├ scripts/         # saved experiment scripts
├ docker/          # compose and container assets
└ start.sh         # unified launcher
```

## Current Capabilities

- TDD-first backend workflow (`engine/tests/routes_tdd.rs`)
- Axum API server with:
  - module control endpoints
  - eBPF run pipeline endpoint (`/ebpf/run`)
  - eBPF template catalog (`/ebpf/templates`)
  - eBPF attachment endpoints (`/ebpf/attachments`, `/ebpf/attachments/details`, `/ebpf/detach`)
  - eBPF kernel trace stream (`/ws/events`, plus `/events` snapshot)
  - eBPF attach diagnostics events (`ebpf.attach_verified / ebpf.attach_missing / ebpf.attach_not_applicable`)
  - helper environment check endpoint (`/helper/environment`)
  - user script endpoints (`/scripts`, `/scripts/save`, `/scripts/delete`)
  - event settings endpoint (`/settings/events`)
  - C header module endpoints (catalog/download/delete/select/inject metadata)
- Auth system:
  - register/login/logout/session (`HTTP cookie`)
  - OTP/TOTP verification
  - OTP bootstrap QR flow
  - password change via `current_password + otp`
  - account deletion via `password + otp`
- Password security:
  - no plaintext storage
  - per-user random salt + multi-round SHA-256 derivation
- Auth persistence:
  - PostgreSQL-backed `users` + `sessions`
  - fallback to in-memory if DB temporarily unavailable
- Frontend pages:
  - `/dashboard`, `/ebpf`, `/helper`, `/modules`, `/events`, `/terminal`
  - `/login`, `/register`, `/otp-setup`, `/account`
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

### 1) Start services

```bash
./start.sh
```

Useful commands:

```bash
./start.sh start --local
./start.sh start              # fast-start (default, no forced rebuild)
./start.sh start --rebuild    # force rebuild when deps/Dockerfile changed
./start.sh start --pull       # pull base images before start
./start.sh status
./start.sh logs
./start.sh stop
```

### 2) Open UI

- Frontend: `http://localhost:3000`
- Engine health: `http://localhost:8080/health`
- Postgres: `localhost:15432`

### 3) Default dev account

- username: `admin`
- password: `cyanrex-admin`
- TOTP secret (Base32): `JBSWY3DPEHPK3PXP`

You can override with environment variables:

- `CYANREX_ADMIN_USERNAME`
- `CYANREX_ADMIN_PASSWORD`
- `CYANREX_ADMIN_TOTP_SECRET`

## Auth API (Implemented)

- `POST /auth/register`
- `POST /auth/login`
- `POST /auth/totp/bootstrap`
- `GET /auth/me`
- `POST /auth/logout`
- `POST /auth/password/change` (requires auth session + OTP)
- `POST /auth/delete` (requires auth session + OTP)

## eBPF APIs (Implemented)

- `POST /ebpf/run`
  - accepts optional `program_name` and `template_id`
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

## Scripts APIs (Implemented)

- `GET /scripts` (user-scoped list)
- `POST /scripts/save` (user-scoped create)
- `POST /scripts/delete` (user-scoped delete)

## Auth Persistence

- Tables: `users`, `sessions`
- Migration template: `engine/migrations/0001_auth_users_sessions.sql`

## Data Persistence

- `event_records` for event center (`engine/migrations/0002_event_records.sql`)
- `user_scripts` for script storage (`engine/migrations/0003_user_scripts.sql`)

## Engineering Rule

- This repo is TDD-first.
- Flow: `Red -> Green -> Refactor`.
- For backend route changes, update tests in `engine/tests/` first.
- See `TDD.md` for team conventions.
