# cyanrex-engine

Axum backend for authentication, module orchestration, event streaming, and eBPF experiments.

The composition root is split by responsibility: `application.rs` owns HTTP and access tiers,
`state.rs` wires services, and route handlers call the domain services under `services/`. See the
[system architecture](../docs/en/architecture.md) for dependency and deployment boundaries.

## Implemented services

- Cookie sessions, account management, and TOTP verification
- PostgreSQL persistence with an in-memory availability fallback
- Module lifecycle and command dispatch
- Persistent user-scoped event storage and WebSocket streaming
- eBPF compilation, loading, attachment inspection, event collection, and detach
- `bpftool` and Aya runtime backends
- User script storage and C header management

## API

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
- `POST /ebpf/check`
- `POST /ebpf/complete`
- `POST /ebpf/run`
- `GET /ebpf/templates`
- `GET /ebpf/attachments`
- `GET /ebpf/attachments/details`
- `POST /ebpf/detach`
- `GET /helper/environment`
- `GET /scripts`
- `POST /scripts/save`
- `POST /scripts/delete`
- `GET /settings/events`
- `POST /settings/events`
- `GET /ws/events`

## Development Credentials

- username: `admin`
- password and TOTP secret are generated on first start in `docker/.env`
- use `CYANREX_ADMIN_PASSWORD` and `CYANREX_ADMIN_TOTP_SECRET` to provide explicit values
- never commit or publish `docker/.env`

## Password Storage

- Passwords are never stored in plaintext.
- Engine uses Argon2 password hashes and can verify legacy hashes during migration.
- Persisted session tokens are stored as SHA-256 digests.

## Persistent Auth Tables

- `users`
- `sessions`
- SQL migration template: `engine/migrations/0001_auth_users_sessions.sql`

## Verification

Run the engine checks on Linux because Aya targets Linux kernel APIs:

```bash
cargo fmt --manifest-path engine/Cargo.toml -- --check
cargo test --manifest-path engine/Cargo.toml --locked
```
