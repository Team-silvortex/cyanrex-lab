# Owner-scoped event distribution — 2026-09-08

## Outcome

Event WebSockets now use bounded queues keyed by the authenticated session owner. Unrelated users
cannot overwrite those queues; a deliberately tiny-channel real-handshake regression demonstrated the
old failure (`1013` after another user's burst) and now receives the correct raw Event frame instead.
An owner's own overload still closes explicitly. Authentication, Origin checks, history retention,
DropNew behavior, HTTP schemas and recovery close codes remain unchanged.

Connections for one owner share an immutable event and a lazily computed JSON string. No encoding is
done until requested, and each connection still creates/sends its own frame payload. The last
subscription removes the owner entry; weak guards and generation checks cover shutdown, cancellation
and simultaneous reconnects without keeping senders alive or deleting a replacement channel.
The global `EventBus::subscribe()` API remains available, without an unused global payload clone.

## Reproduction and scope

```sh
node scripts/bench-event-fanout.mjs <new-output-directory> [frozen-baseline-mainline-binary]
node scripts/bench-event-stream.mjs <another-new-output-directory>
```

Both runners refuse existing directories. Runs are serial with rotating case order, database variables
removed, locked offline release builds, source/executable fingerprints, and GNU time CPU/RSS counters.
No heavy build or quality gate ran concurrently with the measurements. This was a shared desktop,
not a pinned-core or isolated host: Ryzen 7 7735H, Linux 7.0.0-30-generic, rustc 1.95.0, 16 Tokio workers.

The fan-out comparison uses **two runtime paths in the same current release test executable**, not
historical-vs-current application binaries. Each case has 240,000 events, 32 writers, 32 subscribers,
500 prefilled history records per owner, and a 128- or 4,096-byte string field inside the event payload.
The global path deep-clones broadcast events and encodes matching frames independently; the owner path
uses the shared event/JSON implementation used by the real handler. Both construct every matching
`Utf8Bytes` payload. Elapsed throughput includes event construction, publication and consumer drain;
publish latency excludes input construction. GNU time includes process setup and teardown as well.

The channel capacity is deliberately **262,144**, exceeding the whole run so no lag can mask missing
work. This is not production capacity or a production memory estimate. Production remains 1,024 slots
per active owner. This service/frame-construction benchmark excludes sockets, HTTP/auth, PostgreSQL,
browser rendering, Runner Agents and the kernel; the separate loopback checks are described below.

## Three-run medians

| Layout | Global events/s | Owner events/s | Throughput change | Publish p95, global → owner |
|---|---:|---:|---:|---:|
| 1 owner, 128-byte field | 115,562 | 143,167 | +23.9% | 383.2 → 266.9 µs |
| 1 owner, 4,096-byte field | 70,627 | 115,553 | +63.6% | 609.6 → 500.3 µs |
| 8 owners, 128-byte field | 156,514 | 244,926 | +56.5% | 264.7 → 157.1 µs |
| 8 owners, 4,096-byte field | 115,792 | 162,446 | +40.3% | 338.7 → 244.9 µs |

All 24 fan-out runs had zero lag and exact matching-delivery counts. With eight owners, total receiver
copies fell from 7,680,000 to 960,000 while matching copies remained 960,000. With one owner, both
delivered 7,680,000 matching copies. Matching frame byte totals were identical between paths and rounds
for each layout; throughput was not increased by dropping events. These are generated frame bytes,
not bytes transmitted over a network.

| Layout | Process CPU seconds, global → owner | Peak RSS MiB, global → owner |
|---|---:|---:|
| 1 owner, 128-byte field | 17.87 → 8.26 | 52.1 → 58.6 |
| 1 owner, 4,096-byte field | 44.47 → 16.20 | 419.2 → 435.9 |
| 8 owners, 128-byte field | 12.91 → 2.77 | 56.9 → 120.2 |
| 8 owners, 4,096-byte field | 21.25 → 6.12 | 91.6 → 164.7 |

CPU is the median of each run's user + system seconds. RSS is a whole-process peak, not live queue
size; the one-owner 4-KiB owner runs varied from 303.2 to 491.5 MiB. The oversized per-owner channels
and cached representation have real memory costs. This change is not a general memory reduction.

The [first prototype](../2026-09-08-owner-fanout/REPORT.md), without cached JSON, showed a 7–13%
one-owner regression despite better multi-owner results. It was superseded. The final table compares
equal current-path workloads; the changed prototype/final harness sizes do not support a cache-only
before/after claim. Full min/max values and every raw run remain in the adjacent JSON files.

## Unchanged no-subscriber control

An additional six runs used the unchanged public `mainline_bench.rs` example with
`events 2000000 8 32 500 128 0 0`. The pre-change binary was built and copied from clean commit
`5fa774110a50e8835422402014182827c21d3044` before editing. Its SHA-256 is
`c095a71106f03db496d1a0ce104375350ec6cb43644b12b2f9d2bbbbd57658e1`; the example source hash is unchanged
at `5c7d2028737123d6f2aafc1a951e1c42d66e82c3a015ae74901ba738e6a63111`.

Median publication throughput was 640,524 → 707,585 events/s (+10.5%), publish p95 69.0 → 60.4 µs.
This checks the cost of the new empty-registry lookup and skipped unused global clone. Shared-host
variation remains visible (before: 590,236–660,657; after: 688,363–709,558 events/s). Unlike the fan-out
table, this control really compares historical and current binaries. It is not a WebSocket measurement.

## Real transport and correctness

The [owner-queue loopback report](../2026-09-08-owner-event-stream/REPORT.md) records nine independent
checks using the real handler: all paced runs delivered 8,000/8,000 copies without closes; tiny-channel
overload closed all 32 clients with `1013`; a non-reading TCP peer released both its receiver and owner
queue after 5.0056–5.0058 seconds. The paced cases are delivery checks, not maximum-throughput claims.

Focused regressions cover owner filtering (including a spoofed username query), own-vs-unrelated lag,
shared event allocation, lazy concurrent encoding including Unicode/escaping, live-only subscriptions,
last-connection cleanup, stale generation drops, closed registries, cancelled receives, task abortion,
concurrent writers/connection churn, legacy global delivery and DropNew semantics.

The full `./scripts/quality-gate.sh --security --no-npm-install` passed after measurement: 184 Rust
tests, formatting, version/course/OpenAPI/SDK compatibility, 43 common JavaScript tooling regressions,
Runner/distribution tools, the 17-route frontend production build and type check, 29 frontend regressions,
11 SDK runtime and 3 package tests plus types. Cargo and production frontend/SDK audits reported no
vulnerabilities. Five Rust tests remain normally ignored (two external integrations, three manual
benchmarks); fan-out and transport benchmarks were explicitly invoked here. No browser or privileged
kernel acceptance was rerun in this increment. No dependencies or contract schemas changed.

## Limits and next work

Queue capacity is per active owner, not a global memory/connection budget. Publishing to an unsubscribed
owner does not create a channel, but aggregate memory still scales with active owners, retained events,
payload size and cached JSON. The registry may retain its hash-table allocation after entries are removed.
History/settings/unread locks, CPU and persistence remain shared. No independent-kernel isolation,
connection admission limit, idle heartbeat, active-session revocation or durable cursor is added.
New subscriptions are live-only; HTTP history recovery remains best-effort and asynchronous persistence
can trail the stream. See [Event Stream Recovery](../../../docs/en/event-stream.md).

Further performance work should separately measure remaining shared-lock and persistence-queue
contention, or the previously measured LearningStore file-copy/rewrite cost. Durable replay and global
admission policy require explicit contracts rather than increasing buffers or claiming tenant isolation.

## Fingerprints

- Owner implementation: `bd6117829bfc70c9d38b26e4f370b720cb119b4b2fe7478a2976672063bff77f`
- Handler: `9aae8b53ce4d1ed51f7d989b36016e802f91eaf48a9ab3a16971d4c6e99ad249`
- Fan-out harness: `9557b8fa968417ea2d5703373736248346c6070f4240e7eb28c3414b8ce1eae6`
- Fan-out runner: `6e75b8bf2cb2d2948a9b8ba4ce3f5cb0a801b2b7ce1e44fa75e326cbdac88328`
- Release test executable: `ed3349661131b95913a9742012bc12605435b31073fa59303a3c05ce04dd767e`
- Current public mainline example binary: `0a95e997567120ae89af89fca232b198171fb53b016fedaa08e398096e286bd1`

Results were collected on dirty main based on `5fa774110a50e8835422402014182827c21d3044`, version
`0.3.4` plus this Unreleased increment. No version bump, commit, tag or push was performed by this stage;
these results are not release-artifact or classroom/kernel acceptance evidence.
