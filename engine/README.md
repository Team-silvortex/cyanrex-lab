# cyanrex-engine

Axum backend skeleton for Cyanrex.

## Planned modules

- `auth`
- `module-manager`
- `event-bus`
- `command-dispatcher`
- `ebpf-loader`

## Initial API

- `GET /`
- `GET /health`
- `POST /auth/login`
- `POST /auth/register`
- `POST /auth/totp/bootstrap`
- `GET /auth/me`
- `POST /auth/logout`
- `POST /auth/password/change`
- `POST /auth/delete`
- `GET /modules`
- `POST /modules/start`
- `POST /modules/stop`
- `GET /events`
- `POST /command`
- `POST /ebpf/run`
- `GET /helper/environment`

## Dev Auth Defaults

- username: `admin`
- password: `cyanrex-admin` (override with `CYANREX_ADMIN_PASSWORD`)
- TOTP secret (Base32): `JBSWY3DPEHPK3PXP` (override with `CYANREX_ADMIN_TOTP_SECRET`)

## Password Storage

- Passwords are never stored in plaintext.
- Engine uses per-user random salt + multi-round SHA-256 derivation.

## Persistent Auth Tables

- `users`
- `sessions`
- SQL migration template: `engine/migrations/0001_auth_users_sessions.sql`
