# Security and Classroom Deployment

## Key Conclusion

Cyanrex Engine is a privileged kernel experimentation environment, not a multi-tenant safe sandbox.

Docker requires access to host/VM eBPF, tracefs, BTF, and bpffs. If a user gains admin access to the Engine,
they can perform privileged kernel observation and loading. Do not expose Engine to untrusted networks.

## Default Protections

- Frontend, Engine, and PostgreSQL are bound to `127.0.0.1` only.
- First startup generates random DB, admin, and TOTP secrets.
- `.env` permissions are `0600` and ignored by git.
- User registration and OTP bootstrap are disabled by default.
- eBPF checks, semantic completion, script list/save/delete, and session-bound operations are available to authenticated users.
- Module browsing is available to admin/teacher roles; module modification and system-level settings remain administrator-only.
- Temporary lockout after repeated login failures.
- Global and per-user Runner capacity, source size, and execution time limits are enforced for eBPF tasks.
- Runner status reports `shared_kernel`; local quotas do not claim tenant isolation.
- DB stores session token hashes, not raw usable tokens.

These measures reduce accidental exposure, but they do not make the privileged Engine a shared safe runtime.

### Runner Agent credential boundary

Remote Agent registration is disabled by default. Enabling it requires a secret
`CYANREX_RUNNER_AGENT_TOKEN` of at least 32 characters. Keep the endpoints on a private management
network or behind TLS and source restrictions; never place the token in frontend code, logs, or a
repository. Rotate it when a node is lost or compromised.

The pre-shared Bearer token is accepted only for registration. Registration returns a distinct
256-bit Agent credential once with `Cache-Control: no-store`; re-registering the ID rotates it.
Heartbeat and job endpoints require HMAC-SHA256 over method, path, Agent ID, Unix timestamp, nonce,
and body hash. The timestamp has a bounded freshness window and each nonce is accepted once.

A holder of the bootstrap token can still re-register and impersonate an Agent ID, so it remains a
high-value secret. Current jobs are bounded `control_probe` messages with per-claim lease tokens,
deadlines, cancellation acknowledgement, and size-limited results. They never contain eBPF source
and `/ebpf/run` remains local. Signed transport proves which registered credential sent a result; it
does not prove that a remote kernel executed honestly. Mutual TLS or node-bound keys and attestation
should be considered before using remote results across a stronger trust boundary.

## Recommended Isolation

### Personal Computer

One instance per student. Windows users should use WSL2, Linux users choose native Linux or Docker. This is preferred.

### Remote Personal VM

One VM per student, accessed via SSH tunnel. A VM can be fully destroyed after class.

### Centralized Server

Do not share one Engine among multiple untrusted students. If resources are limited, isolate with VMs/microVMs,
independent lab nodes, and limit network, CPU, memory, and lifetime per node.

## Prohibited Operations

- Do not set `CYANREX_BIND_ADDRESS` to `0.0.0.0` and expose directly to public internet.
- Do not commit or share `docker/.env`.
- Do not mount Docker socket into Engine.
- Do not disable auth or treat public registration as classroom isolation.
- Do not allow unknown source programs from students.
- Do not run production workloads on the same trust boundary as teaching Engine.
- Do not treat verifier as a full malware sandbox.

## Remote Access

Prefer SSH tunnel:

```bash
ssh -L 3000:127.0.0.1:3000 \
    -L 8080:127.0.0.1:8080 \
    USER@SERVER
```

Long-running services should use TLS reverse proxy, source restrictions, and host firewall, and set
`CYANREX_SECURE_COOKIES=true`. Enable Secure Cookie only when browser access is HTTPS.

### Optional persistence warning tuning

The following variables are optional and only affect diagnostics tuning for high-throughput event classes:

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default: `true`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default: `80`)
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default: `40`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default: `10000`)

## Pre/Post-Class Checklist

Before class:

- Update dependencies and run `npm audit`, `cargo audit`.
- Verify random secrets and loopback binding.
- Snapshot or backup lab VMs.
- Run through all labs in dry-run mode.
- Ensure host is not running sensitive production workloads.

After class:

- Detach all eBPF attachments.
- Stop the lab stack.
- Clean temporary accounts/credentials.
- Check for stale files under bpffs.
- Prefer destroying and recreating VMs in shared environments instead of reusing unknown states.

## Dependency Audit Policy

- We run `cargo audit` for Rust backend dependencies and track accepted exceptions in
  `scripts/security-audit-exceptions.json`.
- Current accepted exception:
  - `RUSTSEC-2023-0071` (`rsa`) — transitive via `sqlx-mysql` in `Cargo.lock`.
    The backend does not currently use RSA operations directly, and the advisory currently has no patched release.
- Each accepted advisory has a review deadline and must be re-evaluated before it expires.

## Security Incident Response

If you suspect credential leakage or unknown programs:

1. Immediately disconnect lab node network.
2. Stop Engine.
3. Do not keep using the same password after only an in-app change.
4. Export required logs and attachment details.
5. Destroy single-use VM and rebuild.
6. Rotate DB, admin, TOTP, and SSH credentials.
7. Check host workloads for secondary impact.
