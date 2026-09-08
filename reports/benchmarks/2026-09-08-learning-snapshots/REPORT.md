# LearningStore first-pass diagnostics — superseded ordering, 2026-09-08

These 72 before/after measurements are retained unchanged, but are not the accepted comparison.
The initial runner rotated configurations by five positions per round and selected pair order from
`round + rotated_index`. Those parity changes cancelled, leaving each configuration in the same
before/after order across all three rounds. Filesystem/cache/order effects were therefore not balanced
as intended. A new regression reproduced this flaw, and the runner now uses the original case index.

Use the [corrected disk comparison](../2026-09-08-learning-snapshots-final/REPORT.md) and its separate
tmpfs control for the final findings. They use the same two release binaries and unchanged public
`mainline_bench.rs` harness; the ordering helper, runner fingerprints and cfg(test)-only assertion
changed. This directory's metadata and raw results were not rewritten to describe the new runner.

The first pass showed promising warm reads and lower write peaks, but large write tails varied heavily.
For 50,000 larger-source records, the three after-run p95s were 1,422 / 2,996 / 1,375 ms versus
759 / 1,128 / 1,876 ms before. Those values are diagnostic, not evidence of a stable speedup or a
production capacity guarantee. Corrected-order disk and tmpfs experiments were run separately.

A new aggregate-response test also initially searched for the generic phrase `source `, incorrectly
matching legitimate automated feedback text. It was corrected to check for a submitted-source JSON
field; no production behavior was changed to satisfy that assertion.

- Baseline binary: `0a95e997567120ae89af89fca232b198171fb53b016fedaa08e398096e286bd1`
- Current binary: `23604a1b10f5ffbdcf3997b134311c7b72ce7985e32d785ce5a45b682bf66185`
- Source base: `5fa774110a50e8835422402014182827c21d3044`, version `0.3.4`, plus Unreleased work.

This used synthetic local files only, with no PostgreSQL, HTTP, compiler, Agent or kernel execution.
Full source fingerprints and all measurements remain in `metadata.json`, `runs.jsonl` and `summary.json`.
