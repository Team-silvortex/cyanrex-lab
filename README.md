# cyanrex-lab

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
  - helper environment check endpoint (`/helper/environment`)
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

## Quick Start

### 1) Start services

```bash
./start.sh
```

Useful commands:

```bash
./start.sh start --local
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

## Auth Persistence

- Tables: `users`, `sessions`
- Migration template: `engine/migrations/0001_auth_users_sessions.sql`

## Engineering Rule

- This repo is TDD-first.
- Flow: `Red -> Green -> Refactor`.
- For backend route changes, update tests in `engine/tests/` first.
- See `TDD.md` for team conventions.
