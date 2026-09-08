# LearningStore shared snapshots and borrowed queries — 2026-09-08

## Outcome

Local LearningStore queries now take shared immutable snapshots instead of copying every record.
Recent review retains bounded references before cloning the response, while student progress and
teacher overview aggregate in one pass without cloning submitted source. Appends copy the pointer
index and new record; feedback copies the index and only the changed record. Old readers remain
unchanged. Local JSON, full-history responses, SQL queries, authorization and revision checks stay
compatible. Local writes still replace the complete file after encoding the full pretty JSON buffer.

For 50,000 records with 1,112-byte source, corrected-order disk measurements found recent-20 p95
1.744 → 0.220 ms and overview p95 41.907 → 4.446 ms. Write-process peak RSS fell 296.3 → 180.1 MiB.
These are local warm-service measurements, not HTTP or classroom capacity figures.

**Disk writes are not uniformly faster.** In the larger-source case, mean write latency increased
696.5 → 891.4 ms despite lower CPU/RSS and a lower median run p95. A separate tmpfs control improved
both mean and p95, supporting a filesystem/wait-cost explanation but not proving it. No general disk
write-latency improvement or durability guarantee is claimed.

## Method and provenance

```sh
node scripts/bench-learning-store.mjs <new-output-directory> <frozen-baseline-mainline-binary>
```

The public `engine/examples/mainline_bench.rs` is unchanged on both binaries. The baseline was copied
before LearningStore edits, after the previous owner-fanout increment; its LearningStore sources still
matched commit `5fa774110a50e8835422402014182827c21d3044`. It is not described as a clean release binary.
The current build uses locked offline dependencies. Runs never touched live classroom data.

There are 12 configurations × 2 binaries × 3 rounds = 72 measured invocations, plus two small uncounted
warmups. Thirty synthetic students span 1,000 / 10,000 / 50,000 initial records; source sizes are 1,112
bytes and a 50,000-record 4,184-byte control (fixed example source plus 1,024 / 4,096 bytes of padding).
Recent review takes one student's newest 20; overview includes all 30 students; record adds a failed
compile attempt without running code. Each invocation uses a fresh, automatically cleaned temporary file.

Latency starts after loading the fixture. Each invocation has 100 recent samples, 30 overview samples
or 10 writes. Write nearest-rank p95 is thus the maximum of ten samples. Tables take the median of three
run statistics, not pooled request percentiles. Min/max and every run remain in the adjacent JSON.
GNU time measures the whole process, including creation, parsing, first-load warmup and cleanup;
its RSS is not steady-state retained data, and short-run CPU counters have coarse resolution.

Configurations rotate, and each configuration swaps its before/after order between successive rounds.
Two common-tool regressions verify pair membership and independent alternation. The
[first-pass ordering defect](../2026-09-08-learning-snapshots/REPORT.md) and all 72 earlier measurements
are retained, not silently corrected or merged into this dataset.

Environment: Ryzen 7 7735H shared desktop, Linux 7.0.0-30-generic, rustc 1.95.0, 16 Tokio workers.
`/tmp` was on ext4 (`/dev/nvme0n1p2`, about 92% used at inspection). No heavy builds/tests overlapped the
measurements. There was no CPU pinning, page-cache eviction, per-I/O trace or fsync. The experiment
excludes PostgreSQL, HTTP, browser rendering, Clang, Runner Agents, eBPF and crash/power-loss behavior.

## Corrected-order ext4 results

All values below are before → after, median run p95 in milliseconds.

| Initial rows / source bytes | Recent 20 | Teacher overview | Record write |
|---|---:|---:|---:|
| 1,000 / 1,112 | 0.0133 → 0.0094 | 0.553 → 0.083 | 4.901 → 4.588 |
| 10,000 / 1,112 | 0.150 → 0.044 | 8.012 → 0.484 | 44.098 → 31.838 |
| 50,000 / 1,112 | 1.744 → 0.220 | 41.907 → 4.446 | 216.882 → 194.568 |
| 50,000 / 4,184 | 2.755 → 0.309 | 53.731 → 4.784 | 1,787.317 → 1,237.929 |

The 50,000-row 1,112-byte warm read reductions are 87.4% for recent-20 and 89.4% for overview. The
1,000-row recent absolute difference is only a few microseconds; this shared-host run cannot establish
fine-grained production thresholds. Full-history export, feedback-write and progress latency were not
separately benchmarked; their behavior has regression coverage, not a new quantitative performance claim.

| Write configuration | Mean latency ms | Peak process RSS MiB |
|---|---:|---:|
| 50,000 / 1,112 | 193.7 → 156.2 | 296.3 → 180.1 (-39.2%) |
| 50,000 / 4,184 | 696.5 → 891.4 (+28.0%) | 730.7 → 476.0 (-34.9%) |

The larger-source median run p50 also rose 469.1 → 928.6 ms; a favorable p95 alone would hide this.
Its process CPU median dropped approximately 4.94 → 4.23 seconds, but wall time remained dominated by
variable non-CPU time. For the 1,112-byte 50,000-row case, run p95 ranged 204.9–584.0 ms before and
153.3–602.9 ms after. There is no stable disk-tail claim. Whole-file writeback/scheduling is a plausible
contributor, not a measured function-level attribution.

Memory also has a tradeoff: recent-only 50,000-row/1,112-byte process peaks rose 151.9 → 165.4 MiB
(+8.8%) with per-record shared-pointer bookkeeping and load/setup allocations. This is not a universal
memory reduction. Writes still serialize about 74 MiB / 220 MiB of pretty JSON per operation in the
two largest configurations; they do not become incremental.

## Independent tmpfs control

A [second 72-run matrix](../2026-09-08-learning-snapshots-tmpfs/REPORT.md) used the same runner and
binary fingerprints with a new empty directory under the existing `/dev/shm` tmpfs, set only through
that process's `TMPDIR`. No mount, kernel setting or production path was changed; scratch data was removed.

| tmpfs write configuration | Mean ms, before → after | p95 ms, before → after |
|---|---:|---:|
| 50,000 / 1,112 | 175.454 → 136.561 | 191.358 → 140.284 |
| 50,000 / 4,184 | 422.038 → 366.575 | 493.040 → 374.619 |

All three large-source tmpfs after-run means were below their before partners, unlike ext4. This
separates an in-memory-filesystem control from disk behavior; it is not a suggestion to keep classroom
history in tmpfs, a power-loss test, or a replacement for the unfavorable disk observations.

## Correctness and quality

- Allocation-sensitive tests first failed against the old deep-copy implementation. Shared collection
  and untouched-source addresses now remain stable across reads/appends/feedback; old snapshots remain
  unchanged. This is a private cost regression, not a public pointer-stability contract.
- Twelve new Rust regressions cover bounded borrowed selection, timestamp ties/arbitrary order, one-pass
  aggregation differential to the prior algorithm, unknown labs, owner isolation, fresh progress,
  independent response payloads, legacy/plain pretty JSON, write/rename failures and retry, concurrent
  appends/feedback, and preservation of revision conflicts and successful data after reload.
- Two new JavaScript regressions prevent the pair-ordering flaw from returning. No production behavior
  was changed to satisfy an initially over-broad test looking for the phrase `source ` in feedback.
- Full `./scripts/quality-gate.sh --security --no-npm-install` passed: 196 Rust tests (5 normally ignored),
  formatting, 45 common tooling tests, version/course/OpenAPI/SDK compatibility, Runner/distribution tools,
  frontend build with 17 static routes and type checking, 29 frontend tests, and SDK runtime/type/package
  checks (11 runtime + 3 package tests). Cargo and production npm audits reported no vulnerabilities.
  No dependency, wire schema, version, commit, tag or push was changed. PostgreSQL's external test and
  real browser/LAN/kernel acceptance were not rerun in this increment.

## Limits and follow-up

The local collection still grows without a new retention cap. Reads scan it; recent selection uses
`O(k)` temporary storage, not a user index. Writers still copy an `O(n)` pointer index, serialize a full
buffer, and rewrite the file. Readers may retain older snapshots. Disk rename and memory publication
are not a cancellation-safe atomic transaction; there is no new fsync, recovery log or cross-process lock.
PostgreSQL still fetches its existing fields, including source. See
[Learning Record Storage](../../../docs/en/learning-storage.md) for compatibility and operational limits.

The next storage stage should address whole-file encoding/writeback, recovery and cancellation as a
separate design. Do not infer disk throughput, durable audit logging or tenant isolation from faster
warm queries or a tmpfs control.

## Fingerprints

- Base commit: `5fa774110a50e8835422402014182827c21d3044` (`0.3.4`) plus prior Unreleased owner-fanout work.
- Baseline LearningStore: `7ca94212ffe5902bcc58f753359e4faf071b8c998b850a65b97985e60c52aa1c`
- Baseline feedback: `e5690a74a6b16bf2b1d8fb1e44b4d66a35c655116681c7686caf5c7e9d07ddb8`
- Baseline binary: `0a95e997567120ae89af89fca232b198171fb53b016fedaa08e398096e286bd1`
- Current binary: `23604a1b10f5ffbdcf3997b134311c7b72ce7985e32d785ce5a45b682bf66185`
- Unchanged public harness: `5c7d2028737123d6f2aafc1a951e1c42d66e82c3a015ae74901ba738e6a63111`
- Current LearningStore: `c7a477f0534b41496d293bc1a5a0ed8365b07f0dc39ac8684360976229dedfa8`
- Query module: `60501c66f6a5ba0a5489058f9b979cca32ad977cc339c16990cb84b047378412`
- Runner: `b817641a57dcd5817587f18e0546ee6a84c47c557dcf0c225f53c09b5116e035`
- Ordering helper: `3f78275f052872256f2d5fbe5a6f4bc8d9d798a21acb701d12f6a4169578dc37`

`metadata.json` binds all measured sources and executables; `runs.jsonl` and `summary.json` retain raw
results and min/max. These are uncommitted development measurements, not release-artifact acceptance.
