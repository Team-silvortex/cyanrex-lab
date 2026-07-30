# docker

Run the full Cyanrex stack with Docker Compose.

## Services

 - `postgres` on `127.0.0.1:15432` by default (`CYANREX_POSTGRES_PORT`)
 - `engine` on `127.0.0.1:8080` by default (`CYANREX_ENGINE_PORT`)
 - `frontend` on `127.0.0.1:3000` by default (`CYANREX_FRONTEND_PORT`)
- Multiple instances can run simultaneously by setting distinct IDs and ports:
  - `--instance-id room-a` and `--instance-id room-b`
  - `--engine-port`, `--frontend-port`, and `--postgres-port`
- Example quick assignment (avoid collisions):
  - room-a: `--instance-id room-a --engine-port 18080 --frontend-port 13000 --postgres-port 25433`
  - room-b: `--instance-id room-b --engine-port 18081 --frontend-port 13001 --postgres-port 25434`

## eBPF Notes

- `engine` is an intentionally privileged teaching sandbox with host PID visibility.
- `memlock` is set to unlimited in compose to satisfy libbpf requirements.
- Host paths `/sys/fs/bpf` and `/lib/modules` are mounted for kernel/BPF integration.
- Loaded programs target the Docker host kernel on Linux, or the Docker Desktop VM kernel on Windows/macOS.
- Do not expose the Engine port to untrusted networks or allow untrusted users to submit programs.

## Start

```bash
./start.sh start
# optional pre-flight check before classroom launch:
./scripts/check-instance-conflicts.sh --instance-id room-c --engine-port 18100 --frontend-port 13100 --postgres-port 25435
# explicit Docker backend:
./start.sh start --mode docker
# force rebuild when Dockerfile/dependencies changed:
./start.sh start --rebuild
./start.sh start --instance-id room-a --engine-port 18080 --frontend-port 13000 --postgres-port 15433
./start.sh status --instance-id room-a
./start.sh stop --instance-id room-a
```

The launcher creates `docker/.env` with random database, administrator, and TOTP secrets on the
first run. All published ports bind to loopback by default. For remote use, prefer an SSH tunnel or
a TLS reverse proxy. Do not set `CYANREX_BIND_ADDRESS=0.0.0.0` without a firewall and trusted-user
access controls.

For an existing installation that used the old development credentials, set
`CYANREX_ROTATE_ADMIN_CREDENTIALS=true` in `docker/.env`, start the Engine once, then immediately
set it back to `false`. This invalidates existing administrator sessions.

If the primary registries are unavailable, the launcher retries with fallback image, Debian, and
Cargo mirrors. They can also be overridden explicitly with `ENGINE_RUST_IMAGE`,
`ENGINE_DEBIAN_IMAGE`, `ENGINE_APT_MIRROR`, and `ENGINE_CARGO_REGISTRY_INDEX`.
The frontend npm registry can be overridden with `FRONTEND_NPM_REGISTRY`.

## WSL2 Backend

For the clearest Windows teaching experience, clone the repository into the WSL Linux filesystem
(for example `~/cyanrex-lab`) and run:

```bash
./start.sh start --mode wsl
```

This runs the Engine against the WSL2 kernel and starts PostgreSQL with Docker. The launcher builds
the Engine as the current user, then requests `sudo` only for the Engine process because loading
eBPF programs requires elevated Linux privileges.

Before a lab, use the Environment Helper page to verify BTF, bpffs, bpftool, tracing, and memlock.
If those kernel capabilities are absent, update WSL first; a custom WSL kernel may be required for
advanced exercises.

## Check

```bash
docker compose -f docker/docker-compose.yml ps
curl http://localhost:8080/health
```

## Stop

```bash
docker compose -f docker/docker-compose.yml down
```
