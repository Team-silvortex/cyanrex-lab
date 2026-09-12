# Acceptance checklist

Snapshot: **2026-09-09**, working tree on the **0.3.5** version line with Unreleased changes.
These increments are now recorded in **0.3.6**. The measurements, input manifests and binary hashes
remain bound to their pre-bump snapshots; they have not been rewritten as 0.3.6 artifact acceptance.
This is a local integration checkpoint, not an accepted release artifact or a claim that the running
deployment contains this source. See [project status](project-status.md) for the broader test baseline.

The 0.3.7 [test network](testing-network.md) originally found **9 failures** (4 auth, 3 script
persistence, 2 event deletion). These are now fixed with the original assertions passing; see the
separate [fix verification](../../reports/acceptance/2026-09-09-boundary-fixes/result.json).
Historical failed evidence below is retained, not rewritten. No deployment update is implied.
These fixes are recorded in **0.3.8**; the reports retain their pre-bump **0.3.7** inputs and hashes,
not 0.3.8 artifact acceptance claims.

## First round: passed

| Check | Observed result | Boundary |
|---|---|---|
| Real PostgreSQL | 1 test passed on PostgreSQL 16.14 | Service-level SQL calls, not browser/HTTP acceptance |
| Real Runner Agent | 1 test passed using signed loopback HTTP and Clang 18.1.3 | Probe and compile-only jobs; no eBPF load |
| CI wiring | Regression and YAML structure checks passed locally | New PostgreSQL step not yet run in GitHub Actions |

The database test starts with the legacy learning table, migrates feedback fields, races two writes at
the same revision, reloads the winning update, and retains source and automated completion. It also
checks exact historical reads for the owner, excludes foreign/missing records, reloads a new submission,
and rejects reads and feedback writes after the pool closes without creating a local snapshot.
This does not test a network partition, power loss, database capacity, or a full student/teacher session.

The Agent test creates its own HTTP server on `127.0.0.1:0`, uses synthetic bootstrap credentials,
registers the client and completes probe/compile jobs. It checks successful job states and nonempty
object size/hash, then removes its work directory. The test runs as an unprivileged host process;
its declared `Container` capability is metadata, **not proof of container or VM isolation**.

The database used an existing local PostgreSQL 16 image by content ID, a random password, ephemeral
loopback port, UID/GID 999, read-only root, dropped capabilities, `no-new-privileges`, resource limits
and tmpfs data. It had no host data mounts or persistent data volume. Only this newly created container
was removed after the checks. No deployment credentials, existing database, Compose stack, VM, or kernel
attachment were changed. No privileged Engine or offline installation smoke was started.

Machine-readable observations, selected source hashes and test-output excerpts are retained under
`reports/acceptance/2026-09-09-real-integrations/`. The hashes cover selected files, not a full source
manifest; the recorded HEAD alone cannot identify this dirty working tree. These records do not replace
the candidate-bound package checksums and live-kernel evidence required for a release.

## Second round: disposable-VM kernel acceptance passed

After explicit approval, a new QEMU/KVM guest was created from the Ubuntu 24.04 minimal image dated
2026-09-05. Its signed SHA-256 list was verified with the installed Ubuntu cloud-image keyring, then
the image checksum was verified. Guest packages were updated; the tested system was Ubuntu 24.04.5,
kernel `6.8.0-139-generic`, Clang 18.1.3 and bpftool 7.4.0.

- Resources: 2 vCPUs, 3072 MiB guest memory and an 8 GiB sparse disk. The unprivileged QEMU process
  additionally had a 200% CPU quota, 4 GiB memory ceiling, no swap, 64-task ceiling and two-hour lifetime.
  It used seccomp; the guest had no host filesystem shares, device passthrough or Docker socket.
- SSH used a new private key and a host key pinned from the guest serial console. Password authentication
  and agent forwarding were disabled. Only host loopback port 32222 was forwarded to guest SSH.
  Setup temporarily allowed package downloads. Before any eBPF load, the guest was shut down and restarted
  with QEMU `restrict=on` and IPv6 disabled. An owned host-loopback canary was reachable in setup mode
  and blocked in isolated mode while the host could still reach it. This is a focused network check,
  not comprehensive LAN/escape testing.
- The locked, native **dev-profile** Engine and Rust acceptance CLI were built from the working tree;
  SHA-256 matched after transfer. The Engine ran only inside the guest, bound to its own loopback,
  with freshly generated in-memory admin/TOTP secrets and no deployment `.env`, database or frontend.
- Real Aya `sched/sched_switch` attach, a uniquely matched 24-byte ring-buffer event, exact detach
  and an empty attachment inventory passed. Independent bpffs inspection found no pinned files;
  before/after kernel inventories retained the same 12 program IDs and one link ID while the Engine
  was still alive. The native evidence verifier passed both in the guest and on the host.
- The acceptance Engine, canary and VM were stopped; the VM disk passed `qemu-img check`. Private VM
  files remain outside the repository for follow-up; no existing VM or Compose service was restarted.

Evidence and all 145 Engine source-input hashes are in `reports/acceptance/2026-09-09-kernel-vm/`.
The evidence deliberately has `candidate: null`: this is **source-level acceptance on one guest kernel**,
not an optimized release build, an offline package, a tagged candidate or a full teaching/LAN flow.
Its `native-linux` runtime description refers to the guest kernel, not the physical host kernel.
The manually managed VM is not a new selectable Runner mode or per-student VM lifecycle feature.

## Additional 0.3.7 auth boundary probes: not passed

On 2026-09-09, the uncommitted 0.3.7 working tree was tested without changing business logic or the
running deployment. Eight previously uncovered PostgreSQL cases produced **4 passes and 4 failures**
in three runs, including serial and parallel scheduling. The previous 12 authentication regressions
still passed. Five new classroom-browser failure/cancellation cases passed; the combined browser suite
passed all 25 cases using mocked Engine responses, not a live classroom end-to-end connection.

Unresolved findings, highest priority first:

- **P1 — account generation:** an old-credential login paused before session insertion can resume
  after same-name account deletion/recreation and obtain a valid session for that username.
- **P2 — login/deletion race:** deletion without recreation still lets the pending login return 200
  and a cookie, although that cookie is invalid; a foreign-key rejection also disables auth persistence.
- **P2 — suppressed session cascade:** with a synthetic `BEFORE DELETE` trigger returning `NULL`,
  deletion reports success but leaves a valid durable session without its user.
- **P2 — suppressed session insert:** with a synthetic `BEFORE INSERT` trigger returning `NULL`,
  login reports success without a valid new session. Both trigger cases are fault-injection boundaries,
  not evidence that the default schema spontaneously suppresses writes.

Session-step errors, deferred COMMIT errors and cancellation between the deletion steps correctly
rolled back account/session deletion. Cancelled classroom joining did not restore its invitation, but
the already-dispatched account INSERT did commit in these runs: cancellation is not rollback.
Browser network errors, invalid JSON, 503, incomplete enrollment responses and navigation discarded
secrets without automatic retries. This does not establish that an interrupted enrollment needs no
manual account recovery.

Reproductions are in `engine/src/services/auth_service/postgres_boundary_tests.rs`; selected source
hashes and test excerpts are retained in `reports/acceptance/2026-09-09-auth-boundaries/`. Against a
**new disposable database only**, select `services::auth_service::postgres_boundary_tests::` with
`cargo test --manifest-path engine/Cargo.toml --locked --lib` and pass `-- --ignored --nocapture`.
Keep `DATABASE_URL` unset and configure only `CYANREX_TEST_DATABASE_URL` with `CYANREX_DB_FALLBACK=true`.
These new probes are not in the current CI exact-name list and remain opt-in for database isolation;
four are still failing. A green portable/default gate must not be interpreted as passing these probes.
The disposable database was removed and the temporary frontend was stopped; no deployment or
live-kernel work was performed. This checkpoint is not release acceptance.

## Repeat the non-kernel integrations

For PostgreSQL, first provision a **new disposable database** with a UTF-8 locale and a role able to
create/drop schemas. Set `CYANREX_TEST_DATABASE_URL` through your local environment to that database,
never to an existing classroom database. Do not copy a deployment `.env` or put secrets into a report.
The test drops only its generated schema on success; a failed test can leave it behind, so dispose of
the test database after either outcome. CI owns a fresh PostgreSQL service for this purpose.

Run from the repository root with Rust and Linux `/usr/bin/clang` supporting the BPF target:

```bash
set -euo pipefail
test -n "${CYANREX_TEST_DATABASE_URL:-}"
task_acceptance_data=$(mktemp -d /tmp/cyanrex-integration-data.XXXXXX)
task_pg_case=services::learning_store::feedback::tests::postgres_feedback_migrates_legacy_rows_and_serializes_updates
cargo test --manifest-path engine/Cargo.toml --locked --lib "$task_pg_case" \
  -- --ignored --list | grep -Fx "$task_pg_case: test"
env -u DATABASE_URL -u CYANREX_BENCH_DATABASE_URL \
  CYANREX_DB_FALLBACK=true CYANREX_DATA_DIR="$task_acceptance_data" \
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$task_pg_case" \
  -- --ignored --exact --nocapture
env -u DATABASE_URL -u CYANREX_TEST_DATABASE_URL -u CYANREX_BENCH_DATABASE_URL \
  CYANREX_DATA_DIR="$task_acceptance_data" \
  cargo test --manifest-path engine/Cargo.toml --locked --test runner_agent_client_tdd \
  -- --ignored --nocapture
```

Both tests remain ignored in the portable default gate; CI explicitly invokes each one. The PostgreSQL
URL and opt-in flag belong to that step only. The service publishes an ephemeral loopback port, not the
deployment's fixed port. The exact-name/list guard prevents a renamed PostgreSQL test silently selecting
zero tests. Inspect and remove only the recorded temporary data directory after finishing.

## Remaining gates, in order

1. **Disposable Linux target — prepared:** the dedicated, bounded guest above passed source-level
   kernel acceptance and is powered off. Use a fresh guest state for candidate installation; do not
   substitute a production host or an unrelated existing VM. This temporary guest is not a backup.
2. **Candidate installation and kernel evidence:** build a clean versioned candidate, verify its source
   and image/archive checksums, then run the extracted package smoke on that target. The kernel check
   must observe a real Aya attach, a uniquely bound ring-buffer event, exact detach and no residue.
   Installation/live-kernel scripts are privileged and destructive; read their preconditions first.
3. **LAN teaching acceptance:** validate the selected TLS/origin policy and reachability from intended
   clients, login/CSRF/role boundaries, real student submission → teacher feedback → confirmed resume,
   stream reconnect, and restart persistence with synthetic accounts/data. No public ingress is required.
4. **Isolation boundary:** if students must safely execute untrusted programs, implement and verify
   VM lifecycle, persistent owner binding, runtime/event routing, failure recovery and cross-owner denial
   before allowing it. Current local execution shares the host kernel; the compile-only Agent does not
   provide this boundary. See [classroom isolation](classroom-isolation.md).
5. **Release decision:** review all evidence, finish the version/Changelog and clean-source checks, then
   separately authorize commit, push/tag and deployment. Retain known gaps rather than labelling them passed.

The current running frontend was observed to still contain Next.js 15.5.22; the workspace lock contains
15.5.24 and sharp 0.35.4. Nothing was redeployed during this checkpoint. Apply the
[security guide](security.md) rebuild guidance when a deployment change is authorized.
