# Private whole file assembly

Included in source release **0.5.7** (**2026-10-12**). Dated 0.5.6/Unreleased implementation and
verification records retain their original scope; source inclusion does not mount or validate a deployment.

Status: **Unreleased Unix preparation**, source **0.5.6**, API **0.3.0**, **2026-10-12**.
This trusted local operation assembles existing legacy `v1` chunks into one private file and checks
the actual ordered chunk and whole-file digests. It follows the [single-chunk client](large-file-chunk-client.md)
and [proposal #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2); it is not a complete downloader,
user-selected save operation, publication, resume protocol or current remote authorization check.

## Input and output ownership

Included in 0.5.8, [ordered private download](large-file-download-client.md) composes this assembler
with HTTP/stage calls using the same original handle. Its optional crate-private flow reference enters
the actual assembly job. This standalone method still fetches nothing, and its old execution record
is not a new network-to-whole result.

`services::file_transfer::staging::ChunkStaging::assemble_staged_file(&manifest)` accepts only an
existing trusted staging handle and typed manifest. The workspace must match before admission.
It takes no destination path, user filename, reader, hidden upload key, network credential or SQL
store. An empty manifest still requires the original private root and creates a real empty file.

The same originally opened owned **0700** directory contains both inputs and output. The output
name is `f1-<SHA256(full typed manifest)>`, distinct from `v1-<fingerprint>-<index>` inputs and
Source/upload-bound `u1` chunks. Display metadata never becomes a path. Streaming typed serialization
computes the fingerprint without materializing the manifest JSON. Equal bytes or a matching
`FileVersionRef` do not merge different complete manifests or adopt another file.

Success returns a move-only `AssembledFile` exposing only `file() -> &FileVersionRef` metadata.
The Unreleased [explicit private save](large-file-save.md) follow-up now privately retains the original
read-only output/root descriptors, Snapshot and complete manifest binding after real final checks;
the original writer stays live until that read-only inode opens, then closes before byte readback.
There is still no public constructor, Clone, serialization, byte/path getter or descriptor. Retained
proofs hold two descriptors, not workers or remote authority; old execution records below are unchanged.
This local acknowledgement is not a durable progress record, authenticated provenance, remote
delivery receipt or authority to recover a cancelled/lost acknowledgement.

## Checks and failure outcomes

The operation uses descriptor-relative no-follow traversal and the existing protected-ancestor,
owner, regular-file, single-link and **0400** input checks. Original entry snapshots include device,
inode, size, mode checks and modification/change times. Only one chunk descriptor is retained at a
time; a final bounded metadata scan checks earlier entries after later chunks have been read.

All existing output types conflict, including a valid complete file, symlink, hardlink, FIFO or
directory. Input verification checks each exact per-file EOF, ordered chunk SHA-256 and actual whole
SHA-256 before creating output. Missing, corrupt or unsafe inputs fail without producing output.
The copy reads and checks real bytes again against the original observations, rather than treating
the first pass or manifest as a reusable integrity promise.

New output uses exclusive creation with requested **0600** permissions. After checked copying it
becomes **0400**, synchronizes file and directory, closes the writer and verifies actual bytes from
the originally created visible inode. Original input snapshots and the root are checked again before
return. Output remains inside the private directory throughout; this is not atomic public publication
or a rename into a user's chosen location.

Scope mismatch is `InvalidInput`; fail-fast worker admission returns `CapacityExhausted`. Before
creation, failed input/root checks return `Unavailable` and an existing output returns `Conflict`.
Other creation errors, post-creation failures and lost blocking-job results are conservatively
`Unconfirmed`. A partial 0600 or complete 0400 output can remain. Nothing is overwritten, adopted,
renamed, deleted or automatically retried. Neither cancellation nor matching retained bytes proves
rollback or restores the original caller's acknowledgement.

## Resource limits and integration

Assembly shares the original clone-shared **1–16** staging workers, not a new unbounded pool. Admission
precedes metadata cloning and one blocking dispatch. The actual queued/running job owns its slot until
completion even if its async waiter is dropped. Independent staging handles remain additive.

The stream buffer is **64 KiB**; metadata is bounded by the existing **16,384 chunks / 64 GiB** manifest
contract. Preverification, copying and output readback require three full payload passes, plus bounded
metadata scans. No whole payload Vec or one-descriptor-per-chunk set is retained. These are structural
bounds, not measured RSS/throughput or a global process quota. The extra private whole copy consumes
disk space without deleting chunks; there is no reserved-space policy, autonomous cleanup, hard file
deadline or kill mechanism.

Same-UID/root changes can still occur after a successful observation; 0400 is not WORM storage or
isolation. Filesystem synchronization is not tested power-loss durability. Local bytes do not establish
current recipient membership, grant validity or server identity. The client and receiver remain separate
calls; no manifest fetch, multi-chunk scheduler, restart/resume, Session issuer, normal AppState/route,
OpenAPI/SDK/UI, migration, live deployment or peer transfer is added. Versions and dependencies stay fixed.

## Tests and evidence

`file_chunk_assembly_tdd` uses real owned private files and independent output EOF, SHA-256, byte and
inode checks. Private staging units test deterministic queued/running cancellation and observable drift
inside actual jobs. `fileChunkAssembly` guards cover composition and documentation only, not behavior.
The normal backend gate runs these Unix cases; no new database or network runner is required for assembly.

Current bilingual notes and generated course copies describe the local edge. Earlier **106 status
blocks**, **34 detailed execution tails** and the dated capability tensor remain unchanged; this is
not a new maturity score or reuse of previous SQL/TLS evidence as fresh execution.

## Recorded execution 2026-10-12

The inert assembly scaffold compiled against a real **4 MiB + 17** source, two independently checked
legacy chunks and an owned 0700 root. Clean behavioral Red was **0 passed / 1 expected failed**,
`Unavailable`, zero warnings: no whole output existed. Green passed **1 / 0**, zero warnings, with
independent reopened EOF/chunk/whole/byte, 0400, single-link and original-input checks. Fixture cleanup
preceded the final assertion. The first attempted test had an `E0507` closure-move compilation error;
it was corrected and rerun, and is not behavioral Red.

| Fresh selection | Actual result and scope |
|---|---|
| Assembly default target | **8 passed**, zero ignored/warnings; real empty/17-byte/cross-chunk output, scope/full-manifest refusal, wrong actual whole digest despite individually valid chunks, missing/corrupt/short/extra/writable/linked/special inputs, every existing output type including dangling symlink, and ancestor/root restoration controls |
| Private staging units | **14 passed**, zero warnings: **8 existing + 6 new**; actual queued/running cancellation retains shared capacity until the original created inode finishes, earlier input/output substitutions and post-readback root replacement retain unconfirmed evidence, same-inode equal-length byte tampering is rejected by actual output SHA readback |
| Compile-fail contracts | All **12** default documentation cases passed, including **3 new** prohibitions on public construction, Clone and serialization; these are already included in the full Rust total |
| Adjacent explicit SQL runners | **8 whole-upload + 13 publication + 10 current-recipient reader passed**, zero warnings; **31 unique fresh cases** on this run's disposable PostgreSQL 16 private socket, not new assembly schema/API tests |
| Full backend gate | `scripts/quality-gate.sh --backend-only`: **936 Rust passed**, **841 opt-in ignored**, **88 result groups**, **301 common passed**, zero warnings/failures |

The eight new defaults, six units and three documentation cases are already included in 936, not
additional passes. Ignored SQL/kernel/external inventory is not default acceptance; the 31 explicit
SQL passes are separate. No earlier client SQL/TLS evidence is relabelled as fresh. There is no new
frontend/SDK/security-audit run, real LAN/TLS deployment, network-to-whole scheduler acceptance,
user-selected save/resume, disk-full/fsync/power-loss fault test, 64 GiB payload execution or measured
RSS/throughput. Structural 64 KiB/manifest bounds do not substitute for those measurements.

All source editors froze before root-owned Cargo execution; production owner/ancestor checks stayed
intact under native file/socket tests. Final database audit reported the exact temporary data directory,
no TCP, **zero fixture schemas and zero other clients**. The server was stopped; only validated `data`
and `socket` were removed, releasing **101048 KiB (about 99 MiB)** of reconstructible test data.
Logs remain at `/tmp/cyanrex-file-chunk-assembly-psHCkBWh`; no project/deployment data was deleted.
Versions, dependencies/lockfile, normal startup and live API/SDK/UI are unchanged; no commit/push.
All **106 old status blocks**, **34 old execution tails** and the capability tensor remain unchanged.
The initial composition guard wrongly treated a private phase enum's Clone as a public ACK Clone;
it was narrowed to `AssembledFile` and passed, not counted as behavioral Red. Review added the two
readback/root cases and dangling-symlink control before final acceptance. Bilingual notes/course
copies are synchronized. Post-evidence `scripts/quality-gate.sh --format-only` passed **301 common
checks**, zero warnings/failures, plus version/course/OpenAPI drift, tool and Rust formatting checks.
