# LearningStore streaming commits — tmpfs control, 2026-09-08

This is an independent 72-run matrix accompanying the
[primary disk report](../2026-09-08-learning-streaming-writes/REPORT.md), not a replacement for disk
measurements or a suggestion to keep classroom history in non-durable RAM.

The same paired runner and exact before/after binaries ran separately under the existing `/dev/shm`
tmpfs at 11:58:57–11:59:46 UTC. `TMPDIR` was scoped to the benchmark's newly created empty directory
`/dev/shm/cyanrex-learning-streaming.Gv10Sh`; the public harness created and removed its own child fixtures.
All children and the empty parent were removed after completion. No mount, kernel setting, production
path or live classroom data changed. No heavy builds/tests overlapped measurements.

Reproduce with a new empty directory on an existing tmpfs, without changing production configuration:

```sh
TMPDIR=<owned-empty-tmpfs-directory> node scripts/bench-learning-store.mjs <new-output-directory> <frozen-baseline-mainline-binary>
```

## Write results

Before → after, medians of three run statistics; latency in ms. The baseline already includes shared
LearningStore snapshots, not just the clean release. Source sizes below include the fixed example source.

| Rows / source bytes | Mean | p50 | p95 | Peak process RSS MiB |
|---|---:|---:|---:|---:|
| 1,000 / 1,112 | 2.278 → 2.005 | 1.978 → 1.877 | 3.807 → 2.495 | 12.10 → 9.69 |
| 10,000 / 1,112 | 21.055 → 19.484 | 19.342 → 18.952 | 31.185 → 20.761 | 62.79 → 38.91 |
| 50,000 / 1,112 | 133.322 → 91.607 | 132.940 → 91.607 | 138.723 → 93.401 | 180.36 → 165.79 |
| 50,000 / 4,184 | 354.390 → 231.370 | 354.098 → 231.217 | 361.326 → 240.870 | 475.86 → 462.68 |

The three large-source paired means were 354.390 → 231.069, 355.954 → 231.370 and 353.914 → 234.792 ms.
Run p95 ranges were 357.738–367.048 ms before and 233.517–241.708 ms after. Each write invocation has only
ten samples, so its p95 is the maximum, not an established service tail.

Whole-process memory includes fixture construction/loading/cleanup and does not include all kernel-owned
tmpfs pages. Removing a complete encoded-file allocation does not bound source retention, total memory
or file size. Both implementations still rewrite the whole file and neither fsyncs it. Read-control
measurements, all raw runs and metric ranges remain in the adjacent JSON files; see the primary report
for regressions, correctness checks, scope and the limitations of these synthetic results.

## Provenance

- Baseline binary: `23604a1b10f5ffbdcf3997b134311c7b72ce7985e32d785ce5a45b682bf66185`
- Current binary: `e4e84ece0eb3b364792eea04190e00b5a88762fd661aefca9025961d0cd67ce2`
- Unchanged public example: `5c7d2028737123d6f2aafc1a951e1c42d66e82c3a015ae74901ba738e6a63111`
- `metadata.json`: complete, with 140 source fingerprints and environment/phase provenance.
- `runs.jsonl`: all 72 invocations; `summary.json`: medians and min/max, not pooled request percentiles.

The baseline was frozen before this increment after the prior Unreleased optimizations on base
`5fa774110a50e8835422402014182827c21d3044`. This dataset does not certify disk durability, crash recovery,
HTTP/LAN/classroom capacity, real kernel behavior or release acceptance.
