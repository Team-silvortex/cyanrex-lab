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

You can also tune event persistence behavior in `docker/.env` if needed:

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default: `true`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default: `80`)
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default: `40`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default: `10000`)

Local Runner capacity is configurable in the same file:

- `CYANREX_RUNNER_MODE` (currently supported: `local_process`)
- `CYANREX_RUNNER_MAX_CONCURRENT` (default: `2`, range: `1`–`32`)
- `CYANREX_RUNNER_MAX_PER_USER` (default: `1`, never above the global limit)
- `CYANREX_RUNNER_TIMEOUT_SECS` (default: `45`, range: `5`–`300`)
- `CYANREX_RUNNER_AGENT_TOKEN` (optional, empty/absent disables Agent registration; minimum 32 characters)
- `CYANREX_RUNNER_AGENT_TTL_SECS` (default: `30`; node becomes offline after this interval)
- `CYANREX_RUNNER_AGENT_RETENTION_SECS` (default: `300`; stale in-memory record retention)
- `CYANREX_RUNNER_AGENT_SIGNATURE_WINDOW_SECS` (default: `60`, range: `15`–`300`)

These are fairness and resource limits. The Docker Engine still reports `shared_kernel` and is not
a multi-tenant isolation boundary. An unsupported mode fails Engine startup instead of silently
falling back to local execution.

The Agent protocol issues a distinct credential at registration, then requires HMAC-SHA256 request
signatures with timestamp and one-use nonce. Its queue carries built-in probes and explicitly
submitted compile-only checks; it does not dispatch `/ebpf/run`, load eBPF, or return object files.
Treat registration and issued credentials as secrets, use TLS or a private management network, and
rotate by re-registering a node if it is lost or compromised.

### Standalone Runner Agent

The Engine image includes `cyanrex-runner-agent`. Copy `runner-agent.env.example`, mount the
bootstrap token as a read-only file, and override the image entrypoint:

```bash
cp docker/runner-agent.env.example runner-agent.env
chmod 600 runner-agent.env agent-token
docker run --rm --name cyanrex-runner-agent \
  --user "$(id -u):$(id -g)" \
  --read-only --security-opt no-new-privileges \
  --pids-limit 64 --memory 1536m --cpus 1 \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=128m \
  --entrypoint cyanrex-runner-agent \
  --env-file runner-agent.env \
  --mount type=bind,src="$PWD/agent-token",dst=/run/secrets/cyanrex-agent-bootstrap-token,ro \
  cyanrex/cyanrex-engine:0.2.0
```

Compile checking is disabled by default and forbidden with `shared_kernel`. Even when enabled, the
Agent must run without `--privileged`, host PID mode, kernel mounts, or extra capabilities. See the
Runner Agent guide in the frontend learning center for Linux and WSL2 instructions.

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

For packaged deployments produced by `scripts/package-distribution.sh`, use the generated helpers:

```bash
cd /path/to/cyanrex-lab-<version>-<build-id>
cp .env.example .env   # fill secure credentials first
./run.sh               # same as ./deploy.sh up
./deploy.sh status     # show running services
./deploy.sh logs       # follow logs
./deploy.sh down       # stop services
```

The packaged deploy helper validates secrets, docker daemon readiness, and engine startup health by
default.

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
docker compose -f docker/docker-compose.yml ps   # or: docker-compose -f docker/docker-compose.yml ps
curl http://localhost:8080/health
```

## Stop

```bash
docker compose -f docker/docker-compose.yml down   # or: docker-compose -f docker/docker-compose.yml down
```
