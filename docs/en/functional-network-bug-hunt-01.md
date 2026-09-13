# Chain-guided bug hunt 01: learning and persistence

[简体中文](../zh-CN/functional-network-bug-hunt-01.md) · [Functional map](functional-network.md) ·
[Machine-readable record](../../reports/functional-network/2026-09-13/bug-hunt-01.json)

Date: 2026-09-13. Baseline `main@c26a529aecc0ab64d0a1743be10972e5d8ac137b`, version 0.3.8.
Fixes remain in the worktree; no version bump, commit, push or deployment was performed in this pass.

## Traversed scope

F20 run → F28 assessment/recording → F27 progress, F29 history/resume, F30 teacher review →
F31 feedback → D03 persistence. Focus: E40 and E45–E49, read/write failures, identity/ownership,
feedback revision conflicts and request cancellation. The existing run/event integration uses a
non-kernel synthetic driver; it is not real eBPF execution acceptance.

## Reproduced and fixed

| ID | Boundary | Previous failure | Fix |
|---|---|---|---|
| BH01-01 | D03 → F27/F29/F30 | Four reads returned 200 empty history/zero progress/empty classroom for corrupt local data; selected SQL failures could also read stale local state | Fallible reads, generic 500, no fallback for a selected SQL read; successful projections/storage failures are no-store |
| BH01-02 | F28/F31 → D03 | Learning files/directories relied on ambient umask; newly created directories reproduced as 0755 | New Unix directories 0700 and snapshots 0600; existing parent permissions untouched |
| BH01-03 | F28/F31 → D03 | Fixed temporary names followed existing symlinks and truncated their targets | Exclusive random same-directory temporary creation; pre-existing temporary paths untouched |
| BH01-04 | F28/F31 → D03 | Failed final rename left temporary source/feedback data | Scope-based cleanup of this commit's temporary file; previous snapshot retained and retryable |

The first run against the old implementation failed all seven new regressions: four HTTP reads and
three persistence boundaries; the existing learning integration passed. All eight pass after the fixes.
Two closed-pool failure tests, one legitimate empty-classroom test and an API error-contract test were
also added. Closed lazy pools are never connected to a database; tests cover schema readiness,
cold/warm local snapshots and repeated errors.

Failed reads must not overwrite corrupt files, leak stored values or execute a lab. Repairing the test
file restores reads with the original record and feedback revision. Authentication, teacher filtering,
ownership, legacy JSON, concurrent feedback conflicts and completion of admitted cancelled commits remain.

## Regression entry points

- [Learning reads/repair](../../engine/tests/module_boundaries/learning_reads.rs): four router paths, auth/teacher guards, 500/no-store and recovery.
- [Persistence boundaries](../../engine/tests/module_boundaries/learning_persistence.rs): private permissions, unchanged symlink targets, failed-write cleanup and retry.
- [Backend read failures](../../engine/src/services/learning_store/read_failure_tests.rs): selected SQL does not fall back, repeat failures preserve cache, legitimate empty reads do not write.
- [Cancellation](../../engine/src/services/learning_store/cancellation_tests.rs) and [snapshots](../../engine/src/services/learning_store/snapshot_tests.rs): retained guarantees; fault injection now blocks final replacement rather than the obsolete fixed temporary name.
- [Read-error contract](../../scripts/tests/learningReadContract.test.mjs): explicit 500, no-store, unchanged access and successful payloads.

## Verification and exclusions

Focused learning-store tests: 31 passed, one real PostgreSQL test intentionally ignored. Focused learning
chain tests: eight passed. Full backend: 273 passed, 29 intentionally ignored. Common gate: 63 Node tests
plus contract, format and fixture-tool checks passed. Benchmark examples compile; benchmarks were not run.
Detailed output is in the record above; overlapping test sets must not be added together. All data fixtures are newly created synthetic
directories with scoped cleanup, not deployed learning data.

This pass did not run real PostgreSQL, browser-to-Engine, SSH, LAN or kernel acceptance, or benchmarks.
It does not change legacy run-record write fallback, existing directory permissions, cross-process
coordination or crash recovery, and adds no fsync. Process kill, power loss or cleanup errors can still
leave temporary files; loading ignores them rather than recovering automatically. Parent data paths
require trusted ownership. Successfully loaded snapshots still do not watch external edits.

The map JSON and original `verification.json` remain pre-fix evidence; fingerprints are not auto-refreshed.
Four referenced files changed, so `check-functional-network.mjs` is expected to report those four source
drifts, with no operation/access/page/lab/template inventory changes. New regression files are separately
listed in this pass's record.

Suggested next segment: F18/F26 compiler checks → Runner/Agent submission, polling, cancellation and
result ownership. This is a proposed next traversal, not new focused acceptance of that segment.
