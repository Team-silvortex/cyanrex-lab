# Owner queue prototype — superseded intermediate, 2026-09-08

This is the first, **not final**, owner-scoped implementation. It shared `Arc<Event>` payloads but
still serialized JSON independently for every matching subscriber. All raw measurements and metadata
are retained unchanged. Use the [final report](../2026-09-08-owner-fanout-final/REPORT.md) for the
implemented path, longer confirmation runs, transport checks, and limitations.

The prototype ran 24,000 events, 32 writers, 32 subscribers, 500 retained records per owner, and
channels of capacity 32,768. Each of four layouts compared the current global and owner paths three
times, with no permitted lag and equal matching output bytes. Six additional runs compared the
unchanged no-subscriber mainline example before/after (two million events per run).

| Layout | Global median events/s | Owner median events/s | Change |
|---|---:|---:|---:|
| 1 owner, 128-byte field | 135,158 | 125,524 | -7.1% |
| 1 owner, 4,096-byte field | 68,092 | 59,297 | -12.9% |
| 8 owners, 128-byte field | 157,984 | 260,333 | +64.8% |
| 8 owners, 4,096-byte field | 121,105 | 143,530 | +18.5% |

Owner routing solved unrelated-traffic eviction and reduced CPU work, but this prototype was not
accepted as the final performance result because one-owner throughput regressed. The implementation
then added one lazy JSON cache per shared event, keeping separate frame payloads per connection.

The final harness also uses ten times as many events, larger no-lag buffers, and explicit `Utf8Bytes`
construction on both paths. Do **not** subtract this table from the final table to claim a cache-only
speedup: those harness configurations differ. The final within-run global/owner comparisons use
identical inputs and matching output bytes.

Source base: `5fa774110a50e8835422402014182827c21d3044` (`0.3.4`) plus uncommitted changes.
The machine was a shared Ryzen 7 7735H desktop, Linux 7.0.0-30-generic, rustc 1.95.0, 16 Tokio workers.
This service-only experiment did not use PostgreSQL, sockets, a browser, an Agent, or kernel eBPF.
`metadata.json` contains the original source/runner/executable fingerprints; `runs.jsonl` and
`summary.json` retain all 30 runs. The current runner no longer executes this prototype configuration.
