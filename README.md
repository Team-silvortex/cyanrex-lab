# cyanrex-lab

Cyanrex monorepo scaffold.

## Repository Layout

```
cyanrex-lab/
├ frontend/        # Next.js app
├ engine/          # Axum backend
├ sdk-js/          # cyanrex-js SDK
├ modules/         # module examples
│  ├ module-ebpf
│  ├ module-network
│  └ module-protocol
├ scripts/         # saved experiment scripts
└ docker/          # docker compose and container assets
```

## Engineering Rule

- This repo is TDD-first from now on.
- Change flow: `Red -> Green -> Refactor`.
- For backend route changes, add/update tests in `engine/tests/` first.
- See `TDD.md` for details.

## Day 1 (Phase 0) Status

- monorepo skeleton created
- `engine` Axum routes scaffolded
- `frontend` Next.js starter files added
- `sdk-js` client wrapper scaffolded
- `docker-compose` for PostgreSQL added

## Next Step

Start Phase 1 by implementing engine internals (`module-manager`, `event-bus`, `command-dispatcher`) with persistent storage via `sqlx` + PostgreSQL.
