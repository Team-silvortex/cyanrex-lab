# Event stream lag and slow-peer recovery — 2026-09-08

## Outcome

The WebSocket handler no longer silently resumes after broadcast lag. It closes with `1013`, bounds
normal sends to five seconds and close sends to one second, and releases timed-out subscribers.
Both browser consumers now reconnect, refresh bounded recent history, invalidate stale work, and
retain a possible-gap notice. Raw Event JSON is unchanged; durable/exactly-once replay is not promised.

A real handshake regression also found that the existing CSRF middleware treated WebSocket GET as
safe and accepted a disallowed Origin. The handshake now applies the existing Origin/Referer policy.
Native consumers may need to add an allowed Origin; see [the migration guide](../../../docs/en/event-stream.md).

## Reproduction and scope

```sh
node scripts/bench-event-stream.mjs <new-output-directory>
```

The runner refuses existing output directories. It builds a locked, offline release library test
executable and invokes only the private ignored transport pressure test. Three scenarios run three
times each, serially with rotating order. `metadata.json` binds source and executable hashes;
`runs.jsonl` and `summary.json` retain every result and GNU time process counters.

Environment: AMD Ryzen 7 7735H, Linux 7.0.0-30-generic, rustc 1.95.0, 16 Tokio workers, shared desktop.
These checks use the real handler, global Tokio broadcast and loopback TCP/WebSocket connections.
The test-only router bypasses authentication, and the fixture does not use EventBus histories,
PostgreSQL, browser rendering, Runner Agents or the kernel. Authentication has separate route tests.
The paced case checks delivery at its configured rate, not maximum throughput. This is not an A/B
performance comparison with the earlier service-only subscriber benchmark.

## Three-run results

| Scenario | Configuration | Result in all three runs |
|---|---|---|
| Paced fan-out | 8 users, 32 subscribers, channel 1024, 2,000 events, 8 events every 2 ms, 128-byte payload field | 8,000 / 8,000 matching copies delivered, zero resync closes; 0.5005–0.5020 s |
| Forced overload | 8 users, 32 subscribers, deliberately tiny channel 8, 60,000-event burst | All 32 clients received explicit `1013`; 0 / 1 / 0 data copies arrived before closure |
| Non-reading TCP peer | 1 subscriber, receive buffer requested at 4 KiB, channel 128, 64 × 256 KiB payloads | Handler ended after 5.0056–5.0064 s; zero subscribers remained |

The overload case deliberately loses nearly all data; its success criterion is explicit failure, not
delivery. The tiny capacity makes lag reproducible and does not represent the production capacity.
The stalled-peer burst is below channel capacity, so lag cannot satisfy that case: the peer stays
open without reading, fills the real TCP send path, and the send deadline releases the handler.

## Correctness and quality evidence

- TDD: the original handler timed out waiting for a lag notification; the original middleware accepted
  the prohibited Origin. Both real-loopback regressions pass after the change.
- Full local `quality-gate.sh --security --no-npm-install` passed: 174 Rust tests, OpenAPI/SDK contract
  and compatibility checks, 43 common JavaScript tests, frontend production build/type check and
  regressions, SDK runtime/type/package checks, Cargo audit, and frontend/SDK production npm audits.
  Four Rust tests remain normally ignored: external PostgreSQL, external Agent, and two manual
  benchmarks. The WebSocket manual benchmark was explicitly run here (nine successful cases).
- Thirteen new fake-clock client regressions cover in-flight and late snapshot/live overlap,
  multiplicity, bounded buffering (including a 10,000-frame burst), owner-view filters, stale request
  cancellation, backoff, deadlines, terminal errors and disposal. They run in local and CI frontend gates.
- One production-page Chromium smoke with synthetic HTTP/WebSocket fixtures passed: visible gap
  notice, reconnect history refresh, live row preservation, filter switching, one active connection,
  zero page errors, and no horizontal overflow at 390 px. The first smoke attempt had an incorrect
  selector (it included the sidebar language picker); the fixture selector was corrected. No production
  behavior was changed to satisfy it. Screenshot was visually checked locally.

## Limits and next work

Recovery is limited to the retained recent window (event page: 200 rows; breakpoint panel: latest
200 kernel events narrowed to the active session, at most 50 visible hits). The asynchronous database
snapshot may trail broadcast. Content/multiplicity reconciliation cannot replace unique event IDs
or an atomic cursor barrier; identical records or timestamp normalization can remain ambiguous.
Persistent overload can keep the view in recovery. There is no new idle heartbeat/session revocation.

Global fan-out is unchanged: unrelated users still wake every receiver and can trigger conservative
resync. A useful next performance stage is owner-scoped subscription distribution with bounded
lifecycle cleanup and comparable fan-out measurements. Durable replay is a separate protocol project.

## Measurement fingerprints

- Handler: `61890b3da073525046a1cb49339a53bf96c7d64132f1ea355e0c30f2fe62d0c9`
- Pressure harness: `be521a999888ceb1af4fe98341dee5ffe4c4319769747c4afa697c81044b4373`
- Runner: `ce902a167bf46a0646fe380c5600ed34bf7257ac6a7e671218fc84509c900f4b`
- Release test executable: `971092faa84890a0b6e31c0fe53b446335efc4286412d9b4f4f053d62332308c`

Results were collected on dirty main based on `0fc3f9b142eebcc5284876d1c8a3c3ecb0018dd8`, not a released
artifact. No version, commit, tag, or push was created by this stage.
