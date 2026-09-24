# Chain-guided bug hunt 10: honest event reads and retry after repair

[简体中文](../zh-CN/functional-network-bug-hunt-10.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-24/bug-hunt-10.json)

Date: 2026-09-24. Git base: `main@bfecf77292bb87c01d69b8bbd3dc600de1f29e58`, version 0.4.0.
The starting worktree already contained uncommitted pass 09 changes; this is not a clean-release test.
No version change, commit, tag, push or deployment. The original inventory and passes 01–09 are preserved.

## Scope and findings

F32 authenticated history and JSON/CSV export → F34 unread, across HTTP query validation, storage
selection, warm/cold memory, SQL failure/repair and fresh EventBus state. Tests use synthetic owners and
a disposable PostgreSQL 16.15 cluster on a private Unix socket, never real classroom events or accounts.

| ID | Reproduction | Fix |
|---|---|---|
| BH10-01 | Selected SQL read failures looked like successful partial/empty history, downloads or zero unread; reads could latch persistence off. Pool waits lacked the new read deadline. Previously latched fallback could expose volatile rows as authoritative. | HTTP reads use fallible service methods: generic no-store `503`, no failed download attachment, and a ten-second cooperative wait including schema/pool work. A read failure does not disable persistence. Configured invalid/unavailable or already-latched stores are not explicit memory-only mode. |
| BH10-02 | Invalid/unknown filters and unsupported formats could silently widen/alter a query; overflowing relative windows panicked. Successful/error responses lacked a consistent no-store contract. | Validate extraction, filters, RFC3339 dates, format and checked relative-time arithmetic before storage; reject invalid/reversed ranges with no-store `400`. Freeze one cutoff for either backend. |
| BH10-03 | Incompatible payload columns fabricated `{}` and timestamp decoding panicked. After the first fix, two column-repair retries still failed because cached history SELECT result types were stale. | Decode every history column fallibly; report unavailable without fabricating data. Do not retain history SELECT prepared statements across reads, so a subsequent read can recover after column-type repair. No performance claim is made. |

Successful response shapes, session-derived ownership, inclusive exact timestamps, zero-window semantics,
limit defaults/caps and legacy ignored query fields remain. Authentication is still checked before reads.
No new route, Event/database schema or SDK signature is introduced. OpenAPI now records the failure and
no-store contracts. Legacy infallible Rust snapshot/unread helpers remain best-effort compatibility APIs;
the HTTP endpoints no longer use them as authoritative reads.

## Failure-first evidence and coverage

Before implementation, all **three HTTP and eight PostgreSQL** regression cases failed; the contract/CI
check had two failures among five cases. The first HTTP authoring run incorrectly treated a
100,000,000,000-minute window as Chrono overflow: it fits Chrono's range. The corrected red run uses
1,000,000,000,000 minutes for date overflow, plus `i64::MAX` for duration overflow. Both logs are retained;
the mistaken assertion is not a defect claim or an exhaustive PostgreSQL calendar-domain test.

The first SQL fix passed 31/33 cases. The remaining payload/timestamp repair failures led to a separate
diagnostic showing `cached plan must not change result type`; after the SELECT change all 33 passed.
The final run includes cancellation and unauthenticated-read guards. Temporary diagnostic tracing was
removed. Two service-level guards and the final assertion extensions were added after the initial red runs.

Added **14 unique cases: five ordinary Rust, eight ignored PostgreSQL and one Node contract test**.
An existing CI-selection test was extended; it is not an additional case. Totals below overlap, not sums.

| Check | Result | Boundary |
|---|---|---|
| Focused HTTP | 8 passed | Three new plus five existing event/learning-boundary cases. |
| Default event/filter suite | 41 passed, 35 ignored | Focused run overlaps the complete final gate. |
| Real event PostgreSQL | 33 passed | Previous 25 plus eight new; warm/cold failures, owner isolation, repair, schema error, pool exhaustion/cancellation, prior latch and incompatible payload/timestamp types. |
| Contract and CI selection | 5 passed | Each new ignored case explicitly selected with nonempty guards; not a remote CI run. |
| All default Rust | 303 passed, 61 ignored | Ignored PostgreSQL cases are not counted as default passes. |
| Common, frontend and SDK gate | All passed | 67 common tests, 93 frontend, SDK 13 runtime/3 package plus type checks; production build generated 18 routes. Frontend/SDK production dependency audits reported zero vulnerabilities. |
| Frozen map | Expected drift | 0.3.8 → 0.4.0 and 23 cumulative source changes; no access/API/page/catalog-count drift. |

The final full gate used Node 24.19.0 / Rust 1.95.0. Documentation mirrors and common preflight were
rechecked after recording results; runtime sources were unchanged from the full gate. All 240 prior
inventory/report/log/history-mirror files remain byte-identical, including uncommitted pass 09 evidence.
The machine record links source fingerprints, raw red/intermediate/green logs and cleanup receipts.
Before stopping the temporary cluster, both test schemas and other clients were zero. Only this pass's
cluster and regenerated build caches were removed; dependencies, source, evidence and deployment remain.

## Limits and next slice

- Ten seconds bounds cooperative async read work, not authentication, serialization, CPU stalls or proof
  that PostgreSQL cancelled an already-running statement. Abandoning a read does not disable persistence.
- Read errors are retryable only when no independent writer has latched fallback. A prior latch remains
  unavailable until fresh state; no automatic reconciliation of volatile events is added.
- Fresh EventBus construction against retained fixture rows is not process-crash, network-partition,
  lost-commit or power-loss recovery. No cross-Engine/external-writer coordination or read queue barrier
  is added; a snapshot can still trail live delivery.
- Existing volatile write, mark-read and deletion acknowledgements are unchanged. Legacy infallible
  helpers retain fallback behavior; only the new fallible HTTP path is authoritative.
- This is not exhaustive enum/corrupt-data validation, PostgreSQL calendar-domain validation, CSV
  spreadsheet/formula/CR safety, byte quotas, durable replay or exactly-once delivery.
- History SELECT planning reuse changed without a new benchmark. Remote CI, Rust dependency auditing,
  optional browser suites, real SSH/LAN/kernel/deployment and release-artifact acceptance were not run.

Next: F34 mark-read / F35 delete acknowledgement under storage failure and recovery, using isolated
fixtures. A successful read does not by itself establish durable mutation success.
