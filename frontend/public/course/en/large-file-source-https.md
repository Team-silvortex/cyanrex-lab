# Real Source authorization over HTTPS

Status: **Unix external test coverage included in source 0.5.9**, API **0.3.0**, **2026-10-13**.
This follow-up to [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) was outside
the 0.5.8 tag; its dated execution retains that scope. It combines the actual [Source delivery](large-file-delivery-client.md),
[paired router](large-file-distribution-router.md) and [controlled HTTPS client](large-file-delivery-https.md)
in one acceptance chain. It adds test coverage, not a production listener, trust policy or Session issuer.

## One actual authorization and byte path

An explicit disposable PostgreSQL fixture creates the original audited identity, Session, Published
file and independent recipient grants through the existing commands. Original Source-owned files
provide the bytes. One supplied Source and chunk workspace construct one distribution router;
the client still requires its original exact expected version and per-call private Session cookie.

Disposable OpenSSL terminates loopback HTTPS. Its decrypted original method, raw URI, repeated
headers and actual request body enter that same router. The transport fixture sends the actual
status, response headers and Body frames back through TLS; it does not synthesize a manifest or
chunk reply. Missing Content-Length may be supplied from the actual body's exact size hint, as
normal HTTP serving would do; real frames and EOF are still required. Connection-close framing
is transport only, not a new response or authorization contract.

The original binary Bytes owner stays alive through each pipe write or held response. It is not
flattened into a whole-body Vec before its Source/HTTP leases are released. Test observation and
OpenSSL/kernel buffering are outside those existing budgets; this is not a global memory quota.

## Three separate failure boundaries

- Complete controlled delivery checks actual large, short and empty Published files, exact GET
  inventory, private credentials, selected output bytes/hash/ownership/mode and unchanged Source
  records. Empty files still require a current authorized manifest GET.
- Revocation occurs after the actual first chunk ACK, but before the later chunk enters Source.
  A dedicated `BeforeDispatch` barrier is not a response-header hold: after Source has approved a
  byte snapshot, later revocation cannot reclaim it. The later private 404 preserves only the old
  acknowledged prefix, without final assembly or selected saving; revoked empty metadata is refused.
- Explicit cancellation holds an actual TLS metadata or later-chunk body after the real Source
  response. An eight-byte pipe prefix proves only that the pipe accepted those bytes, not that the
  client consumed them. Cancelled ends the client's waiting/continuation, not already-approved
  bytes, OS work or output. The old peer is stopped before a separate healthy peer/control flow.

These cases retain existing error projection: certificate/non-timeout transport failures are metadata
Unavailable, header deadline expiry is TimedOut, and private Source rejection is stage InvalidResponse.
These mappings do not claim that all timeout/certificate paths ran in the new three cases.
No richer remote error, retries, adoption, deletion, resume or continuous grant check is invented.

## Execution and cleanup

The three `source::source_https` cases belong to `file_delivery_tdd` and are explicitly ignored by
default. `bash scripts/test-file-delivery.sh` runs six exact Source cases: the original three and
these three combined Source/HTTPS cases. It requires both an explicit disposable
`CYANREX_TEST_DATABASE_URL` and installed OpenSSL. The existing CI entry runs the same inventory;
static `fileSourceHttps` guards check only wiring and evidence scope, not actual SQL/TLS behavior.

Complete positive controls await the actual responder's full-body EOF count before shutdown;
client Content-Length completion alone does not imply the responder has polled that final EOF.
The later-chunk hold probes Source byte capacity; independent paired HTTP/Source lease evidence
remains in the original router lifetime case rather than being relabelled as a new TLS probe.

The native pipe, gates, router/frame waits and certificate commands are bounded. Cleanup first
stops the owned responder and dispatch waits, releases gates, kills/reaps OpenSSL and joins its
thread, then drains original Source work before removing that fixture's SQL namespace and private
directories/key material. No live database, system CA, existing target or unrelated directory is used.

This remains disposable loopback integration. Normal-app/UI/SDK mounting, secure public Session
issuance, an unprivileged data-service launcher, deployed-LAN acceptance, recovery and physical
disk policy remain separate. Versions, dependencies, dated tensor scores, published release reports,
**106 historical status blocks** and **56 existing execution tails** remain unchanged. The execution
below is fresh evidence; the old separate SQL and mock-TLS runs are not reused as combined-chain proof.

## Recorded execution — 2026-10-13, Unreleased after 0.5.8

- New external cases compiled without warnings; no production behavior was changed and no
  production Red is claimed. After formatting, two of fifteen focused guards rejected whitespace
  alone; their comparisons were made formatting-insensitive and all fifteen passed.
- The first explicit native run passed all nine unique cases. Review then identified a possible
  fixture-only shutdown/count race, not an observed production failure. Successful controls now
  wait at most ten seconds for the real responder EOF counter, retaining exact count assertions.
- The final explicit run again passed all nine unique cases: three existing Source cases, three
  new actual Source/HTTPS cases and three existing controlled mock-TLS cases. The new cases are
  the overlap of SQL and TLS coverage, not three extra executions in each total. Default Cargo
  still executes no external database/TLS cases automatically.
- Real selected short, 4 MiB + 17-byte and empty outputs passed independent bytes/hash/EOF,
  ownership/mode and distinct-inode checks after checked fixture cleanup. Real Source record,
  original file and SQL object snapshots stayed unchanged except the exact deliberate grant delta.
- The first ACK followed by pre-Source revocation produced the expected later private 404 and
  original indexed InvalidResponse, preserving only the verified prefix. Explicit header/body
  cancellation closed the old view without Complete; stopping the held binary peer restored a
  real Source byte ACK, and separate healthy peers/roots saved verified outputs.
- Thirteen related static groups passed **65** actual checks. Full backend/common gate passed:
  **1,097 Rust tests**, **861 explicitly ignored**, **98 result groups**, **356 common checks**,
  zero failures and zero Rust warnings. Final source-specific guards also passed **5/5**.
- Hash guards confirmed all **484 engine/src files** unchanged, **510 protected files** unchanged,
  and the **106 historical blocks / 56 old tails / 8 historical protected files** unchanged.
  Source/API stay **0.5.8 / 0.3.0**; published 0.5.7 and 0.5.8 reports/tags were not rewritten.
- After all SQL/TLS cases ended, the owned UID-1000, mode-0700, socket-only PostgreSQL was stopped
  and its exact temporary data/socket directories removed (**57,740 KiB**). Logs remain in
  `/tmp/cyanrex-source-https-57DyWUCm` (`native-final.log`, `full-backend.log`, guard/build/stop logs).
  No project cache, existing output, live database, legacy data or system trust store was removed.

No fresh frontend/SDK build, browser/deployed-LAN/kernel acceptance, security audit or offline
release bundle is claimed. This slice adds tests, documentation and runner inventory only; it does
not establish normal-app mounting, privilege isolation, global memory quotas or restart recovery.
