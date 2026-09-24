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
older in-flight badge responses. Browser confirmation alone is not proof of storage success; see the
Engine confirmation policy below.

See [bug hunt 07](functional-network-bug-hunt-07.md) for reproduction and source-bound evidence.

### Engine read failures and query validation

History, JSON/CSV export and unread HTTP handlers now use fallible reads. A configured store's schema,
query or row-decoding failure returns generic `503`, not a successful empty/partial history, zero count or
download. Asynchronous read waiting (including schema initialization and pool acquisition) is capped at
ten seconds without disabling future persistence. After a transient read fault is repaired, a new read
can succeed; cancellation also leaves storage usable. History SELECT statements are not cached across
reads so a repaired column type does not leave a stale result-type plan. This affects history SQL planning,
not writer caching, and no performance improvement is claimed.

Previously latched write/settings/legacy-service fallback still blocks these HTTP reads until storage is
healthy and the Engine is restarted; invalid configured database URLs are unavailable too. Existing volatile
events are not automatically merged on restart. Explicit memory-only instances keep their bounded volatile
reads. Legacy infallible EventBus helpers remain best-effort for compatibility; HTTP handlers do not use them.

History/export reject invalid or empty category/severity/date filters, unknown/duplicate keys, negative or
overflowing windows, reversed effective ranges, and unsupported export formats with generic `400`.
`since_minutes=0`, history's default/clamped limit, case-insensitive filter values and JSON/CSV payloads stay
compatible. Legacy `username` is ignored on both routes; history's `format` and export's `limit` are also
ignored. No query field selects another owner. Successful reads/downloads and handler query/storage errors
are `Cache-Control: no-store`. Failed exports have no attachment header. Auth middleware still controls access.

The deadline bounds cooperative async waiting, not authentication, response serialization, arbitrary CPU
work, or confirmed SQL cancellation. Reads still have no persistence barrier or atomic snapshot/live join.
See [bug hunt 10](functional-network-bug-hunt-10.md).

### Engine mutation confirmation

HTTP mark-read and deletion must confirm the selected storage mutation before returning success.
Configured invalid/unavailable storage, failed write barriers, prior fallback and unconfirmed writes
return generic no-store `503`, never a successful memory-only acknowledgement. Definite schema/query
errors do not clear unread/history or disable persistence, so repaired storage can be retried. Existing
barrier/storage deadlines may still latch fallback: inspect storage and volatile events before restarting.
No automatic volatile reconciliation is added. Explicit memory-only instances retain volatile behavior;
legacy infallible service helpers still have their compatibility fallback contract.

The caller's cooperative service wait is capped at ten seconds including owner admission. Cancellation
before admission dispatches nothing. After SQL admission, a background task retains per-owner ordering
through cache publication even when the caller cancels or gets a timeout. Consequently `503`, a timeout
or a lost response is **unconfirmed, not rollback**: inspect current state before an explicit retry, and
do not retry automatically. A SQL statement or local publication can complete after the response.
This is not a whole-worker lifetime bound, an authentication/serialization deadline, process-shutdown
draining or proof of server-side SQL cancellation. It adds no external-writer/multi-Engine coordination.

Successful JSON shapes, session ownership, CSRF, all-owner mark-read and no-filter delete-all remain.
Deletion freezes its validated relative cutoff before waiting. Invalid/duplicate/unknown query fields
and invalid ranges now return no-store JSON `400`; successful acknowledgements are also no-store.
No-match deletion and already-read acknowledgement remain valid successes. SQL deletion counts include
uncached rows and preserve surviving read flags; all memory deletion/cache locks are acquired before
editing, so cancelled memory-only waits cannot split state. See [bug hunt 11](functional-network-bug-hunt-11.md).

### Event retention settings

`GET /settings/events` reads configured storage, not a potentially stale runtime cache. A missing row
means the normal 500/DropOldest default; invalid stored limits/policies, decoding or query errors, prior
fallback and ten-second service read waits return generic no-store `503`. Read failure remains retryable
without disabling persistence or caching a default. This read does not synchronize external writers
into runtime policy; legacy publication still uses its own cached, best-effort helper.

`POST /settings/events` confirms the write barrier and a single policy/trim transaction before publishing
local settings/history/unread/capacity changes. Definite SQL failure preserves memory and is retryable;
failed barriers/prior fallback cannot silently save or trim memory. It shares the above ten-second caller
wait and admitted-worker cancellation semantics: unconfirmed is not rollback; verify before retrying.
Success and handler errors are no-store. JSON syntax, oversized bodies, content type and field/type
errors retain 400/413/415/422 with generic JSON. Limits still clamp to 50..50000; unknown body fields do not
override session ownership. Explicit memory-only settings remain volatile. See [bug hunt 12](functional-network-bug-hunt-12.md).

The teacher settings page now requires a successful validated read before saving and never treats a
cached draft as confirmation. Each reviewed save confirms events before sending the optional compiler
write; malformed/mismatched acknowledgements cannot trigger the next stage. Browser waits are bounded
to 10 seconds per read and 20 per write, including JSON bodies. A partial/unconfirmed result requires
explicit reload before a fresh confirmation; no automatic retries or rollback claims. Confirmed retention
changes invalidate unread counts. Navigation discards obsolete results, while metrics refresh preserves
the settings warning. These UI checks use isolated browser fixtures, not a live Engine connection; see
[bug hunt 13](functional-network-bug-hunt-13.md).

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
dispatched database work or prove rollback, and they are not a total worker deadline. Legacy service
helpers can still acknowledge volatile changes; HTTP mark-read/deletion/settings use the separate policies above.
After an outage, review storage health and any volatile events before restarting; restoration/reconciliation
is not automatic. Plain history/unread reads do not gain a barrier or an atomic snapshot/live join.

Cold DropNew publication now checks retained SQL rows before changing history, unread or live queues.
Locally admitted pending events consume slots even across the writer's one-second count-cache expiry.
Per-owner admission serializes initialization and reservation; no slot is consumed by cancellation before
publication. Deletion, replacement and settings changes rebase this count after their existing barrier.
Only cold or invalidated DropNew owners need this additional count query, bounded to ten seconds of
application waiting. Failure latches the existing volatile fallback; recovery still requires reviewing
volatile events and restarting, not automatic reconciliation. DropOldest and memory-only admission stay
unchanged. Counts are single-Engine metadata, not coordination with external SQL writers.

The background writer no longer owns a producer handle. Once the last producer clone leaves, healthy
queued batches (including the last partial batch) drain and the worker exits while the async runtime is
alive. Disabled storage consumes/discards queued events and rejects barriers rather than claiming durable
success. This does not add a process-shutdown hook, whole-writer deadline, lost-commit recovery or durable
delivery: runtime termination, stalled SQL or ambiguous write retries remain outside this guarantee.

See [bug hunt 08](functional-network-bug-hunt-08.md), [09](functional-network-bug-hunt-09.md),
[10](functional-network-bug-hunt-10.md), [11](functional-network-bug-hunt-11.md) and [12](functional-network-bug-hunt-12.md).
The 52 PostgreSQL cases use disposable schemas, real SQL and controlled queues/locks; CI explicitly selects
these ignored tests. Cold-cache recreation uses a fresh EventBus against the same schema, not process
termination. Run only with an explicitly disposable `CYANREX_TEST_DATABASE_URL`, never the deployment URL:

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
