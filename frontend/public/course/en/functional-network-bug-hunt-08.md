# Chain-guided bug hunt 08: event retention and persistence ordering

[简体中文](../zh-CN/functional-network-bug-hunt-08.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-13/bug-hunt-08.json)

Date: 2026-09-13. Commit baseline: `main@705fecc4415d678180c3a6b5f7eefb507f4788e6`, version 0.3.9.
Pass 07 was already uncommitted at the start and is preserved. This pass does not commit, bump, push or
deploy. The original 0.3.8 map and all passes 01–07 retain their original files, fingerprints and logs.

## Scope and reproduced findings

F36 retention/overflow → F32 history/export → F34 unread/read acknowledgement → F35 deletion, including
the private replacement service API and the asynchronous PostgreSQL writer. No route, permission, wire,
schema, SDK or frontend-product change is introduced in this pass.

| Finding | Reproduction | Repair |
|---|---|---|
| BH08-01 | Real batch and replacement writes produced zero SQL rows, silently falling back to memory. PostgreSQL reported duplicated `VALUES`. | Let QueryBuilder emit its own keyword and bind native JSON; assert real rows and Unicode/nested payload round trips without fallback. |
| BH08-02 | Once SQL insertion worked, paused queued events reappeared after deletion, became unread after acknowledgement and restored 80 rows after a 50-row retention update. | Per-owner admission plus a barrier through the ordered writer before mutations. Admitted SQL callers retain ownership through cache publication even after cancellation. |
| BH08-03 | Full queues spawned bypass writers instead of bounding work. | Full/closed queues warn and latch the existing volatile fallback; no per-event bypass task. Barrier and selected schema/settings/mutation stages have ten-second application deadlines. |
| BH08-04 | Cancelled retention/replacement split memory state; rejected SQL settings already changed cache/history. | Acquire memory locks before editing; settings/trim and replacement use transactions before memory publication. Rejected replacement cannot erase prior durable rows. |
| BH08-05 | DropNew retained stale capacity after deletion; late cold settings could overwrite newer cache; replacement accepted foreign-owner events. | Invalidate mutation capacity caches, preserve newer cached settings and reject foreign-owner replacements before changes. |
| BH08-06 | Equal-time events were reordered by event-type spelling and retention discarded later publications. | Stable timestamp batch ordering and timestamp/ID query ordering preserve equal-time publication order. |

## TDD and follow-up evidence

The initial four memory/queue cases and five PostgreSQL cases failed. Three SQL race cases were initially
masked by the broken INSERT rather than proving ordering failures. Two dedicated real-insertion cases
also failed. After only the SQL/JSON correction those two passed, while all five deeper SQL cases still
failed with the actual stale-state outcomes. Those stages have separate logs.

The first complete repair passed 33 selected memory/helper tests and eight PostgreSQL tests. The next
two tie-order cases were 1 passed/1 failed; stable ordering repaired the failure. Expanded SQL coverage
then passed 17 cases. A follow-up test exposed head-of-line blocking in the first global admission lock
(1 failed); admission is now per owner. This was a refactor regression, not another original finding.
The CI selection guard also failed before the explicit test list was added.

New cases: seven ordinary Rust queue/memory/deadline tests, 17 ignored-by-default real PostgreSQL cases,
and one CI-selection tooling test: **25 total**, already included in suite totals below.

## Verification

| Check | Result | Boundary |
|---|---|---|
| Full default Rust suite | 295 passed, 46 ignored | Unit, route, module, package/SSH-tool fixtures; no live kernel or deployment. Includes seven new non-DB cases. |
| Focused event/helper suite | 36 passed, 20 ignored | Includes real ten-second stalled barrier/operation deadlines; existing retention, filtering and owner-fanout regressions. Overlaps the default suite. |
| Real event PostgreSQL suite | 18 passed | 17 new plus the existing deletion test. Private disposable PostgreSQL 16.15 over a Unix socket, unique schema per case, controlled queue draining and SQL table locks. |
| Portable frontend regressions | 93 passed | Prior browser transport/filter contracts, including pass 07. No new frontend behavior. |
| Production build and standalone TypeScript | Passed | 18 generated routes; no dependency/version/type-cache change. Browser suites were not rerun. |
| Common gate | 66 passed + tool checks passed | Length/version/course parity, OpenAPI/API/SDK compatibility and Rust formatting; isolated tool fixtures, not a new dependency audit or live deployment. |
| Frozen map | Expected drift | Existing 0.3.8 → 0.3.9 version drift and 22 cumulative source drifts; original fingerprints unchanged, no operation/access/catalog/page/count drift. |

The new SQL cases are explicitly selected in CI against its disposable PostgreSQL service, with a
nonempty/exact-name guard. They remain ignored in the ordinary Rust run; that run alone is not SQL
acceptance. Node 24.19.0 was used locally; CI still uses Node 22 and no remote run was triggered here.
See [Event Stream Recovery](event-stream.md#persistence-ordering-and-retention) for reproduction.

All 92 historical map/report/log files match the starting worktree byte for byte. The final PostgreSQL
rerun also passed 18/18; every owned schema and client connection was gone before stopping the temporary
cluster. Logs retain each test stage separately; no past result is overwritten or relabelled.

## Limits and remaining work

- Ordering is per owner within one EventBus/Engine; the shared queue, pool, memory and CPU are not
  tenant isolation or a global byte/owner quota. Owner admission metadata, like existing owner history,
  is retained for the process lifetime. A slow database can still cause shared queue pressure.
- A barrier orders earlier publications, not all future event producers or snapshots. Snapshot and
  unread reads retain independent fallback behavior; a snapshot can still trail live publication.
- Existing fallback remains volatile and latches until restart. Read/deletion responses can still
  acknowledge memory changes, not durable cross-process deletion/read state. Review volatile events
  before restart; restoration/reconciliation is not automatic.
- A stage deadline stops application waiting, not necessarily the database statement. It is not a total
  HTTP deadline or proof of rollback. Known transaction failures were tested; lost commit acknowledgements,
  process crashes and multi-Engine concurrency were not. Existing background INSERT retry ambiguity remains.
- Equal timestamps have deterministic ties; arbitrary timestamp order, database precision and snapshot/live
  overlap are not converted into event IDs, cursor replay or exactly-once semantics. DropNew cold-start
  admission versus already-full SQL history and worker shutdown/drain remain separate inspection targets.
- Replacement is a private service helper, not a new import route. Transactions cover settings/trim or
  replacement, not the whole publication/HTTP/recovery lifecycle. Schema and raw Event formats are unchanged.
- No deployment account/event collection/database, deployment `.env`, live Engine/Agent, SSH/LAN or kernel
  mutation was used. No new dependency audit, benchmark, SDK runtime, browser-to-real-Engine acceptance,
  release artifact or remote CI run was performed. Default Rust includes isolated compiler/tool fixtures.

Next: F32/F34 storage-read failure and restart/fallback consistency, with F36 cold DropNew admission and
writer shutdown as related boundaries. These are pending inspections, not authority to touch real events.
