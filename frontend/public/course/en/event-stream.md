# Event Stream Recovery

The event center and eBPF breakpoint panel are best-effort recent-history views. They do not provide
durable subscriptions or an exactly-once audit log.

## Wire compatibility and access

- `GET /ws/events` still upgrades to a WebSocket and sends one raw `EventRecord` JSON object per text
  frame. There is no new JSON envelope, control event, cursor, sequence field, or subprotocol.
- The Engine subscribes by the authenticated session owner, never by a client-supplied username. Live
  queues are owner-scoped: another user's traffic cannot overwrite this subscription's queued events.
  Raw Event JSON, authentication, and recovery close codes are unchanged.
- The handshake now applies the existing state-change Origin/Referer policy in addition to session
  authentication. Browser clients already send Origin. Native/SDK consumers must supply an allowed
  Origin, configured through `CYANREX_CORS_ORIGINS` (or the normal frontend defaults). Missing origins
  return `403` unless the operator explicitly enables `CYANREX_ALLOW_MISSING_ORIGIN`; disallowed origins
  remain rejected. This security correction may require updates to older native clients.
- The JavaScript SDK still returns a WebSocket URL; it does not manage reconnection or replay for callers.

## Server behavior

The subscription is established before completing the upgrade. If the bounded owner-scoped receiver
reports lag, the Engine closes the socket with code `1013` and reason
`event stream lagged; reload /events` instead of silently continuing. This indicates a possible gap,
not an exact per-user number of missing events.

Each active owner has one bounded channel (currently 1,024 slots in the production Engine). Connections
for the same owner share immutable events and one lazily computed JSON representation per event; each
socket still builds/sends its own frame. No JSON is encoded until a subscriber requests it. Publishing
to an owner without subscribers does not create a queue. The last connection's drop/cancellation removes
the owner entry, with generation checks to protect simultaneous reconnections; subscriptions do not
keep the registry alive. New subscriptions are live-only and still need an HTTP history snapshot.

Capacity is **per active owner**, not one global memory budget. Aggregate queue memory grows with active
owners and payload sizes, and cached JSON adds a representation for consumed events. There is no new
connection/active-owner admission limit. History, settings, unread locks, CPU and persistence remain
shared; this eliminates cross-owner queue eviction, not all resource contention or tenant isolation.
The existing global `EventBus::subscribe()` service API remains compatible for internal callers, but
`/ws/events` does not use it and publishing avoids its payload clone when it has no receivers.

Each data send has a five-second deadline. On expiry the transport and subscription are dropped;
because a frame may be partially written, another close frame is not attempted. The peer may observe
an abnormal close (`1006`) rather than `1013`. Close-frame sends have a one-second deadline. These bounds
cover asynchronous sends, not arbitrary serialization CPU time or OS detection of a disconnected idle
connection. No new heartbeat or active-session revocation mechanism is introduced here.

## Browser recovery

Both event consumers use one controller per active view:

1. Establish the socket, then fetch `/events` with credentials and the view's current filters. Initial
   subscription happens before the snapshot, reducing the connect/fetch gap.
2. Buffer at most 200 matching live records during that request. Merge the snapshot with buffered
   events in insertion order, accounting for repeated identical records rather than using a lifetime Set.
3. On disconnect, request failure, or buffer overflow, invalidate the old connection/request, display a
   persistent possible-gap notice, and retry with jittered backoff from 500–750 ms up to 30 seconds.
   A connection must stay live for 30 seconds before rapid-failure backoff resets.
4. Bound handshake and snapshot waits to ten seconds each. Maintain at most one active connection,
   request, and retry/deadline timer; abort stale requests when reconnecting, changing filters, deleting
   records, or leaving the page. HTTP `401`/`403` snapshot errors and WebSocket policy close `1008` stop
   automatic retries. Browser handshake errors hide their HTTP status, so those retry with backoff.

The event center retains 200 rows. The breakpoint panel fetches the latest 200 kernel events, selects
the active debug session, and displays at most 50 hits; its previously unbounded seen-event Set is gone.
The possible-gap notice remains after a successful reconnect because refreshing cannot prove that all
events were recovered. Under sustained overload the client may remain in recovery until load subsides.

Snapshot reads request no-store and reject redirects while retaining cookie credentials. Malformed JSON,
invalid Event shapes and non-text live frames are discarded with a persistent possible-gap notice; they
do not trigger reconnection by themselves. Valid events excluded by the view's filters are not gaps.

Breakpoint state is keyed by Engine, debug session and normalized instrumented lines from the run report.
Switches hide the old view immediately, before subscription effects run. Both snapshot and live filtering
require kernel breakpoint events, the exact session, and a declared positive integer line; an empty set
does not subscribe. Matching events still use the existing 200-row recovery/50-hit display limits and
occurrence-count overlap handling. Model-owned decorations and callbacks are removed on replacement or
disposal, and out-of-model hit lines are not displayed. Clearing requested breakpoints prepares future
runs; it does not uninstall probes from an already running program.

The Engine trace parser requires a nonempty session and an entire positive decimal token. Zero, fractions,
numeric prefixes, overflow and blank line tokens are rejected. Normal trace suffix whitespace is accepted.
Session markers/line allowlists do not prove that a kernel producer is trusted in a shared trace log.
See [bug hunt 06](functional-network-bug-hunt-06.md) for scoped evidence and remaining limitations.

## History, export and unread controls

The Events page keys history and connection/gap state by Engine, navigation and filters. Invalid or
reversed custom ranges stop history/export/deletion instead of throwing or silently querying everything.
Changing filters hides previous rows immediately; relative windows age displayed rows on a one-second
tick without extra snapshots or acknowledgements. Refreshing the same scope preserves its gap notice.
Only a successful current snapshot can establish an empty view; recovering history disables deletion.

Export freezes the chosen query/format for one request, prevents duplicate dispatch and aborts browser
waiting on input changes or navigation. A 20-second deadline includes the body. Its MIME type must match
JSON/CSV; filenames are safe and format-bound, and object URLs are released after download activation.
Export remains a separate full-history query, not an atomic snapshot of the 200 displayed rows; file
contents, spreadsheet formula safety and total bytes are not newly validated or bounded by this patch.

Deletion retains its target-bound typed confirmation and fixed upper cutoff, never the visible count as
its server scope. Only exact `ok: true` plus a nonnegative safe-integer `deleted` count refreshes the view;
failure/timeout is uncertain, does not clear rows and never retries automatically. Leaving the page cancels
waiting, not deletion. Explicit session/permission rejection remains distinguishable.

After successful history/live delivery, a 1.2-second debounce acknowledges ALL current-account events,
including events outside the filters/view; the existing endpoint has no per-event IDs or cutoff. The UI
discloses that scope. Only one acknowledgement can be pending; filter changes/navigation cancel its timer
and waiting. Failure is visible and suppresses automatic attempts until a manual retry or new view scope.
Sidebar unread reads are validated, have ten-second deadlines and poll four seconds after completion.
Unavailable state is `?`, not zero. Successful acknowledgement/deletion triggers a fresh read that invalidates
older in-flight badge responses. These are UI acknowledgements, not new durable SQL guarantees.

See [bug hunt 07](functional-network-bug-hunt-07.md) for reproduction and source-bound evidence.

## Recovery limits

### Persistence ordering and retention

While SQL remains healthy, mutations drain earlier queued publications through a writer barrier before
mark-read, filtered deletion, replacement or retention changes. Per-owner admission prevents later local
publications overtaking that sequence, including after cancellation of an admitted SQL caller. Other
owners can still publish. Memory-only cancellation while waiting for locks leaves the old state intact.
Settings/trim and replacement each use a transaction; failures do not publish their new memory state.
Capacity caches are cleared after mutations so DropNew can immediately use newly freed slots. Equal-time
rows preserve queue order, and SQL history uses timestamp plus ID ordering.

The queue still has 2,048 slots and 64-record batches. Full/closed queues no longer spawn an additional
SQL task per event: they warn and latch volatile history until Engine restart. Barrier waiting and selected
schema/settings/mutation stages each have ten-second application deadlines. They do not stop already
dispatched database work or prove rollback, and they are not a total request deadline. The existing
fallback can still acknowledge volatile read/deletion changes; HTTP success is not a new durability flag.
After an outage, review storage health and any volatile events before restarting; restoration/reconciliation
is not automatic. Plain history/unread reads do not gain a barrier or an atomic snapshot/live join.

See [bug hunt 08](functional-network-bug-hunt-08.md). Its 17 new PostgreSQL cases plus the earlier deletion
case use disposable schemas, real SQL and controlled queues/locks; CI explicitly selects the new ignored
tests. Run only with an explicitly disposable `CYANREX_TEST_DATABASE_URL`, never the deployment URL:

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib services::event_bus::tests -- --ignored --test-threads=1
```

### Remaining replay limits

Only retained recent history can be recovered. Retention policy, event filters, other kernel traffic,
or Engine restarts may remove missing records before a client reconnects. PostgreSQL persistence is
asynchronous, so an HTTP snapshot may also trail the in-memory broadcast.

Without event IDs or cursors, snapshot/live overlap is reconciled by full-record content and occurrence
count. Identical events and database timestamp normalization can remain ambiguous; this is not an atomic
snapshot/live join or an exactly-once guarantee. Full exports keep their existing behavior. Work that
requires complete replay needs a separately versioned cursor/persistence contract, not larger client buffers.

## Verification

- `frontend/tests/breakpointLifecycle.browser.mjs` and `breakpointSafety.browser.mjs`: optional actual-hook
  and production-Monaco tests for session/line filtering, model cleanup, gap recovery and explicit cleanup.
- `engine/src/routes/ebpf/ringbuf.inc.rs`: pure trace-marker parsing regressions; no live sampler is started.
- `engine/tests/routes_tdd/events_ws.inc.rs`: real authenticated loopback handshakes, Origin checks,
  unchanged raw Event payloads, immunity to other-owner bursts, and explicit own-overload closure.
- `engine/src/services/event_bus/subscriptions/tests.rs`: shared-event/JSON allocation, owner isolation,
  cancellation, last-connection cleanup, stale-generation drops, concurrent publishing/churn, and
  legacy global/DropNew behavior.
- `frontend/tests/eventStream.test.mjs`: fake-clock transport tests for races, filtering, overlap,
  cancellation, deadlines, bounded buffering, retry growth, and terminal failures. Included in the
  frontend quality gate; they do not substitute for a browser/LAN end-to-end test.
- `frontend/tests/eventStream.browser.mjs`: optional production-page Chromium smoke with synthetic
  HTTP/WebSocket fixtures. Exercises the visible gap notice, snapshot/live merge, filter changes,
  and narrow viewport layout (`npm --prefix frontend run test:event-stream-browser`).
- `node scripts/bench-event-stream.mjs <new-output-directory>`: private release-mode loopback pressure
  harness, with 32-subscriber paced delivery, a deliberately tiny-channel burst, and a real non-reading
  TCP peer, using owner-scoped queues. Runs serially with source hashes and immutable output. It excludes authentication, EventBus
  history, database, browser rendering, and kernel/eBPF work, and does not measure maximum throughput.
- `node scripts/bench-event-fanout.mjs <new-output-directory> [baseline-mainline-binary]`: serial current
  global-vs-owner delivery comparisons, including shared lazy JSON and equal matching frame bytes.
  Deliberately oversized channels prevent lag from inflating throughput; they are not production memory
  sizing. An optional frozen pre-change mainline example adds a no-subscriber before/after control.
  It measures the service and frame construction, not network delivery. Historical results keep their
  original harness/source hashes; source provenance of a supplied baseline binary must be established separately.
