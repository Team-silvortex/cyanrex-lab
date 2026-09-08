# LearningStore tmpfs control — 2026-09-08

This is a separate memory-backed-filesystem control for the
[corrected ext4 comparison](../2026-09-08-learning-snapshots-final/REPORT.md), not a replacement for it.
The same 12 configurations, three rounds, runner and two release binaries ran serially (72 invocations)
with `TMPDIR` pointing to a new benchmark-owned directory under the existing `/dev/shm` tmpfs.
No filesystem was mounted, no production setting changed, and all synthetic scratch data was removed.

Reproduce by creating a fresh empty directory on a sufficiently sized existing tmpfs and setting
`TMPDIR` for `node scripts/bench-learning-store.mjs <new-output-directory> <baseline-binary>` only.
The fixture and temporary replacement file coexist during writes; allow ample RAM and free tmpfs space.
This test does not recommend tmpfs for durable classroom history.

| Write configuration | Mean ms, before → after | p95 ms, before → after |
|---|---:|---:|
| 1,000 rows / 1,112-byte source | 2.764 → 2.098 | 4.977 → 3.483 |
| 10,000 / 1,112 | 27.891 → 21.216 | 44.792 → 32.641 |
| 50,000 / 1,112 | 175.454 → 136.561 | 191.358 → 140.284 |
| 50,000 / 4,184 | 422.038 → 366.575 | 493.040 → 374.619 |

Values are medians of three run statistics. Each record run contains ten writes, so its nearest-rank
p95 is its maximum, not a reliable service tail estimate. Every large-source after-run mean
(357.8 / 366.6 / 372.8 ms) was below its paired baseline (417.9 / 423.0 / 422.0 ms). Ext4 did not show
that consistency and had a higher after median mean; both observations remain in the main report.

The experiment supports a reduction in copy/CPU work while showing that whole-file disk behavior
still needs separate work. It does not identify a specific I/O cause, quantify fsync durability, or
establish HTTP/PostgreSQL/browser/kernel throughput. Process RSS excludes some filesystem-owned pages;
do not interpret it as the total RAM cost of using tmpfs.

Environment: Ryzen 7 7735H, Linux 7.0.0-30-generic, rustc 1.95.0, 16 Tokio workers, shared desktop.
No heavy build/test ran concurrently; no CPU pinning, cache eviction or fsync was introduced. All
source hashes, counters, orders and read/write results are in the adjacent metadata and JSON files.

- Baseline binary: `0a95e997567120ae89af89fca232b198171fb53b016fedaa08e398096e286bd1`
- Current binary: `23604a1b10f5ffbdcf3997b134311c7b72ce7985e32d785ce5a45b682bf66185`
- Runner: `b817641a57dcd5817587f18e0546ee6a84c47c557dcf0c225f53c09b5116e035`

Source base remains `5fa774110a50e8835422402014182827c21d3044` plus Unreleased work. No release,
commit, tag or push was created.
