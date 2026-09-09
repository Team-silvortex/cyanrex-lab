# Learning cold-load prototype — 2026-09-08

This immutable 40-process diagnostic matrix measures the first prototype, **not the final loader**.
The baseline is the `5c663cccb3fa082222c5989d189f83561f344118` (`0.3.5`) Engine library plus the new,
read-only `learning_load_bench.rs` harness. It was built before production edits and copied outside
the target directory. The candidate moves parsing to an admitted blocking worker and directly builds
reference-counted records, but uses `fs::read` / `serde_json::from_slice`.

- Baseline binary SHA-256: `0f145f79299740904aed51d0c8195be6ad8fdd2b71b9086056c05961144fed09`.
- Candidate binary SHA-256: `c851dc69b05f5be9d113e982c5035570ebd55e00be91758ef56f9faf8fcc1a7d`.
- Identical harness SHA-256: `ec7427c820e29d04f90e7031c7041a3bb396761bfe36356a88afcc8013a12ee6`.
- Baseline `learning_store.rs` SHA-256: `2ffe24c59b51680c46531040d0e6e6500350ffa89731690d65ccb0a01198fadd`.
- [Metadata](metadata.json) binds the candidate source, binaries, fixtures and environment;
  [raw runs](runs.jsonl) and [summary](summary.json) are retained without replacement.

Five rotated rounds compare four read-only fixtures, alternating each pair's order. Fixtures contain
30 synthetic students and are created outside measured Rust processes; both binaries read identical
bytes, verified unchanged afterwards. These are fresh processes with OS page-cache-warm files on `/tmp`,
not cold-disk measurements. A single-thread Tokio runtime exposes async-executor stalls using a 2 ms
timer. Process RSS includes loading, result/overview validation and teardown, not fixture generation.
No HTTP, PostgreSQL, kernel execution, CPU pinning, fsync or cache eviction is involved.

## Diagnostic outcome

Values are medians of five process observations, **not request p95**.

| Rows / source bytes | First read ms, before → prototype | Per-process max timer delay ms, before → prototype | Peak RSS MiB, before → prototype |
|---|---:|---:|---:|
| 1,000 / 1,112 | 4.034 → 3.863 | 3.102 → 1.965 | 8.777 → 8.199 |
| 10,000 / 1,112 | 33.190 → 37.196 | 20.276 → 1.973 | 38.051 → 35.324 |
| 50,000 / 1,112 | 158.772 → 184.646 | 97.410 → 2.023 | 168.852 → 155.781 |
| 50,000 / 4,184 | 398.532 → 478.972 | 209.608 → 2.036 | 461.660 → 448.652 |

Async responsiveness improved, but larger first reads regressed by 12–20%. An additional differential
regression then demonstrated a compatibility defect: `from_slice` can skip an unknown field containing
invalid UTF-8, while the prior `read_to_string` loader rejects the entire file. This candidate was not
kept. The final loader restores whole-file UTF-8 validation and `from_str` on the blocking worker,
retaining cancellation-safe admission and per-record shared allocation.

The unchanged baseline/harness is compared again in
[the final matrix](../2026-09-08-learning-cold-load-final/README.md). Do not pool these two experiments,
rewrite this dataset's source hashes to the final tree, or treat either as release artifact acceptance.
