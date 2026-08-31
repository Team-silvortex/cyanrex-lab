# Troubleshooting Guide

## First Determine Which Stage Failed

| Symptom | Priority Check |
|---|---|
| Page not opening | Frontend container, port, SSH tunnel |
| Login failed | `docker/.env`, system clock, TOTP |
| clang unavailable | Engine health, account permission, network |
| compile failed | C syntax, include headers, types, macros |
| load failed | verifier, BTF, permissions, kernel capability |
| attach failed | hook name, bpftool version, tracefs |
| run success but no events | trigger condition, sampling, filters, read path |
| dirty detach | attachment list, pin path, Engine logs |

## Services and Logs

```bash
./start.sh status
./start.sh diagnose
./start.sh logs engine
./start.sh logs frontend
docker compose -f docker/docker-compose.yml ps
```

Health check:

```bash
curl http://127.0.0.1:8080/health
```

By default, services bind only loopback. On remote hosts, do not directly open `SERVER:3000`;
use SSH tunnel.

## Login and TOTP

Check:

- use current `docker/.env`, not old screenshot or docs sample
- phone time and server time are in sync
- no more than 5 consecutive failed logins; failure lockout lasts 5 minutes
- `CYANREX_ROTATE_ADMIN_CREDENTIALS` is back to `false`

Do not share `.env` in chat, issues, or class screensharing.

## clang Live Checking

If status stays `unavailable`:

1. Confirm admin account login.
2. Check Engine health.
3. Confirm source size not over 256 KiB.
4. Confirm Engine logs show clang available.
5. Wait until other compile tasks finish. Engine allows at most two concurrent tasks by default.

Semantic completion also calls backend clang. If backend is briefly unavailable, local snippets may still help.

## BTF and vmlinux.h

If `kernel_btf` / `btf_dump` in helper fails:

```bash
ls -l /sys/kernel/btf/vmlinux
bpftool btf dump file /sys/kernel/btf/vmlinux format c >/dev/null
```

Docker mode requires BTF exposure from host/VM kernel to Engine. Without BTF, CO-RE examples depending on `vmlinux.h`
cannot run, but simple UAPI-only programs may still work.

## bpffs and Permissions

```bash
mount | grep /sys/fs/bpf
ls -ld /sys/fs/bpf
ulimit -l
```

Do not `chmod 777 /sys/fs/bpf` to bypass errors. Fix mount, startup, and container capabilities instead.

## Auto-attach Unavailable

Old bpftool versions may not support `autoattach`. Cyanrex will attempt manual tracepoint attach.
If program type needs explicit interface/cgroup/target, system may load but not choose target correctly.
Provide target explicitly according to lab steps instead of relying on auto selection.

## Program Runs but No Events

Check in order:

1. Attachment list contains the program.
2. Actions trigger the expected hook.
3. Runtime duration has not ended.
4. Sample rate not too low.
5. Event filter in Events page is correct.
6. Ring buffer structure matches user reader expectations.
7. No continuous ring buffer reserve failures (full buffer).

## Final Cleanup

Run “Detach All” first in UI, then if Engine crashed, restart Engine and inspect attachment/bpffs state.
Do not recursively remove `/sys/fs/bpf`; other software may have programs/maps there.
