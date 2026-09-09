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

Select **Review attempts** beside a student to inspect up to 20 recent submissions. Each record
shows the backend stage, run and attachment result, automated feedback, and the exact submitted
source. Use this evidence to distinguish a compile failure from a verifier/attach failure and to
discuss the student's reasoning. The review API is restricted to teacher and administrator roles;
students can only read their own attempt history.

The compact summary keeps the student roster near the top. On narrow screens, each roster row becomes
a labelled card, keeping **Review attempts** reachable without sideways scrolling. Selecting a student
focuses and scrolls to the review heading when its request completes. Use **Back to students** to return
to the roster; changing viewport size does not discard an open feedback draft.

Write and save feedback below a submission. The student can read it under **Learn → My lab history**.
Students can then choose **Continue from this attempt**, review the original source/current feedback,
and explicitly replace their editor draft. Loading alone does not execute a program or change acceptance;
a later manual run creates a new attempt and retains the original submission and its comment.
Each attempt keeps one current comment with the last teacher/admin's identity and modification time;
this is not a grade and never changes automated completion. Comments are plain text, trimmed before
validation, and limited to 1–2000 Unicode characters. If another teacher edits the same comment, the
page keeps your draft and rejects the stale save. Select **Load latest feedback**, compare the latest
comment with your draft, and explicitly save again. Failed saves also keep the draft. Role and CSRF
origin checks apply to the write API.

Local history uses shared snapshots and bounded recent selection, but still rewrites the full JSON on
save. A started local save continues after a timeout/disconnection, so reload the current feedback
before retrying. Keep backups and do not treat it as an append-only durable log; see
[Learning Record Storage](learning-storage.md) for costs, limits and reproducible local measurements.

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

Destructive controls now show their targets and impact before execution. Header deletion affects this
Engine's downloaded catalog and can disrupt other users' compilation; deleting selected headers requires
`DELETE`. Batch header actions stop on the first failed response and report how many items completed;
there is no rollback of earlier items. Check/refresh the catalog before retrying. Module Start/Stop,
Runner job cancellation and event/compiler settings also require confirmation. Read-only commands,
catalog refresh and health probes do not add a confirmation. Account password/OTP and Engine role/CSRF
checks remain authoritative; a confirmation is only a protection against accidental clicks.

Click “Detach All” in eBPF page and verify attached list is empty, then stop:

```bash
./start.sh stop
```

To clear course data, remove Docker volumes after shutdown. Removing volumes permanently deletes accounts,
scripts, and events; ensure you no longer need this data before doing so.
