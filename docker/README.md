# docker

Run the full Cyanrex stack with Docker Compose.

## Services

- `postgres` on `127.0.0.1:15432`
- `engine` on `127.0.0.1:8080`
- `frontend` on `127.0.0.1:3000`

## eBPF Notes

- `engine` is an intentionally privileged teaching sandbox with host PID visibility.
- `memlock` is set to unlimited in compose to satisfy libbpf requirements.
- Host paths `/sys/fs/bpf` and `/lib/modules` are mounted for kernel/BPF integration.
- Loaded programs target the Docker host kernel on Linux, or the Docker Desktop VM kernel on Windows/macOS.
- Do not expose the Engine port to untrusted networks or allow untrusted users to submit programs.

## Start

```bash
./start.sh start
# explicit Docker backend:
./start.sh start --mode docker
# force rebuild when Dockerfile/dependencies changed:
./start.sh start --rebuild
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
