# HTTPS verification for controlled file delivery

Included in source release **0.5.8** (**2026-10-13**). Dated 0.5.7/Unreleased implementation and
verification retain their original scope; inclusion does not mount or validate a deployment.

Status: **Unreleased Unix external verification**, source **0.5.7**, API **0.3.0**,
**2026-10-13**. This [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) follow-up
tests the existing [controlled delivery](large-file-delivery-cancellation.md) over actual OpenSSL
loopback TLS, rather than inferring whole delivery from the older single-chunk TLS case.
No production API, trust policy, permission, dependency, listener or application wiring is added.

## Complete transfer and exact peer identity

The explicit `file_delivery_tls_tdd` target uses disposable certificates and an actual HTTPS peer.
The positive case covers exact manifest GET, sequential chunks, original private assembly and explicit
host-selected saving for 4 MiB + 17 bytes, 17 bytes and empty files. It uses capacity one and an existing
owned directory target, with independent saved-byte/EOF/digest/inode/UID/mode/link checks and original
parent policy continuity. Empty delivery still requires metadata GET, not a chunk request.

A test CA is trusted only through `FileChunkClientConfig::trusted_root_pem`; HTTPS remains the
default transport. Unknown CA and wrong IP SAN must fail before decrypted HTTP reaches the peer,
so neither selectors nor the dedicated Session cookie are disclosed. Trusting a CA does not disable
the exact peer-identity check. Redirect refusal must not become HTTP fallback, a second Origin request
or credential forwarding. The existing fixed Origin, no proxy/retry/redirect/decode and private-error
policies remain unchanged.

The peer supplies synthetic manifest/chunk responses. Its TLS certificate proves transport identity,
not current Session membership, recipient grants or immutable publication. Real Source authorization
and revocation have separate dated SQL evidence; these tests do not replace it or connect a live service.

## Cancellation and independent output

Explicit requests while real HTTPS metadata or a later chunk is held return `Cancelled` only after
that branch is selected. The last snapshot retains the actual ACK prefix and closes without assembly,
save or `Complete` from the cancelled continuation. Previously staged bytes remain evidence; a later
call cannot adopt them as a recovered attempt. Independent healthy fixtures remain usable.

Unused-handle drop, late requests and progress closure preserve their existing meanings. Cancellation
is cooperative and does not preempt synchronous polling, kill OS jobs, revoke an already-selected result,
roll back bytes or authorize retry/overwrite/deletion. The [cancellation contract](large-file-delivery-cancellation.md)
also retains separate actual queued/running file-job tests; a held TLS response is not a blocked disk job.

## Explicit native runner and cleanup

Run `bash scripts/test-file-delivery-tls.sh` with native OpenSSL available. It lists and executes each
of the three exact ignored cases; normal Cargo does not pretend to run them. No database URL, private
deployment key or system trust-store change is needed. Certificates/keys stay in a fixture-owned 0700
temporary parent; processes, pipes, threads and hold gates are drained before those directories are
removed, including failed/cancelled tests. No private key is written to the repository or test log.

`fileDeliveryTls` checks exact inventory, runner/CI wiring, bounded fixture ownership, unchanged
production policy and current bilingual scope. It is static composition evidence, not TLS behavior.
CI invokes the explicit TLS runner alongside the existing large-file runners; it does not mount routes.

UI saving/cancellation, Session issuance, a separate unprivileged data service, deployed LAN acceptance,
resume/recovery, physical disk quotas and throughput/RSS benchmarks remain open. Versions, published
release evidence, **106 historical status blocks**, **52 existing execution tails**, **8 protected files**
and the dated capability tensor remain unchanged. Actual results are recorded below.

## Recorded execution — 2026-10-13

This is new external coverage of an existing HTTPS implementation, not a new transport feature or
manufactured behavioral Red. Source remains **0.5.7**, API **0.3.0**; no commit, tag or push occurred.

| Check | Actual native result |
| --- | --- |
| Exact OpenSSL runner | **3 new cases passed**, zero warnings. Complete delivery covers 4 MiB + 17 bytes in two chunks, empty metadata-only delivery and 17 bytes, with controlled capacity/worker one, owned-0755 or private-0700 targets, exact original GETs/credentials and independent original chunks/f1/selected file EOF, chunk/whole SHA, pattern, 0400, UID/GID, single-link, new inode and unchanged parent identity/policy. Handle drop does not cancel; late requests do not retract saved results |
| Peer refusal | Unknown CA and trusted-CA wrong `127.0.0.2` SAN return the exact existing wrapped metadata `Unavailable`, with **zero independently counted decrypted HTTP bytes**, no accepted requests/files and a closed `FetchingManifest` view. A real HTTPS 302 remains metadata `InvalidResponse`; the independent HTTP poison listener records **zero requests both before and after its drain**. Header rejection need not wait for complete old-peer body output |
| Explicit TLS cancellation | Actual partial metadata, later-chunk headers and later-chunk partial-body holds return `Cancelled` after the explicit request. The server's pipe accepts the first eight plaintext response bytes before a body hold; this does not claim client consumption of eight bytes. Only the actual prior chunk ACK survives, independently SHA/EOF-checked; no f1/target or later `Complete`. A new trusted peer with the original staging/target returns exact chunk-0 `Conflict`, not adoption; its separate empty spy root is not called the old prefix. An independent healthy peer/root/target still saves |
| Parallel and adjacent checks | All **3 new TLS cases passed together in parallel**; the **1 existing single-chunk TLS case passed** separately. This is **4 unique TLS cases**, not seven; repeated executions are not new coverage or a throughput benchmark |
| Fixture/guard corrections | Initial fixture compilation reported two `AsRawFd` reference-bound errors; the helper now borrows `&F` without moving the owned pipe. A static guard's formatting-only substring mismatch was changed to whitespace-tolerant assertions, retaining both poison checks. These are not production behavior failures or a behavioral Red. Native execution and final focused **50 guards passed**, zero warnings/failures |
| Full backend gate | Native `--backend-only`: **1,088 Rust tests passed / 857 optional ignored**, **97 result groups**, **346 common checks passed**, zero warnings/failures. All **15 existing compile-fail examples** passed unchanged; the three new ignored TLS cases were explicitly executed above, not inferred from this default gate |
| Preservation and cleanup | **81 related source-file hashes** are unchanged, as are **106 historical status blocks / 52 existing execution tails / 8 protected release/version/tensor files**. CI remains 600 lines and English project status 1,999. Native fixtures stop/reap OpenSSL and join owned threads before removing their exact temporary key/root directories; final directory/process inventories show **zero new fixture residuals**. Logs remain, keys do not |

Logs: `/tmp/cyanrex-delivery-https-a7QTZfaM/{native-tls,parallel-tls,adjacent-tls,focused-guards,full-backend,post-evidence}.log`.
Initial compilation and formatting-guard logs are retained separately. After recording, common/format,
course-copy, historical/source hash and unchanged Git identity checks are rerun without counting another
backend/TLS acceptance run.

No SQL, new frontend/SDK suite, deployed LAN/system certificate, Session issuer, normal-app route,
unprivileged service separation, 64 GiB/RSS/throughput, disk-full/power-loss or hard job cancellation
was accepted here. A synthetic TLS peer is not a currently authorized publisher or recipient.
