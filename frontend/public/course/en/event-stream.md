# Event Stream Recovery

The event center and eBPF breakpoint panel are best-effort recent-history views. They do not provide
durable subscriptions or an exactly-once audit log.

## Wire compatibility and access

- `GET /ws/events` still upgrades to a WebSocket and sends one raw `EventRecord` JSON object per text
  frame. There is no new JSON envelope, control event, cursor, sequence field, or subprotocol.
- The Engine filters events by the authenticated session owner. Its broadcast channel is still global:
  another user's traffic can cause lag, but neither that user's events nor their skipped-event counts
  are exposed to the client.
- The handshake now applies the existing state-change Origin/Referer policy in addition to session
  authentication. Browser clients already send Origin. Native/SDK consumers must supply an allowed
  Origin, configured through `CYANREX_CORS_ORIGINS` (or the normal frontend defaults). Missing origins
  return `403` unless the operator explicitly enables `CYANREX_ALLOW_MISSING_ORIGIN`; disallowed origins
  remain rejected. This security correction may require updates to older native clients.
- The JavaScript SDK still returns a WebSocket URL; it does not manage reconnection or replay for callers.

## Server behavior

The subscription is established before completing the upgrade. If the bounded broadcast receiver
reports lag, the Engine closes the socket with code `1013` and reason
`event stream lagged; reload /events` instead of silently continuing. This indicates a possible gap,
not an exact per-user number of missing events.

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

## Recovery limits

Only retained recent history can be recovered. Retention policy, event filters, other kernel traffic,
or Engine restarts may remove missing records before a client reconnects. PostgreSQL persistence is
asynchronous, so an HTTP snapshot may also trail the in-memory broadcast.

Without event IDs or cursors, snapshot/live overlap is reconciled by full-record content and occurrence
count. Identical events and database timestamp normalization can remain ambiguous; this is not an atomic
snapshot/live join or an exactly-once guarantee. Full exports keep their existing behavior. Work that
requires complete replay needs a separately versioned cursor/persistence contract, not larger client buffers.

## Verification

- `engine/tests/routes_tdd/events_ws.inc.rs`: real authenticated loopback handshakes, Origin checks,
  unchanged raw Event payloads, owner isolation, and explicit overload closure.
- `frontend/tests/eventStream.test.mjs`: fake-clock transport tests for races, filtering, overlap,
  cancellation, deadlines, bounded buffering, retry growth, and terminal failures. Included in the
  frontend quality gate; they do not substitute for a browser/LAN end-to-end test.
- `frontend/tests/eventStream.browser.mjs`: optional production-page Chromium smoke with synthetic
  HTTP/WebSocket fixtures. Exercises the visible gap notice, snapshot/live merge, filter changes,
  and narrow viewport layout (`npm --prefix frontend run test:event-stream-browser`).
- `node scripts/bench-event-stream.mjs <new-output-directory>`: private release-mode loopback pressure
  harness, with 32-subscriber paced delivery, a deliberately tiny-channel burst, and a real non-reading
  TCP peer. Runs serially with source hashes and immutable output. It excludes authentication, EventBus
  history, database, browser rendering, and kernel/eBPF work, and does not measure maximum throughput.
