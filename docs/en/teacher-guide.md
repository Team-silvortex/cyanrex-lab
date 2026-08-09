# Teacher Quick Start

## 1. Course Positioning

Use Cyanrex for 4–8 hour beginner eBPF classes. It is suitable for teaching:

- eBPF program lifecycle
- hook, helper, Map, Ring Buffer
- clang compile errors and verifier behavior
- minimum privilege and kernel observability boundaries

It is not a production-grade multi-tenant sandbox. Do not let untrusted students share one privileged Engine.

## 2. Recommended Topology

The safest approach is one instance per student:

```text
Student Browser -> Local Cyanrex Frontend -> Local/WSL/Docker Engine -> Personal Linux kernel
```

If you must use a centralized server, prepare one VM per student instead of only multiple Cyanrex users.
Engine containers require privileged kernel access; application account isolation cannot replace VM isolation.

## 3. Pre-class Preparation

On each lab host:

```bash
./start.sh start --mode auto --rebuild
./start.sh status
```

For SSH access to a remote host, establish local tunnel:

```bash
ssh -L 3000:127.0.0.1:3000 \
    -L 8080:127.0.0.1:8080 \
    USER@SERVER
```

Open `http://localhost:3000`. Credentials are stored in `docker/.env`.
Import `CYANREX_ADMIN_TOTP_SECRET` into teacher authenticator as Base32 secret; never share or screen-share it.

## 4. Environment Acceptance

In Environment Helper, verify:

- Backend shows expected profile: `docker`, `wsl2`, or `native-linux`.
- `clang`, `bpftool`, `kernel_btf`, `btf_dump` are healthy.
- `/sys/fs/bpf` is mounted.
- `memlock` is sufficient.
- Overall state is Ready.

Some old bpftool versions do not support `autoattach`; Cyanrex will fall back to manual tracepoint attach. This is non-blocking.

### Optional persistence warning tuning

If a lesson produces high event throughput and the event persistence queue grows quickly, you can tune these variables in `docker/.env`:

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default `true`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default `80`)
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default `40`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default `10000`)

## 5. Suggested Lesson Plan

| Session | Topic | Lab |
|---|---|---|
| 1 | eBPF architecture and security model | Lab 1 |
| 2 | Tracepoint and event observation | Lab 2 |
| 3 | Map and state | Lab 3 |
| 4 | Ring Buffer and userspace consumption | Lab 4 |
| 5 | Verifier reasoning | Lab 5 |

For each lab, use cycle: predict → run → explain → modify. Ask students to predict first, then run.

## 6. Checkpoints

Open **Classroom** as a teacher or administrator to review active students, attempt counts, and the
five lab states. A student attempt is recorded only when the eBPF editor was opened with a lab
context and `/ebpf/run` reached the Engine. Completion therefore cannot be set by a browser-only
checkbox.

Automated checks require the expected template, a successful `run` stage, structured source
evidence, and attachment verification where the lab requires it. Source evidence uses C tokens:
comments, string literals, helper-name substrings, and preprocessor definitions do not count.
Equivalent null guards such as `if (!counter)` and `if (counter == NULL)` are accepted. These
checks do not grade explanations. Treat `completed` as "runtime acceptance passed", then review
the reasoning questions below.

Do not only grade `success` state. Ask students to explain:

1. Which hook is attached.
2. The context type in that hook.
3. Which data goes into Map and which goes through Ring Buffer.
4. How the verifier proves memory safety.
5. How they verify program is fully detached.

## 7. Post-class Cleanup

Click “Detach All” in eBPF page and verify attached list is empty, then stop:

```bash
./start.sh stop
```

To clear course data, remove Docker volumes after shutdown. Removing volumes permanently deletes accounts,
scripts, and events; ensure you no longer need this data before doing so.
