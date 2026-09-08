# Owner-scoped WebSocket pressure checks — 2026-09-08

The existing loopback pressure suite now uses the production owner-queue implementation and real
WebSocket handler, including shared lazy JSON, bounded sends and subscription cleanup. It does not use
EventBus history, database, authentication, a browser, an Agent or the kernel. Production session/Origin
checks and owner selection are covered by separate authenticated route regressions.

```sh
node scripts/bench-event-stream.mjs <new-output-directory>
```

Three scenarios ran three times each, serially with rotating order, in a locked offline release test
executable on a Ryzen 7 7735H shared desktop, Linux 7.0.0-30-generic, rustc 1.95.0, 16 Tokio workers.
No heavy build or quality gate ran concurrently. The runner refuses existing result directories.

| Scenario | Configuration | Result across all three runs |
|---|---|---|
| Paced | 8 owners, 32 connections, 1,024 slots/owner, 2,000 events, 8 every 2 ms, 128-byte field | 8,000/8,000 matching copies, zero closes, 0.5009–0.5014 s |
| Forced own-overload | 8 owners, 32 connections, 8 slots/owner, 60,000-event burst | All 32 clients explicitly closed with `1013`; 16 / 56 / 25 data copies before closure |
| Non-reading TCP peer | 1 owner/connection, 128 slots, 64 × 256 KiB payloads, requested 4 KiB TCP receive buffer | Handler released after 5.0056–5.0058 s, zero receivers and zero owner queues remaining |

The deliberately overloaded case tests explicit failure, not complete delivery. The stalled burst is
smaller than its queue, so lag cannot substitute for the TCP send deadline. The paced case is an
exact-delivery check at the configured rate, not a throughput limit. This is not LAN, browser-to-Engine,
or durable replay acceptance. Owner queues eliminate cross-owner eviction but do not isolate shared CPU,
memory, history locks or persistence resources.

The older `2026-09-08-event-stream` report remains unchanged: it used global broadcast and cannot be
reinterpreted as owner-queue evidence. The [new fan-out report](../2026-09-08-owner-fanout-final/REPORT.md)
documents current runtime-path throughput comparisons, memory costs, and the superseded prototype.

## Fingerprints

- Source base: `5fa774110a50e8835422402014182827c21d3044` (`0.3.4`) plus uncommitted changes.
- Handler: `9aae8b53ce4d1ed51f7d989b36016e802f91eaf48a9ab3a16971d4c6e99ad249`
- Pressure harness: `609de22a2ec419be3da89b73097e28dd06f9d753de09efa06c3ab1536a0cd6e7`
- Runner: `7838a78c9c7b23921ca420c43d08f86222252c47ceb41c0d396ce46da57f2789`
- Test executable: `ed3349661131b95913a9742012bc12605435b31073fa59303a3c05ce04dd767e`

`metadata.json`, `runs.jsonl` and `summary.json` retain source fingerprints and all nine raw results.
No version, commit, tag, push or release artifact was created by these checks.
