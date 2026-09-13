# Chain-guided bug hunt 02: Runner/Agent lifecycle

[简体中文](../zh-CN/functional-network-bug-hunt-02.md) · [Functional map](functional-network.md) ·
[Machine-readable record](../../reports/functional-network/2026-09-13/bug-hunt-02.json)

Date: 2026-09-13. Baseline `main@c26a529aecc0ab64d0a1743be10972e5d8ac137b`, version 0.3.8.
Continues on the uncommitted first pass. The original map JSON, previous machine record and logs remain;
no version bump, commit, push or deployment was performed.

## Traversal

F18 local diagnostics / F26 selected remote checks → F24 Agent health/capacity → F25 claim, lease sync,
result and cancellation. Focus: E50–E53 and D06 ephemeral control state; existing E34 local admission/
ownership regressions were rerun. Browser diagnostics were not changed; this is not full browser acceptance
of F17→F18/F26.

## Reproduced and fixed

| ID | Boundary | Previous failure | Fix |
|---|---|---|---|
| BH02-01 | F24 → F25 claim | Free slots were treated as total capacity, subtracting active leases twice; one active job on a two-slot Agent blocked its second claim | Use `active_jobs + available_slots`; preserve reserved capacity and count cancellation-pending leases |
| BH02-02 | F25 sync → executor | `lost_job_ids` was ignored; tests still observed a successful probe result or cancellation acknowledgement for a lost lease | Reject before execution/result submission; loss wins over cancellation and is a retryable conflict |
| BH02-03 | F26 → F25 queue | Lost submission/cancellation responses could leave unclaimed jobs occupying both user slots indefinitely | Expire user checks after 35 queued seconds on the next queue access, releasing source and user-active quota |
| BH02-04 | HTTP response → Agent decoder | Unknown-length responses were fully buffered before checking 640 KiB; an unfinished oversized stream waited until timeout | Check every chunk and reject immediately above the cap; success/error bodies and the exact limit are covered |

Against the old implementation, the first nine new Rust cases produced five failures and four passes.
Those failures reproduce all four findings. They pass after the fixes. Inclusive queue expiry, retention,
claimed deadlines, staff queue policy and auth/CSRF/owner cancellation cases were added as well: twelve
new Rust tests and two API documentation-contract tests in total.

## Retained guarantees

- Valid work completes and valid cancellation acknowledges; a lease observed as lost is not executed/reported.
- Stale heartbeats cannot exceed queue admission; registered maximum capacity does not override reservations.
- The 35-second queue window matches the current browser wait limit and affects only user-owned remote checks.
  Claimed execution deadlines are not shortened; staff-managed unowned probe/compile jobs retain explicit cancellation.
- Anonymous, wrong-Origin and other-owner/management-job cancellation requests return 401/403/404 without mutation.
- Streaming tests exceed the cap and never send EOF, proving early rejection rather than just a final error code.

## Regression entry points and verification

- [Capacity routes](../../engine/tests/routes_tdd/runner_capacity.inc.rs): real signed heartbeat/claim routing, reservations and cancellation-pending admission.
- [Cancellation boundary](../../engine/tests/routes_tdd/remote_check_boundary.inc.rs): auth, CSRF, ownership and management-job separation.
- [Client lease checks](../../engine/src/services/runner_agent_client/boundary_tests.rs): temporary loopback HTTP fixtures, not a deployed Engine or remote Agent.
- [Response streaming](../../engine/src/services/runner_agent_client/response_tests.rs): no length header, oversized unfinished streams, exact limit and error classification.
- [Queue regressions](../../engine/src/services/runner_job_queue/tests.inc.rs): injected timestamps, no real 35-second sleep.
- [API contract](../../scripts/tests/runnerBoundaryContract.test.mjs): queue lifetime, capacity/lost-lease notes with unchanged signed access and success shapes.

Runner service subset: 39 passed. Capacity route subset: two passed. Full backend: 285 passed,
29 intentionally ignored. Common gate: 65 Node tests plus contract, format and fixture-tool checks passed;
output is in the machine record. Overlapping test sets must not be added together. Existing executor tests compile synthetic source when local Clang
is installed and clean their workspace without kernel loading; they do not guarantee compilation success.

Not run: live LAN/SSH, remote Engine/Agent/Clang integration, real PostgreSQL, kernel attach/events,
browser acceptance, fresh benchmarks, dependency audits, frontend builds or SDK runtime tests.

## Limits and next traversal

Reaping is access-triggered, not a background timer. An idle queue need not release memory exactly at
35 seconds; terminal metadata still counts against the global 512-record/15-minute retention boundary.
There is no new idempotency key, automatic retry, remote loading or client-side job concurrency.
Lease checks are pre-execution, not continuous monitoring or immediate Clang interruption. The response
cap is not a full-process memory guarantee.

Source inspection also found that browser diagnostic cache keys lack an Engine/session dimension;
late async responses and cancellation need focused reproduction. These were not changed or dynamically
validated here. Next: F17→F18/F26 browser cache isolation, stale responses and backend/draft switching.

Original map fingerprints are not refreshed. At this pass's completion, seven cumulative source drifts
are expected: the first pass's four plus Runner routes, Agent client and job queue. API/access/page/template/
lab inventories are unchanged. Later passes may change current files; interpret historical evidence by
its pass and fingerprints.
