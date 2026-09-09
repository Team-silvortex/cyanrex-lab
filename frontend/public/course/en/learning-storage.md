# Learning Record Storage and Query Costs

Learning attempts retain submitted source, automated feedback, completion evidence and the current
teacher comment. These increments change local-memory ownership, query work and file writing, not learning acceptance,
authorization, HTTP/SDK schemas, PostgreSQL queries, or the stored JSON format.

## Local snapshots

The local path is `CYANREX_DATA_DIR/learning/<instance>/attempts.json`. On first access the store loads
the plain JSON array into a shared snapshot containing reference-counted records. Taking a snapshot
copies one shared pointer, not every student's source. Queries release the memory lock before scanning
or aggregating, and use one consistent read-only snapshot.

Appending a run builds a new pointer index and one new record. Saving feedback copies the index and
only the edited record. Existing records and old readers' snapshots remain unchanged. Stale feedback
revisions are rejected before constructing replacements. This is internal copy-on-write, not an HTTP
response cache: subsequent requests see the latest published snapshot.

Local writers share one persistence mutex. A blocking worker streams the complete new array as the same
pretty JSON through a 64 KiB buffer into `attempts.json.tmp`, explicitly flushes, closes the file, renames
it over `attempts.json`, then publishes memory. Encoding and file I/O no longer occupy an async executor
thread or allocate a full encoded-file buffer. Reported encoding, write, flush or rename failures do not
publish unsaved changes. Older arrays without `teacher_feedback` still load;
no reference wrapper or migration is added to the file. API history responses remain independently owned.

## Cold initialization

The first local access acquires the same persistence lock before dispatching a blocking worker. File
reading, whole-file UTF-8 validation and JSON decoding run there, not on the async executor. A private
Serde sequence visitor wraps each decoded record directly in a shared pointer, avoiding an intermediate
full `Vec<LabAttempt>`. Input is still a complete in-memory string, not streamed or size-bounded. Unknown
fields, legacy records, record order and rejection of trailing/invalid input retain the previous behavior;
even invalid UTF-8 inside an ignored field rejects the file.

One initialization or commit per shared store may be queued/running. Concurrent first readers reuse a
successful published snapshot. Cancellation while waiting for admission dispatches nothing; after
dispatch, the worker retains admission and publishes the complete snapshot even without its caller.
Readiness is set only after publication. Warm access checks readiness without dispatching another worker.
This avoids relying on [cancellable OnceCell initialization](https://docs.rs/tokio/1.53.1/tokio/sync/struct.OnceCell.html#method.get_or_try_init)
to own a detached blocking task.

Only `NotFound` initializes an empty snapshot, without creating a file. Other I/O failures (including a
parent path that is not a directory), invalid UTF-8 and malformed JSON leave readiness unset and memory
unchanged. Later calls retry; failures are not cached or coalesced across waiting callers. A generic
warning records load failure without logging stored values. Public reads retain their existing empty/default
result on load errors; writes return storage errors. There is no new `.tmp` recovery or external-file watcher.

## Request cancellation and write completion

The caller acquires the shared writer lock before constructing and handing off a replacement snapshot.
At most one commit per store (including its clones) is queued/running in the blocking pool. Cancellation
while waiting for admission, or before handoff, does not start a disk write. Once handed off, the worker
owns the lock through rename and memory publication even if the request is cancelled. The next writer
cannot read an old in-memory snapshot and overwrite a just-renamed file. A queued worker also retains
admission after its caller disappears; storage errors are logged even without a waiting caller.

This is completion after request cancellation, not rollback: a timeout or disconnected browser can still
have saved its update. Reload history/current feedback before retrying; stale feedback revisions still
conflict, and append requests have no new idempotency key. Normal successful responses await publication.
This behavior relies on [Tokio's blocking-task lifecycle](https://docs.rs/tokio/1.53.1/tokio/task/fn.spawn_blocking.html)
and [detached join handles](https://docs.rs/tokio/1.53.1/tokio/task/struct.JoinHandle.html).

Explicit [buffer flushing](https://doc.rust-lang.org/std/io/struct.BufWriter.html) checks errors before
rename; it is not file/directory fsync. Failed writes may leave a partial `.tmp` file. Loading ignores it;
the next write truncates/replaces it. There is no recovery from that temporary file.

## Query behavior

- **Recent review:** scan for the selected owner, retain at most the requested number of borrowed rows,
  sort that small selection, then copy response payloads. The public limit remains 1–50 (the teacher UI
  requests 20). Descending timestamp order and original order for equal-time records are preserved,
  without assuming monotonic timestamps or file order.
- **Full history:** return every matching record in the existing newest-first order, still copying all
  returned source. The recent-page optimization does not impose a new full-export limit.
- **Progress / teacher overview:** aggregate counts, latest automated feedback and earliest completion
  in one pass, without cloning source or sorting every lab's attempts. Unknown/retired lab IDs still
  count toward student totals/activity; the five current catalog entries determine progress. Equal-time
  latest-record ties retain the previous first-input behavior.
- **Access:** teacher/admin guards, student owner checks and CSRF remain unchanged. Comments never
  change automated completion. Aggregates omit submitted source; authorized history/review still returns it.

For `n` total local records, `m` owner matches and page size `k`, recent selection scans `n` records and
uses up to `O(m log k)` heap work with `O(k)` selection storage. No owner index is added. Aggregation
still scans `n` records with a fixed-size lab catalog and small per-student accumulators. Results are
computed afresh rather than kept in a separately invalidated cache.

## Remaining boundaries

- Loading still holds a complete input string alongside decoded records, with no file-size cap.
  Records remain in memory without a new retention limit. Shared pointers add per-record bookkeeping,
  and in-flight readers can keep older snapshots alive. Lower write peaks are not a general memory or
  classroom-capacity guarantee.
- Writes still copy an `O(n)` pointer index and serialize/write the complete JSON file. The fixed encode
  buffer avoids a full-file allocation, but does not make persistence append-only or bounded-time.
- The existing path does not fsync files/directories, provide crash recovery or cross-process locking,
  or atomically publish to disk and memory across process failure. Request-cancellation completion does
  not cover process kill, panic, runtime teardown or power loss. Slow/stuck I/O retains one store's writer
  lock and blocking thread and can delay runtime shutdown; there is no new load/write deadline or global queue
  limit. Multiple Engine processes or independently constructed stores must not write the same file.
  A loaded store does not watch external edits; keep backups and do not edit a live store's file.
- PostgreSQL queries/schema and fallback policy are unchanged. Successful DB queries still fetch the
  original fields, including source; no SQL-side aggregation or DB performance improvement is claimed.
  Failed PostgreSQL feedback writes still return errors instead of updating a stale local fallback.
- HTTP, browser rendering, real PostgreSQL, kernel execution and power-loss behavior need separate
  acceptance. The focused [2026-09-09 PostgreSQL acceptance](acceptance.md) covers migration, feedback
  concurrency and owner-bound resume reads, not database performance or crash recovery. This is not a
  new independently isolated runtime.

## Reproducing the local benchmark

Before editing, build the existing `mainline_bench` release example, copy it to a new temporary directory,
and record its SHA-256 and source provenance. After the change, run:

```sh
node scripts/bench-learning-store.mjs <new-output-directory> <frozen-baseline-mainline-binary>
```

The runner refuses existing output and compares the unchanged public harness on both binaries. Thirty
synthetic students span 1,000, 10,000 and 50,000 records, plus a larger-source 50,000-record case. Three
rounds give 72 invocations. Cases rotate and each case's before/after order swaps each round, with
ordering regressions in the common quality gate. The current build uses locked offline dependencies.

Latency starts after loading: recent queries have 100 samples, overview 30 and writes 10 per invocation.
Write nearest-rank p95 is therefore the maximum of ten, not a reliable service-level tail estimate.
Process CPU/RSS includes fixture creation, loading and cleanup. No fsync, cache eviction or CPU pinning
is added; filesystem writeback and shared-host load can dominate write tails. Retain individual runs
and distinguish warm query gains from disk behavior.

Regressions cover snapshot sharing, old-reader immutability, legacy JSON, failed writes/renames,
concurrent feedback/appends, owner/order/limit behavior, differential aggregation against the old
algorithm, cancellation before admission/while queued/after rename, stale retries, streaming before
all rows are encoded, short/interrupted writes and explicit flush errors. The external PostgreSQL
regression remains opt-in.

For cold-process initialization, first build and freeze `learning_load_bench` against the baseline
library using the exact same example source as the candidate. Record both library and harness provenance,
copy the baseline binary outside the build output, then run:

```sh
node scripts/bench-learning-load.mjs <new-output-directory> <frozen-baseline-learning_load_bench>
```

Five rotated rounds over four fixtures give 40 measured processes, with alternating before/after order.
The runner creates synthetic files outside the measured processes, verifies identical bytes after all
reads, and removes only its own temporary fixtures. Freshly written files remain OS-page-cache warm:
this measures a cold store/process, **not cold disk**. Each invocation measures first recent-20 latency,
one warm recent-20 query, and maximum lateness of a 2 ms timer on a single-thread async runtime. Summaries
are medians/ranges of five observations, not request percentiles. CPU/RSS includes loading, queries,
overview validation and teardown, but not fixture generation. Timer results expose executor stalls, not
production HTTP health latency or a scheduling guarantee. Keep regressing prototypes separately.

Cold-load regressions cover cancelled queued/decoded initialization, concurrent first readers and writers,
missing versus invalid paths, whole-file UTF-8, legacy/unknown fields, invalid/trailing JSON without partial
publication, successful-load caching and retry after errors.
