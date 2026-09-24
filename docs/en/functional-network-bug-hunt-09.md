# Chain-guided bug hunt 09: cold event admission and writer lifetime

[简体中文](../zh-CN/functional-network-bug-hunt-09.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-24/bug-hunt-09.json)

Date: 2026-09-24. Clean baseline: `main@bfecf77292bb87c01d69b8bbd3dc600de1f29e58`, version 0.4.0.
Continues pass 08's follow-ups without versioning, committing, pushing or deploying. The original 0.3.8
inventory and passes 01–08 keep their historical evidence and fingerprints.

## Scope and findings

F36 retention/overflow → F33 live publication, F32 history and F34 unread, including F35 deletion, private
replacement, settings changes and persistence-worker exit. Synthetic events and disposable PostgreSQL
exercise service boundaries only; no real accounts, data, kernel or deployment operations.

| ID | Reproduction | Fix |
|---|---|---|
| BH09-01 | Cold DropNew checked memory only: a full SQL history published 3 more events; 2 free slots published 10; concurrent clones accepted 30 with only 10 slots. The writer later dropped them, diverging live/unread from durable history. | Load the owner's SQL baseline before publication and count locally admitted pending events. Reject full capacity before visible changes; keep this counter separate from expiring writer counts. Mutations rebase it, cancellation cannot reserve a slot. |
| BH09-02 | The writer retained its own sender. Empty, disabled and successfully drained queues never terminated after all external EventBus handles left. | Release the worker's producer handle on entry. With a living runtime, the final external producer's drop allows healthy queued work to drain and state to be released. |

## Failure-first evidence and coverage

Before implementation changes, both lifecycle cases and the initial four PostgreSQL cases failed. The
expanded seven SQL cases also all failed. CI selection failed before explicitly adding each new ignored
case to the disposable PostgreSQL job. Test-authoring compilation errors are separate diagnostics, not
bug reproductions. Cancellation, three capacity mutations and fallback/recreation are expanded boundary
checks, not additional independent defect counts.

Added **three ordinary Rust and seven ignored PostgreSQL tests**. One lock-order guard was added after
the fix; partial-capacity coverage also crosses one second without flushing to ensure pending slots never
expire. Added cases are already included in the totals below, not counted twice.

| Check | Result | Boundary |
|---|---|---|
| Default event/filter suite | 39 passed, 27 ignored | Includes existing fanout/retention/cancellation and two real ten-second deadline checks; ignored cases are 25 SQL and 2 benchmarks. |
| Real event PostgreSQL | 25 passed | Previous 18 plus 7 new; PostgreSQL 16.15, private Unix socket, independent test schemas. |
| CI selection | 4 passed | Exact named selection rejects empty matches; not evidence of a remote CI run. |
| All default Rust | 298 passed, 53 ignored | Unit, route, module and isolated tool fixtures; the 25 event SQL cases run separately, not as default passes. |
| Common, frontend and SDK gate | All passed | 66 common tests, 93 frontend, SDK 13 runtime/3 package plus type checks; production build generated 18 routes. Frontend/SDK production dependency audits both reported zero vulnerabilities. |
| Frozen map | Expected drift | 0.3.8 → 0.4.0 and 22 cumulative source changes; no access/API/page/catalog-count drift; no fingerprints rewritten. |

SQL cases cover full/partial cold capacity, concurrent clones and owner isolation, TTL crossing, cancelled
SQL/memory-lock waiting, deletion/grown settings/replacement capacity, count-failure fallback and fresh
instance reload, plus 127 records drained across batches before exit. Ordinary cases also verify that a
remaining producer keeps the task available, the final drop releases history, disabled barriers cannot
claim durable success, and cancellation waiting for the capacity lock changes no publication state.

The seven new SQL cases also passed three consecutive repeat rounds (repeat executions, not additional
unique cases). All 218 previous inventory/report/log/history-mirror files remain byte-identical; the
machine record preserves their path list and aggregate fingerprint. Before stopping the temporary cluster,
both test schemas and other client connections were zero; only this pass's temporary cluster was removed.
The gate used Node 24.19.0 / Rust 1.95.0. Remote CI, Rust dependency auditing and optional browser suites
were not rerun.
Regenerated Rust/frontend/SDK build caches were removed after verification; source, dependencies,
reports and runtime data were retained.

## Limits and next slice

- Cold recreation constructs a fresh EventBus on the same private schema; it is not process termination,
  crash or power-loss recovery.
- Coordination covers clones of one EventBus, not external SQL writers, multiple Engines, durable replay
  or cross-process reservations. Admission metadata lives until service destruction, like existing
  history/settings; no aggregate owner/byte quota is added.
- Drain verification assumes healthy SQL and a runtime that keeps running. There is no new process
  shutdown hook, whole-worker deadline or durable ACK. Stalled SQL, lost commit replies, ambiguous retry
  duplicates and abrupt termination remain separate design/testing work.
- Count failure uses existing latched volatile fallback; restoring SQL does not merge volatile events.
  History/export/unread read-failure fallback remains unchanged and snapshots may trail live delivery.
- No route, access, Event/API, database schema, SDK or frontend product change. No real Engine/Agent,
  SSH/LAN, deployment `.env`, database volume or kernel work. No new benchmark, real browser-to-Engine
  or release-artifact acceptance.

Next: F32/F34 storage-read failures that may look empty/zero, filter boundaries, and restart/fallback
consistency. These are investigation targets, not permission to access real classroom events.
