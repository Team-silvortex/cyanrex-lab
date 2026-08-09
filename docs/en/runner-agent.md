# Runner Agent Guide

The standalone `cyanrex-runner-agent` connects a trusted Linux, WSL2, or container node to the
Engine control plane. Version 0.2.0 executes only built-in `control_probe` jobs. It does not accept
shell commands, scripts, eBPF source, or arbitrary executable payloads, and it needs no root or
Linux capabilities for this mode.

## Engine Preparation

Set a random bootstrap token of 32–512 characters on the Engine and restart it:

```bash
openssl rand -hex 32
# Store the output as CYANREX_RUNNER_AGENT_TOKEN in docker/.env.
./start.sh restart
```

Keep this token off command lines, source control, screenshots, and frontend configuration. The
Agent uses it only to register. Registration returns a per-node credential that stays in process
memory; restarting the Agent registers again and rotates that credential.

Engine and Agent clocks must be synchronized because signed requests expire after 60 seconds by
default. Use TLS for traffic between hosts. Plain HTTP to a non-loopback address is rejected unless
the explicit lab-only override is enabled.

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

Use `shared_kernel`, `container`, `virtual_machine`, or `dedicated_host` only when that value
truthfully describes the node boundary. The label is displayed to administrators; it does not
create isolation.

## Container

The Engine image also contains `/usr/local/bin/cyanrex-runner-agent`. Run the Agent without
`--privileged`, host PID mode, kernel mounts, or added capabilities:

```bash
docker run --rm --name cyanrex-runner-agent \
  --user "$(id -u):$(id -g)" \
  --entrypoint cyanrex-runner-agent \
  --env-file ./runner-agent.env \
  --mount type=bind,src="$PWD/agent-token",dst=/run/secrets/cyanrex-agent-token,ro \
  cyanrex/cyanrex-engine:0.2.0
```

Start from [`docker/runner-agent.env.example`](../../docker/runner-agent.env.example). Rebuild the
Engine image after updating the source. The control-probe Agent remains unprivileged even though the
same image can run the privileged Engine service.

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
5. Read the kernel release from `/proc`, build a bounded JSON probe result, and return it.
6. Re-register automatically when the Engine loses in-memory Agent state.
7. Send a best-effort `draining` heartbeat on Ctrl-C.

Administrators use `GET /runner/agents` and `GET /runner/jobs` to inspect the state. Remote
`/ebpf/run` execution remains disabled.

## Troubleshooting

- `401`: bootstrap token mismatch, rotated node credential, replayed nonce, or clock skew.
- `404` after Engine restart: normal; the Agent re-registers automatically.
- `503`: Agent control plane is disabled or its bounded registry/queue is full.
- non-loopback HTTP rejected: configure HTTPS, or set the insecure override only on a trusted,
  firewalled lab network.
- repeated signature failures: synchronize clocks before rotating credentials.
