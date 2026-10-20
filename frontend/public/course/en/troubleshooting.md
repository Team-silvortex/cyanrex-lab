# Troubleshooting Guide

Source **0.6.3** includes explicit loopback/TLS hosting, limited Session issuance and private
Task create/list/reopen/manifest replacement. Browser/SDK saving, new-byte publication, LAN access
and migration remain absent; earlier implementation-stage labels retain their original scope.

First identify which path you are using: the existing teaching runtime, the local task draft editor,
or an explicitly initialized collaboration store. They have different persistence and authorization
contracts; see the [platform map](platform-network.md). An absent future feature is not a deployment fault.

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

## Task drafts and collaboration preparation

| Symptom | Meaning and safe next step |
|---|---|
| Task draft disappeared after navigation/restart | Drafts are page memory, not server records. Explicitly download important work before leaving; no autosave/recovery is implemented |
| Rust/Python/C++ editing has no type errors or project navigation | These local profiles are highlighting/snippet-only. Do not diagnose this through clang or install a server expecting automatic LSP hookup |
| Download succeeded but no server Task exists | Download is a local export; public Task saving is not connected |
| Preparatory command reports stale revision | Preserve edited content and reread the current authorized record; do not overwrite with a guessed revision |
| Task storage reports unsupported schema | Schema 2 requires a fresh dedicated installation. Preserve old data; do not change metadata, drop tables or let a live writer share the namespace |
| Replacement fails after Artifact publication | The published immutable version may remain. Do not delete files, adopt content or automatically replay the command |
| Teacher cannot read another member's general Task | Private preparation APIs derive ownership from the Session; classroom teaching authority is not a cross-user resource grant |

The [editor guide](editor.md), [replacement contract](session-task-revisions.md) and
[provisioning guide](collaboration-provisioning.md) describe the supported operations. These failures
are not instructions to run migrations or fault-injection tests on a deployment.

## Services and Logs

```bash
./start.sh status
./start.sh diagnose
./start.sh logs engine
./start.sh logs frontend
docker compose -f docker/docker-compose.yml ps
```

`./start.sh logs` follows all services; add a service name to select one. Ctrl-C closes the log
viewer, not the stack. Status and log viewing no longer hold the startup lock, so an open log stream
does not block stopping the same instance. Starts and stops remain serialized; a concurrent status
read is an observation, not proof that a transition has finished.

`--debug` prints a safe notice instead of tracing shell commands and credential assignments. Do not
use `bash -x` to collect shareable startup logs. Tool and service output can still contain sensitive
information; review it before sharing.

All source-launcher actions select instance, ports and bind address in this order: explicit CLI
option, nonempty inherited environment variable, `docker/.env`, then the built-in default. Stop,
status, logs and both diagnostic entries use the same selection rules as start. Repeat any CLI or
environment overrides when operating that instance; the launcher does not remember the last target.
The project is always `cyanrex-<instance-id>`; `CYANREX_COMPOSE_PROJECT` does not override it. Instance IDs must be
1–63 lowercase letters, digits, underscores or hyphens; invalid IDs fail instead of being silently
rewritten. For example, use `./start.sh diagnose --instance-id room-a --engine-port 18080`.

These five nonsecret selectors accept standalone literal assignments with optional `export`, outer
single/double quotes, whitespace, inline comments and CRLF; the last assignment wins. Dynamic
expressions and multiline values are unsupported. Read-only commands do not source `.env` or create
credentials. Start still evaluates its other settings as a trusted shell file, against the selected
instance, and refuses a later target change. A default volume expression follows that instance;
an explicit custom PostgreSQL volume remains your responsibility to keep separate between instances.
Compose receives the absolute `docker/.env` path explicitly, not a file selected by the caller’s
working directory or `COMPOSE_ENV_FILES`. It must support `--env-file` and the checked-in Compose
template; obsolete legacy versions are not retried with implicit configuration. Keep `.env` compatible
with Compose dotenv syntax; startup evaluation is not a promise to support arbitrary shell programs.

Direct `./scripts/debug-system.sh` diagnostics tolerate missing optional configuration keys and
continue after a failed Compose/HTTP check. Its one loopback health request ignores curl user/proxy
settings, has a 3-second connect and 10-second total
request limit, and skips invalid ports. Other tools are not covered by that HTTP deadline.

Health check:

```bash
curl -q --noproxy '*' -fsS --connect-timeout 3 --max-time 10 http://127.0.0.1:8080/health
```

For an installed offline package, run `./cyanrex-release package verify-controls .` from its package
directory before comparing controls with a trusted matching-version copy. This checks metadata and
three deployment controls without reading `.env` or image contents, running Docker/SSH or changing
services. It is not full-package, configuration, health or signature verification. Use
`./deploy.sh status` and `./deploy.sh logs engine` there; source-checkout `start.sh` paths do not apply
to a package. The package's `README-DEPLOY.md` includes the routine operations sequence.

The browser Helper requires an explicit fresh check. Starting again clears previous results; failed,
incomplete or timed-out requests do not retain a successful readiness display. Reloading the page
does not restore an earlier report. Check again after changing the instance or session; displayed
results are snapshots, not a deployment or ongoing service-health guarantee.
The 10-second browser deadline does not guarantee that Engine checks or subprocesses have stopped.

Docker published ports default to loopback; native/WSL startup currently sets `ENGINE_HOST=0.0.0.0`.
Inspect actual listeners and restrict LAN ingress. On remote hosts, prefer the reviewed SSH tunnel
instead of opening an unprotected `SERVER:3000`; see [security](security.md).

## Login and TOTP

Check:

- use current `docker/.env`, not old screenshot or docs sample
- phone time and server time are in sync
- no more than 5 consecutive failed logins; failure lockout lasts 5 minutes
- `CYANREX_ROTATE_ADMIN_CREDENTIALS` is back to `false`

Do not share `.env` in chat, issues, or class screensharing.

## clang Live Checking

If status stays `unavailable`:

1. Confirm the current authenticated session; local checks do not require teacher/admin authority.
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
