# Runner Agent Guide

The standalone `cyanrex-runner-agent` connects a trusted Linux, WSL2, or container node to the
Engine control plane. Version 0.3.1 executes built-in `control_probe` jobs and can optionally run
compile-only `ebpf_compile_check` jobs. Compile checking is disabled by default. Neither mode
accepts shell commands or arbitrary executable payloads, and neither needs root or Linux
capabilities. A compile job never loads eBPF or returns its object file.

## Engine Preparation

For an external Agent, set a random bootstrap token of 32–512 characters on the Engine and restart
the Docker stack:

```bash
openssl rand -hex 32
# Store the output as CYANREX_RUNNER_AGENT_TOKEN in docker/.env.
./start.sh stop && ./start.sh start --mode docker
```

Keep this token off command lines, source control, screenshots, and frontend configuration. The
Agent uses it only to register. Registration returns a per-node credential that stays in process
memory; restarting the Agent registers again and rotates that credential.

Engine and Agent clocks must be synchronized because signed requests expire after 60 seconds by
default. Use TLS for traffic between hosts. Plain HTTP to a non-loopback address is rejected unless
the explicit lab-only override is enabled.

## Managed Docker Agent

The source tree and distribution package include an optional `runner-agent` Compose profile. It is
disabled during a normal stack start. After the main stack is configured, start it with:

```bash
./scripts/runner-agent.sh start
./scripts/runner-agent-smoke.sh
```

In a distribution package, use `./runner-agent.sh` and `./runner-agent-smoke.sh` from the package
root. The manager generates a token only when the control plane is not configured, stores it in the
private runtime environment, copies it to a mode-0600 Docker Secret, and recreates Engine once to
apply it. It never prints the token. Use `status`, `logs`, and `stop` actions for lifecycle control.

The managed Agent is a read-only, unprivileged container with all Linux capabilities dropped, no
published ports, a PID limit, and CPU/memory limits. Its writable `/tmp` is a small `noexec` tmpfs.
An internal control-only network connects it to Engine without access to the default PostgreSQL and
frontend network or external networks. It reports `container` isolation and enables compile-only
diagnostics; `/ebpf/run` remains local.
CI runs the same Agent client against a real loopback Engine and Linux Clang, while also validating
both source and distribution Compose profiles.

## Linux or WSL2

Build the Agent as an unprivileged user:

```bash
cargo build --release --locked \
  --manifest-path engine/Cargo.toml \
  --bin cyanrex-runner-agent
```

Create a token file readable only by the Agent account:

```bash
install -m 600 /dev/null ~/.cyanrex-agent-token
# Paste only the bootstrap token into that file.
```

Run it:

```bash
CYANREX_AGENT_ENGINE_URL=https://engine.lab.example \
CYANREX_AGENT_BOOTSTRAP_TOKEN_FILE="$HOME/.cyanrex-agent-token" \
CYANREX_AGENT_ID=lab-vm-01 \
CYANREX_AGENT_ISOLATION=virtual_machine \
./engine/target/release/cyanrex-runner-agent
```

To enable compile-only checks on an isolated node, install Clang with a BPF target and add:

```bash
CYANREX_AGENT_ENABLE_COMPILE_CHECK=true \
CYANREX_AGENT_CLANG_PATH=/usr/bin/clang \
CYANREX_AGENT_ISOLATION=virtual_machine \
./engine/target/release/cyanrex-runner-agent
```

Use `shared_kernel`, `container`, `virtual_machine`, or `dedicated_host` only when that value
truthfully describes the node boundary. The label is displayed to administrators; it does not
create isolation.

## External Container

The Engine image also contains `/usr/local/bin/cyanrex-runner-agent` and Clang. Run it without
`--privileged`, host PID mode, kernel mounts, or added capabilities:

```bash
docker run --rm --name cyanrex-runner-agent \
  --user "$(id -u):$(id -g)" \
  --read-only --security-opt no-new-privileges \
  --pids-limit 64 --memory 1536m --cpus 1 \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=128m \
  --entrypoint cyanrex-runner-agent \
  --env-file ./runner-agent.env \
  --mount type=bind,src="$PWD/agent-token",dst=/run/secrets/cyanrex-agent-token,ro \
  cyanrex/cyanrex-engine:0.3.1
```

Start from [`docker/runner-agent.env.example`](../../docker/runner-agent.env.example). Rebuild the
Engine image after updating the source. The Agent remains unprivileged even though the same image
can run the privileged Engine service. Set `CYANREX_AGENT_ISOLATION=container` when this container
is the real boundary. Compile checking is rejected when the Agent reports `shared_kernel`.

## Configuration

| Variable | Default | Meaning |
|---|---:|---|
| `CYANREX_AGENT_ENGINE_URL` | `http://127.0.0.1:8080` | Engine base URL without a path |
| `CYANREX_AGENT_BOOTSTRAP_TOKEN` | none | Direct bootstrap secret; prefer the file variant |
| `CYANREX_AGENT_BOOTSTRAP_TOKEN_FILE` | none | File containing only the bootstrap secret |
| `CYANREX_AGENT_ID` | `$HOSTNAME` | Stable 3–64 character node ID |
| `CYANREX_AGENT_ISOLATION` | `shared_kernel` | Truthful isolation descriptor |
| `CYANREX_AGENT_MAX_CONCURRENT` | `1` | Advertised capacity, range 1–32 |
| `CYANREX_AGENT_CAPABILITIES` | `control_probe` | Comma-separated capabilities; probe support is required |
| `CYANREX_AGENT_ENABLE_COMPILE_CHECK` | `false` | Opt in to bounded compile-only jobs; adds `clang_check` |
| `CYANREX_AGENT_CLANG_PATH` | `/usr/bin/clang` | Absolute Clang executable used without a shell |
| `CYANREX_AGENT_COMPILE_WORK_DIR` | system temp + `cyanrex-runner-agent` | Private disposable compiler work root |
| `CYANREX_AGENT_POLL_SECS` | `5` | Heartbeat and claim interval, range 1–30 |
| `CYANREX_AGENT_REQUEST_TIMEOUT_SECS` | `10` | HTTP timeout, range 2–60 |
| `CYANREX_AGENT_ALLOW_INSECURE_HTTP` | `false` | Permit non-loopback HTTP on an explicitly trusted lab network |
| `CYANREX_AGENT_ONCE` | `false` | Complete one successful poll cycle and exit |

Do not configure both token variables. The client disables redirects and environment HTTP proxies
so registration credentials cannot be forwarded to another endpoint accidentally.

## Expected Lifecycle

1. Register using the bootstrap token and receive a per-node HMAC credential.
2. Send signed health/capacity heartbeat.
3. Claim at most the advertised free capacity.
4. Synchronize the job lease to observe cancellation.
5. Execute a built-in probe, or invoke Clang with fixed arguments for an enabled compile check.
6. For compilation, enforce resource and output limits, hash the object, then delete the workspace
   without loading or returning the object.
7. Re-register automatically when the Engine loses in-memory Agent state.
8. Send a best-effort `draining` heartbeat on Ctrl-C.

## Administrator Operations

Open **Settings → Runner Agent Operations** with an administrator account. The panel refreshes every
10 seconds and can also be refreshed manually. It distinguishes a disabled control plane from an
enabled control plane with no registered nodes, then shows:

- online and retained Agent counts, healthy free capacity, isolation, version, capabilities, labels,
  kernel release, and the last heartbeat;
- the 12 most recent remote jobs, including state, target or assigned Agent, owner, creation time,
  and the bounded result message;
- explicit health-probe submission for healthy Agents and cancellation for queued or claimed jobs.

The panel never renders job output or source code. It is backed by administrator-only inventory and
action routes; teachers and students continue to receive only the sanitized compiler backend list in
the editor.

Administrators submit explicit compile-only jobs with `POST /runner/jobs/compile-check` and inspect
them through `GET /runner/agents` or `GET /runner/jobs`. Authenticated editor users see a sanitized
compiler inventory at `GET /ebpf/check/backends`. After explicitly selecting an Agent, the editor
submits `POST /ebpf/check/remote`, polls `GET /ebpf/check/remote?job_id=...`, and cancels stale work
through `POST /ebpf/check/remote/cancel`. Jobs are bound to the current user, and each user may have
at most two non-terminal remote checks.

The editor defaults to local checking and never silently falls back when the selected Agent is
unavailable. `/ebpf/run` remains local, so remote loading is still disabled. Inventory records source
size, not source text. This protocol accepts only literal safe system-header includes; quoted,
macro-generated, parent-relative, `include_next`, `embed`, and include-probing forms are rejected.

## Troubleshooting

- `401`: bootstrap token mismatch, rotated node credential, replayed nonce, or clock skew.
- `404` after Engine restart: normal; the Agent re-registers automatically.
- `503`: Agent control plane is disabled or its bounded registry/queue is full.
- non-loopback HTTP rejected: configure HTTPS, or set the insecure override only on a trusted,
  firewalled lab network.
- repeated signature failures: synchronize clocks before rotating credentials.
- compile jobs remain queued: enable compile checking on an isolated Agent and confirm its
  inventory includes `clang_check`.
- compile configuration rejected: use `container`, `virtual_machine`, or `dedicated_host`, an
  existing absolute Clang path, and a private disposable work directory.
