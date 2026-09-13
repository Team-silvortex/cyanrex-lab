# Chain-guided bug hunt 06: debug sessions and event recovery

[简体中文](../zh-CN/functional-network-bug-hunt-06.md) · [Functional network](functional-network.md) ·
[Machine record](../../reports/functional-network/2026-09-13/bug-hunt-06.json)

Date: 2026-09-13. Baseline remains `main@c26a529aecc0ab64d0a1743be10972e5d8ac137b`, version 0.3.8.
Continues on five uncommitted passes; their reports/logs and original JSON inventory are preserved.
No version bump, commit, push, deployment, live Engine operation or kernel loading was performed.

## Scope

F21 debug session/trace marker → F33 live event recovery → current breakpoint view; related F32 history
snapshots and F20/F22 result retirement. Covers E38/E42/E43 and preserves explicit run/cleanup behavior.
Changes are browser observation/lifecycle handling and one pure Rust trace parser, not the sampler,
EventBus, authentication, storage or API wire format. D07 stored breakpoint format remains unchanged.

## Reproduced and fixed

| ID | Boundary | Previous behavior | Fix |
|---|---|---|---|
| BH06-01 | Session/Engine → rendered hits | Old hits/gap state appeared in the first render after context changed | Key state by Engine, session and normalized instrumented set; hide stale state before effects run |
| BH06-02 | Event → accepted breakpoint | Numeric lines included zero, negative, fractional or undeclared values; live platform events could pass; instrumented-set changes did not reset work | Require a kernel breakpoint, exact session and declared positive safe-integer line; invalidate obsolete subscriptions and skip empty sets |
| BH06-03 | Editor/model → glyphs/callbacks | Replacement left old decorations, callbacks could target the new editor, disposal left handlers registered, invalid hit lines reached Monaco | Capture editor/model ownership, dispose registrations, clean original model IDs, guard late disposal and validate hit bounds/source |
| BH06-04 | Recovery transport → completeness claim | Snapshot transport allowed caching/redirects; malformed live frames were silently discarded | Credentialed no-store/redirect-error snapshots; persistent gap notice for invalid frames without invented events or forced reconnects |
| BH06-05 | Trace marker → line number | Zero and numeric prefixes such as `47abc` were accepted; empty session identifiers matched | Require a nonempty session and complete positive decimal token; reject malformed/overflow values, preserve normal whitespace suffixes |

The initial 13 browser cases failed against the original implementation. Two new parser cases also
failed, alongside one passing existing case. A follow-up test caught an additional defect in the first
editor fix: a retained old-model disposal callback cleared the replacement model. Its model identity
guard now rejects that callback. Six more controller preservation cases cover recovery, bounded repeated
hits, empty/canonical line sets, timeout/unmount, F9/gutter/clear behavior and reference changes.

The first combined browser run was 74/75. One assertion included an old-session render legitimately
caused by a pending disconnect notification, although it intended to inspect the cleared session. It now
explicitly checks every render whose session is null, without allowing old hits in those renders. The
initial and final logs are separate; this fixture correction is not an extra production fix or a retry.

There are 30 new tests: 20 controller-browser, four portable event recovery, three Rust parser and three
production UI. They are included in the totals below, not additional passes to sum again.

## Behavior and authority retained

- The event protocol remains raw per-owner Event JSON. No IDs, cursors, payload fields, routes or roles
  changed. Session and Origin checks still precede an owner-scoped subscription; query parameters cannot
  select another owner. Trace markers and declared line sets are not authentication in a shared kernel.
- Matching records still use the existing 200-row recovery buffer and 50-hit display limit, with
  occurrence-count overlap reconciliation. The initial/reconnect snapshot can lag live persistence;
  recovery is neither atomic nor exactly-once. Successful reconnection does not erase a gap warning.
- Malformed JSON, invalid Event records and non-text frames mark a possible gap but do not trigger a
  reconnect by themselves. Valid filtered-out events do not mark gaps. Ten-second handshake/snapshot
  deadlines, jittered backoff and terminal HTTP 401/403 or socket 1008 behavior are unchanged.
- Editor changes invalidate observations without unloading programs. F9/gutter/clear actions change
  requested breakpoints for a later explicit run; clearing them does not remove already running probes.
  A matching current hit may remain highlighted until the run context is retired. Model line bounds are
  checked before decoration; this does not implement edit-aware relocation of requested breakpoint lines.

## Verification

| Check | Result | Actual boundary |
|---|---|---|
| Breakpoint + previous controller browser suites | 82 passed | 20 new + 62 previous; actual React 19 StrictMode/hooks, synthetic models/sockets/fetch and paused clock |
| All portable frontend tests | 81 passed | Four new event recovery cases; existing CI/gate entry retained |
| Production breakpoint/event/runtime/safety/logout UI | 29 passed | Three new + 26 existing; real built Next.js/Monaco; Engine HTTP/WebSocket fixtures only |
| Parser and instrumentation unit tests | 9 passed | Four parser + five existing pure instrumentation cases; no Clang or kernel sampler |
| Existing WebSocket route tests | 4 passed | Temporary authenticated loopback router; session/Origin, raw payload, owner isolation and overload close |
| Production build + explicit TypeScript | Passed | No dependency or generated type-cache changes |
| Common quality gate | 65 tests and checks passed | Length/version parity, API/SDK contracts, course mirrors, tool fixtures and Rust formatting |

Local runtime: Node 24.19.0, Chrome 153, React 19, Next.js 15.5.24 and Monaco 0.55.1. Existing CI uses
Node 22; no new remote CI run. Browser tests remain optional; see [frontend README](../../frontend/README.md).
The existing event-page browser test also blocks unexpected external HTTP. Test services/browsers and
temporary bundles were closed/removed. No real account, database or instance data was read or changed.

## Limits and next segment

This does not validate real trace collection, probe authenticity, attach success, kernel isolation,
cross-tab revocation, durable replay or subscription memory under load. Limits are record counts, not
new byte caps. Source-line relocation after edits, event-center filter render timing, historical query/
export consistency and unread state are separate inspection targets. Previous acceptance results are
historical and not counted here. No live Engine/Agent/Clang, PostgreSQL, LAN/SSH, kernel sampling, SDK
runtime, dependency audit or benchmark was run.

Next: F32 event history/filter/export → F34 unread state → F35 scoped deletion/refresh, using isolated
fixtures first. These are inspection targets, not confirmed bugs or permission to remove live events.

The frozen JSON inventory reports 16 cumulative expected source drifts: the previous 13 plus the trace
parser, breakpoint stream hook and shared event controller. Counts/access/pages/catalogs/version remain
unchanged. This pass records current source hashes and preserves all 60 earlier evidence files.
