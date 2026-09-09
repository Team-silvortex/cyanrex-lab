# LearningStore cold initialization — 2026-09-08

## Outcome

The candidate moves local first-load I/O, whole-file UTF-8 validation and JSON decoding to one admitted
blocking worker. It directly creates shared records, keeps admission after caller cancellation, publishes
only complete validated input and treats only `NotFound` as an empty store. Warm reads, stored JSON,
authorization, SQL and public responses are unchanged.

This improves async responsiveness and lowers process peak RSS, **not all first-read latency**. The
largest fixture reduces the median per-process maximum timer lateness from 218.641 to 2.037 ms, but
increases first-read latency from 405.922 to 485.521 ms (+19.6%). This tradeoff is retained explicitly:
first readers still wait for decoding, while other work on the async executor can progress. The result
does not prove faster disk access, HTTP health latency, classroom capacity or a scheduling bound.

## Provenance and method

- Baseline library: clean Engine production source at `5c663cccb3fa082222c5989d189f83561f344118` (`0.3.5`),
  with the new read-only example added before production edits. This is **not** a released artifact.
- Baseline `learning_store.rs` SHA-256: `2ffe24c59b51680c46531040d0e6e6500350ffa89731690d65ccb0a01198fadd`.
- Identical `engine/examples/learning_load_bench.rs` SHA-256 on both builds:
  `ec7427c820e29d04f90e7031c7041a3bb396761bfe36356a88afcc8013a12ee6`.
- Frozen baseline binary SHA-256: `0f145f79299740904aed51d0c8195be6ad8fdd2b71b9086056c05961144fed09`.
- Candidate binary SHA-256: `243f20c5ab6e2c16fd17fb807c2c8a690139e56634b2f7771fa7e47e457ccad3`.
- Candidate source and runner fingerprints, fixture hashes and environment: [metadata.json](metadata.json).
  All individual observations: [runs.jsonl](runs.jsonl); medians/min/max: [summary.json](summary.json).

The baseline was copied to `/tmp/cyanrex-learning-load-baseline.vzbTrJ/learning_load_bench` before
changing the library. The runner refuses existing output and rebuilding over the same baseline path.
Both builds use locked, offline dependencies and release mode. No dependency or version changes were made.

```sh
node scripts/bench-learning-load.mjs reports/benchmarks/2026-09-08-learning-cold-load-final \
  /tmp/cyanrex-learning-load-baseline.vzbTrJ/learning_load_bench
```

Use a new output directory on repetition. Five rotated rounds × four fixtures × two binaries give
**40 measured processes**, plus two unmeasured small-fixture warmups. Each pair swaps before/after order
every round. Only the `recent` cases of the shared matrix supply fixture dimensions; their names retain
the padding value, while actual source sizes are 1,112 and 4,184 bytes. Thirty synthetic students use
ASCII source padding and fixed timestamps. No real student files are read or changed.

Fixture generation happens in the Node parent before measured processes. Both binaries read the same
four files; size and SHA-256 were verified unchanged after all reads, and only the runner's temporary
files were removed. File sizes are 1,538,552; 15,395,552; 77,022,222; and 230,622,222 bytes. Files were
freshly written on local ext4 `/tmp` and remain in the OS page cache: these are **cold-process/store,
not cold-disk** observations. There is no page-cache eviction, CPU pinning or fsync.

The shared host is an AMD Ryzen 7 7735H, Linux `7.0.0-30-generic`, Rust 1.95.0, Node 18.19.1; the root
filesystem was 93% used. Load averages changed from `[1.61, 1.79, 1.80]` to `[3.48, 2.22, 1.94]`. No other
agent-initiated test/build ran during measurements; this was not an otherwise idle or dedicated machine.

Each fresh process starts a single-thread Tokio runtime and a 2 ms interval timer, waits for the timer
to be ready, then times the first recent-20 query. A 4 ms untimed grace allows an overdue tick to run
before stopping the timer. Maximum lateness is measured against scheduled ticks; missed ticks are skipped,
so tick counts differ and this is not a sample-by-sample tail comparison. A separate warm recent-20 query
is timed; row ownership/source length, identical warm result and teacher totals are checked. GNU time
RSS/CPU/wall includes runtime startup, first load, queries, overview validation and teardown, excluding
fixture creation/deletion. CPU/wall reports have centisecond resolution; small values of zero are not zero cost.

## Results

All medians below are **five process observations, not request p95**. Ranges are the five-run min–max.

| Rows / source bytes | First read ms, before → after | Change | Maximum timer lateness ms, before → after | Peak RSS MiB, before → after |
|---|---:|---:|---:|---:|
| 1,000 / 1,112 | 4.266 → 3.925 | −8.0% | 3.341 → 1.901 | 8.680 → 8.078 |
| 10,000 / 1,112 | 33.065 → 37.538 | +13.5% | 20.136 → 1.990 | 38.098 → 35.316 |
| 50,000 / 1,112 | 163.150 → 180.406 | +10.6% | 100.903 → 2.026 | 168.680 → 155.977 |
| 50,000 / 4,184 | 405.922 → 485.521 | +19.6% | 218.641 → 2.037 | 461.742 → 448.914 |

| Rows / source bytes | First read range ms, before / after | Timer maximum range ms, before / after | Warm read median ms, before → after |
|---|---:|---:|---:|
| 1,000 / 1,112 | 3.683–4.401 / 3.801–4.706 | 2.760–3.469 / 1.192–2.009 | 0.041 → 0.053 |
| 10,000 / 1,112 | 32.390–35.831 / 36.115–48.099 | 19.464–22.905 / 1.965–2.010 | 0.168 → 0.180 |
| 50,000 / 1,112 | 157.157–172.827 / 178.016–190.323 | 95.388–105.903 / 2.002–2.499 | 0.764 → 0.822 |
| 50,000 / 4,184 | 391.710–421.315 / 472.943–556.985 | 204.771–224.402 / 2.029–3.231 | 0.996 → 0.998 |

The candidate's 10k/50k first reads are slower even though the synthetic timer continues to run. Warm
control medians also rise slightly despite unchanged query logic; five observations are not enough to
attribute this to one mechanism. Worker scheduling, allocator behavior, timer work and host noise may
contribute, but were not isolated. This experiment does not establish a cause for the first-read slowdown.
Do not hide regressions or infer a general latency improvement from lower memory/executor-stall results.

## Correctness and remaining work

Six new Rust regressions cover queued/decoded cancellation, concurrent initial readers and writes,
missing versus invalid paths, retry after failure and differential legacy decoding. The decoder matrix
includes unknown fields, tuple-form records, invalid UTF-8 (also inside ignored fields), partial/trailing
JSON and invalid record types. Tests reject partial publication and confirm successful-load reuse.
Three cases first failed on the old loader; the invalid-UTF-8 ignored-field case additionally failed on
the prototype and passed after restoring the previous whole-file UTF-8 validation. The focused
LearningStore suite passes 28 tests; its external PostgreSQL regression remains ignored.

The six cold-load tests passed 20 consecutive runs (120 checks). The full local
`./scripts/quality-gate.sh --security --no-npm-install` passed using isolated `CYANREX_DATA_DIR`, with
`DATABASE_URL` and `CYANREX_BENCH_DATABASE_URL` unset and the bundled Node 24 runtime for Next.js:
211 Rust tests passed / 5 ignored, 46 common tooling tests, 29 frontend regressions, 11 SDK tests plus
3 package smoke tests and the compile-time fixture, and 17 static routes. RustSec and both production
npm audits reported zero vulnerabilities. Quality log: `/tmp/cyanrex-learning-cold-quality.YkTOrb.log`;
repeat-test log: `/tmp/cyanrex-learning-cold-stress.joKpQj.log`. These local logs are not release evidence.

A common JS regression checks that all four cold fixtures appear once per round and alternate pair order.
The first 40-run [byte-slice prototype](../2026-09-08-learning-cold-load/README.md) is retained independently:
it weakened invalid-UTF-8 rejection and regressed large first reads. Its hashes/results are not replaced
with this candidate, and the two experiments are not pooled.

Initialization shares admission with file commits. Before admission, cancellation does nothing; after
dispatch, the worker survives its caller and marks readiness only after complete publication. Readiness
does not cache errors, so failed waiting callers may retry serially. Generic warnings omit stored values.
See [Learning Record Storage](../../../docs/en/learning-storage.md) for the exact lifecycle and retry rules.

Input is still a full string alongside all decoded records, and there is no file-size/retention limit.
The runtime's blocking pool remains shared; one Store's admission does not bound all stores or isolate
other users. Writes still rewrite the whole JSON file without fsync, recovery, cross-process locking or
new append idempotency. Process kill, panic, shutdown and stuck I/O are outside the request-cancellation
completion guarantee. No real PostgreSQL, HTTP/browser, LAN, privileged kernel, power-loss, Docker install
or release artifact acceptance is claimed by this matrix.
