# LearningStore streaming commits and cancellation — 2026-09-08

## Outcome

Local commits now stream the unchanged pretty JSON array through a 64 KiB buffer on a blocking worker.
The worker explicitly flushes and closes the temporary file, renames it, then publishes the shared
snapshot. The complete encoded-file allocation is removed; the complete file is still rewritten.

The worker owns the store's writer lock even after its requesting task is cancelled. This fixes a
reproduced gap where the file had been renamed but memory was still old, allowing a later writer to
overwrite the saved update. Cancellation before worker handoff does not start a write; cancellation
after handoff means the save can complete without a response. Reload before retrying. There is no new
append idempotency, fsync, crash recovery or cross-process transaction.

The new disk matrix found 50,000-row / 1,112-byte-source write mean 144.638 → 111.273 ms (-23.1%) and
p95 152.506 → 122.124 ms (-19.9%). At 10,000 rows, whole-process write peak RSS was 63.2 → 39.2 MiB
(-38.0%). Small-write mean/p50 and some unchanged read controls regressed. Large-source disk tails
remain variable: these results do not establish a uniform speedup or a production latency target.

## Baseline and method

```sh
node scripts/bench-learning-store.mjs <new-output-directory> <frozen-baseline-mainline-binary>
```

The baseline is the preceding shared-snapshot implementation, including the earlier Unreleased
owner-fanout work, based on `5fa774110a50e8835422402014182827c21d3044` (`0.3.4`). It is **not** a clean
release binary. It was built and copied before this increment's edits. The public mainline example,
paired runner, ordering helper, query algorithms, dependency lockfile, SQL and wire schemas are unchanged.
The [previous snapshot report](../2026-09-08-learning-snapshots-final/REPORT.md) retains its own baseline,
data and conclusions unchanged; do not substitute its older measurements into this comparison.

Each filesystem matrix has 12 cases × 2 binaries × 3 rounds = 72 measured process invocations, plus
two small uncounted warmups. Cases rotate, and each case swaps its before/after order each round.
Thirty synthetic students span 1,000 / 10,000 / 50,000 rows with 1,112-byte source, plus 50,000 rows
with 4,184-byte source (the common example source plus 1,024 / 4,096 padding bytes). Each process uses
a newly created, automatically removed fixture. Live timestamps and generated appended UUIDs mean
fixtures are structurally matched, not byte-identical across processes; same-input encoding has an
exact-byte regression. No live classroom files were read or changed.

Measured latency starts after loading; each invocation contains 100 recent queries, 30 teacher
overviews or 10 successful failed-compile-attempt writes (no actual compiler/kernel execution).
Write p95 is nearest-rank and therefore the maximum of ten samples. Tables show medians of three
run statistics, not pooled request percentiles. All runs and min/max remain in `runs.jsonl` and
`summary.json`. GNU time CPU/RSS covers fixture construction, loading, measurement and cleanup;
it is not steady-state retained storage or per-request peak memory.

Environment: shared Ryzen 7 7735H desktop, Linux 7.0.0-30-generic, rustc 1.95.0, 16 Tokio workers.
`/tmp` was on ext4 (`/dev/nvme0n1p2`, about 93% used at inspection). The current binary is built with
locked offline dependencies before measurement. Heavy builds/tests and the two matrices never overlap.
There was no CPU pinning, page-cache eviction, I/O tracing or fsync. The disk matrix ran
11:55:37–11:57:03 UTC; its adjacent metadata binds 140 measured source files and both executable hashes.

## Disk writes

Values are before → after. Source bytes are actual bytes per record, not just the padding parameter.

| Rows / source bytes | Mean ms | p50 ms | p95 ms | Peak process RSS MiB |
|---|---:|---:|---:|---:|
| 1,000 / 1,112 | 1.920 → 2.001 | 1.701 → 1.910 | 3.444 → 2.427 | 12.50 → 10.00 |
| 10,000 / 1,112 | 19.762 → 20.541 | 17.959 → 20.103 | 31.078 → 21.850 | 63.18 → 39.17 |
| 50,000 / 1,112 | 144.638 → 111.273 | 143.205 → 110.796 | 152.506 → 122.124 | 180.77 → 165.76 |
| 50,000 / 4,184 | 815.535 → 647.936 | 713.524 → 544.878 | 1,273.480 → 1,129.551 | 476.41 → 462.85 |

Small-write means increased 4.2% and 3.9%, and p50 also rose; lower run p95 does not erase those
regressions. No extra worker dispatch existed in the old encoding path, but no function-level trace
was taken, so scheduling/buffering overhead is a hypothesis, not a demonstrated attribution.

At 50,000 / 1,112, run p95 ranges were 149.471–160.476 ms before and 119.409–122.549 ms after.
For 50,000 / 4,184, they were 422.939–1,711.362 and 377.622–1,172.865 ms. Every large-source pair's
after mean was lower (rounds 1–3: 815.535 → 647.936, 992.276 → 835.386, 401.806 → 305.292 ms), but
the run-to-run disk variation is large. This does not establish a stable disk tail.

The large-source process CPU medians were 1.72 user + 2.52 system seconds before, versus 1.53 + 1.77
after; they include setup/cleanup, not just commits. At the largest sizes, process peak RSS remains
dominated by loading/retained source and allocation reuse, so removal of a full encoded-file buffer is
not a same-sized reduction in this whole-process peak. Files still become approximately 74 / 220 MiB
of pretty JSON and are rewritten on every measured write. The fixed encode buffer does not bound total
memory, individual source size, record count, pointer-index copies or the lifetime of reader snapshots.

## Unchanged read controls

Before → after median run p95, milliseconds. No query algorithm was changed in this increment.

| Rows / source bytes | Recent 20 | Teacher overview |
|---|---:|---:|
| 1,000 / 1,112 | 0.00731 → 0.00690 | 0.0797 → 0.0813 |
| 10,000 / 1,112 | 0.0462 → 0.0651 | 0.404 → 0.389 |
| 50,000 / 1,112 | 0.193 → 0.199 | 4.161 → 4.580 |
| 50,000 / 4,184 | 0.413 → 0.358 | 4.634 → 4.173 |

The 50,000-row ordinary-source overview is about 10% slower in this disk group; a 10,000-row recent
control also regressed. Other controls improved, and the separate tmpfs overview results change
direction. This shared-host test does not attribute those effects to a specific function or establish
read performance neutrality. No new read-speed claim is made.

## Independent tmpfs control

The [second 72-run matrix](../2026-09-08-learning-streaming-writes-tmpfs/REPORT.md) used the same runner
and exact binaries under the existing `/dev/shm` tmpfs, with `TMPDIR` scoped to a new owned directory.
No mount, kernel setting or production data path changed; all scratch data and the empty parent were
removed. It ran separately at 11:58:57–11:59:46 UTC.

| Rows / source bytes | Mean ms | p95 ms |
|---|---:|---:|
| 1,000 / 1,112 | 2.278 → 2.005 | 3.807 → 2.495 |
| 10,000 / 1,112 | 21.055 → 19.484 | 31.185 → 20.761 |
| 50,000 / 1,112 | 133.322 → 91.607 | 138.723 → 93.401 |
| 50,000 / 4,184 | 354.390 → 231.370 | 361.326 → 240.870 |

All three large-source after-run means were below their paired baselines. The large-source median mean
fell 34.7%, but tmpfs is not persistent storage and its pages are not fully represented in process RSS.
This control is not a recommendation to store classroom history in RAM, a disk-durability result, or a
reason to discard unfavorable/extremely variable disk measurements.

## Correctness and verification

- Two tests first reproduced the old cancellation bug by holding a reader while observing successful
  file replacement, then cancelling append/feedback before publication. Both failed because the writer
  lock was released too early. They now confirm matching disk/memory and preservation of stale-revision
  rejection and subsequent saved records.
- Nine new Rust tests cover the above plus cancellation before admission, cancellation while queued
  behind a single occupied blocking thread, detached-worker failure/retry, emission before all rows are
  serialized, identical Unicode/escaped JSON, short/interrupted writes, partial writes, final flush and
  serialization errors. Existing legacy-file, rename-failure, sharing and concurrent-update tests pass.
- The five cancellation tests passed 20 repeated runs (100 test executions); the focused LearningStore
  suite passed 22 tests, with its external PostgreSQL test ignored as usual.
- Full `./scripts/quality-gate.sh --security --no-npm-install` passed: 205 Rust tests, 5 normally ignored,
  45 common-tooling tests, 29 frontend regressions, frontend production build/type checks with 17 static
  routes, SDK 11 runtime + 3 package checks plus types, OpenAPI/SDK compatibility, Runner/distribution
  tooling, format, version/course sync and file lengths. Cargo and production npm audits found no
  vulnerabilities. No dependencies, version, commit, tag or push changed.
- No privileged Engine, real PostgreSQL, LAN/kernel acceptance, real-browser smoke, power-loss testing
  or feedback-write performance benchmark was run in this increment.

The implementation uses [Tokio blocking task semantics](https://docs.rs/tokio/1.53.1/tokio/task/fn.spawn_blocking.html)
and an explicit [buffer flush](https://doc.rust-lang.org/std/io/struct.BufWriter.html). Request cancellation
does not interrupt an admitted commit, but panic, runtime/process teardown and power loss are outside
this guarantee. Stuck I/O can retain the store's lock/thread and delay shutdown. There is no new deadline,
global admission limit, temporary-file recovery or cross-process lock. See
[Learning Record Storage](../../../docs/en/learning-storage.md) for operational behavior and retry limits.

## Fingerprints

- Base commit: `5fa774110a50e8835422402014182827c21d3044` plus prior Unreleased work.
- Baseline LearningStore: `c7a477f0534b41496d293bc1a5a0ed8365b07f0dc39ac8684360976229dedfa8`
- Baseline feedback: `8aa6752214851143508f9f9a6002d90a7b3cb216338901a5f4c9216eb5f78dcc`
- Baseline binary: `23604a1b10f5ffbdcf3997b134311c7b72ce7985e32d785ce5a45b682bf66185`
- Current binary: `e4e84ece0eb3b364792eea04190e00b5a88762fd661aefca9025961d0cd67ce2`
- Unchanged mainline harness: `5c7d2028737123d6f2aafc1a951e1c42d66e82c3a015ae74901ba738e6a63111`
- Current LearningStore: `2ffe24c59b51680c46531040d0e6e6500350ffa89731690d65ccb0a01198fadd`
- Persistence worker/encoder: `1523fe7c4bea17aaaafe2f7b50334f2ae846373a47db191af81a3de6545847ab`
- Current feedback: `9879921260ecd6fa773d134f40da28bb730162a3802e244b8304f2683c9296a2`

All measured source hashes are retained in `metadata.json`. These are uncommitted development
measurements, not release-artifact acceptance or a claim of a published `0.3.4` artifact.
