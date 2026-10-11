# Large file transfer and distribution proposal

Included in source release **0.5.6**. The dated implementation and verification below retain their
original 0.5.5/Unreleased scope; source inclusion does not enable network transfer or public APIs.

Status: **Distribution design; network transfer not implemented**. Date: **2026-10-11**. Source version: **0.5.5**.
[Proposal issue #2](https://github.com/Team-silvortex/cyanrex-lab/issues/2) records the approved implementation direction.
The agreed direction is to support both centralized distribution and direct desktop transfers,
delivering centralized distribution first. Files are inert bytes, independent of task type, eBPF,
execution and deployment. Transferring a package never installs, extracts or runs it.

The independent Unreleased [manifest and streaming core](large-file-core.md) implements the first
transport-neutral checks. The separate [Unix staging follow-up](large-file-staging.md) adds exclusive
private chunks and worker admission. The independent [upload-intent ledger](large-file-upload-intent.md)
adds trusted persistent metadata and logical quota reservations, but does not connect staged bytes.
The separate [upload-bound local staging](large-file-upload-staging.md) connects exact original
reservation/source pins to byte receipts, not durable progress. The separate [chunk journal](large-file-chunk-journal.md)
records acknowledged Unknown and exact confirmations without restoring attempts. Separate
[whole-byte verification](large-file-whole-verification.md) observes ordered current chunks and the
whole digest, without adopting recorded status or confirming publication. Independent
[publication metadata](large-file-publication.md) now combines complete confirmed metadata and fresh
real bytes in a create-only protocol, with original checks after acknowledgement. Separate
[Session private metadata](large-file-session-metadata.md) derives owner/account to reserve and read
own metadata in one source transaction; it does not call the publisher or grant recipients. Reviewed
receipt-loss/restart reconciliation, physical space admission and the delivery gates below remain required.

Separate [pure publisher/recipient contracts](large-file-access-contract.md) now define exact
account/version, state and expiry matching. They authenticate no issuer and persist no current grant;
an old active snapshot can still match after revocation. Publisher and recipient permissions are
independent, with no automatic teacher/deployment grant or cascading recipient revocation.
Audited management and current-Session grant reads remain separate implementation gates.

The first user workflow is: an authorized publisher uploads a file, explicitly selects recipients,
and those recipients download the exact published version with resumable progress and integrity checks.
Teacher authority controls instance deployment; a workspace publisher still needs explicit publishing
permission. Neither role grants access to another user's private Artifact.

## Existing boundaries

The current [Artifact store](../../engine/src/services/artifact_store/mod.rs) limits complete buffered
content to 1 MiB. The [text HTTP adapter](text-artifact-http.md) separately limits decoded text to
256 KiB and remains unmounted. Its JSON collector and existing transaction deadlines are not large-file
streaming infrastructure. These limits, old schemas and private owner filters remain unchanged.

The native release tools already demonstrate bounded incremental hashing and reviewed OS SSH access.
[SSH deployment](classroom-connection.md) operates on a pre-installed package; it does not upload one.
Classroom discovery and invitations provide neither device credentials nor file-sharing permission.
No existing prepared HTTP router supplies an authenticated live file-distribution workflow.

## Proposed component boundaries

| Component | Responsibility | Excluded authority |
|---|---|---|
| File transfer contract | Immutable file identity, version, lengths, ordered chunk digests and whole-file digest | Parsing does not authenticate a publisher or verify bytes |
| Private staging and blob storage | Bounded streaming, exclusive temporary files, durable verified chunks and immutable publication | No arbitrary filesystem paths, execution, extraction or cross-user access |
| Publication and recipient grants | Exact publisher/account generation, workspace, version and explicitly named recipients | A file hash, discovery label or teacher role is not a download grant |
| Centralized transport | Authenticated upload/status/finalize and exact-version chunk downloads | No automatic mounting, public exposure, authentication fallback or migration |
| Client transfer controller | Cancellation, verified progress, local destination confirmation and explicit resumption | A browser callback or checkpoint is not a server receipt or completed disk write |
| Later desktop transport | Reuse file/chunk verification with separately authenticated peers | No trust in announced addresses, automatic pairing or shared Runner/SSH secrets |

Control commands and file bytes need separate budgets. The target data plane is an unprivileged
service without kernel capabilities, Docker socket or database credentials in clients. Its reviewed
composition must not expose the privileged teaching Engine merely to serve files.

## Proposed first protocol

Use a separate versioned transfer contract, not a larger text payload or an Artifact limit change.
Each immutable manifest identifies the file/version, total byte length, fixed chunk size, ordered
chunk SHA-256 values and whole-file SHA-256. A canonical display filename is metadata, not a storage
path. The same manifest can later travel over centralized or desktop transports.

The current preparation contract fixes **4 MiB chunks**, **64 GiB per file**, and at most **16,384
chunks**; network adapters remain subject to review. These are protocol ceilings, not promises of available disk capacity. Trusted deployment
configuration may choose lower per-account and total reserved-byte quotas. Length/offset arithmetic
must be checked; malformed counts, duplicate fields, unknown versions, missing digests and noncanonical
digests fail closed. Empty files have zero chunks and the verified empty-content digest.

The first network adapter should transfer a single exact chunk per request instead of arbitrary
multi-range responses. Chunk index, expected length and digest are all bound to one immutable manifest.
Upload sends binary bytes rather than Base64/JSON. Download uses attachment disposition and a fixed
binary media type; names never become executable paths. Browser and native destinations need their
own reviewed save mechanics before claiming durable client delivery.

## Upload and publication

1. An authorized start command reserves bounded disk quota and records the immutable upload intent,
   publisher account generation and manifest identity. Requests cannot choose a storage root.
2. Each chunk is streamed through a fixed buffer into an exclusively created private temporary file.
   Verify actual bytes, exact length and digest before marking the chunk durably confirmed. Reject
   conflicting content at an existing index; do not overwrite it.
3. Explicit status/resumption reauthorizes and checks stored chunks before reporting verified progress.
   An imported bitmap or a local success callback is not sufficient. Restart recovery must validate
   bytes, the original upload identity and its storage incarnation.
4. Finalization incrementally verifies ordered chunks and the whole-file length/digest. Incomplete,
   corrupt, expired or revoked uploads cannot become published. Only confirmed durable publication
   makes the exact version eligible for recipient grants and downloads.

Data streaming and hashing must not hold the existing ten-second Session transaction or workspace
writer locks for the whole file. Short commands authorize/reserve, reconcile staged results, and
confirm publication separately. The final durable check must reject changed identity, authority or
upload state after IO. File durability and metadata commits are not one filesystem/database transaction.

After an uncertain result, retain evidence and query exact current state. Retry semantics must bind
the original upload and immutable chunk, reject conflicting bytes, and never adopt a different file.
Do not advertise exactly-once delivery or automatically delete uncertain files. An explicit cancellation
stops admission; cleanup requires confirmed upload state, no active workers/readers and a reviewed target.

## Distribution authorization

Publisher approval names exact recipients and the immutable file version, size and digest. Grants
belong to one authority/workspace and recipient account generation, with expiry/revocation state.
Recreating an account or changing a display name cannot inherit access. Listing itself is private.

Each chunk download rechecks current authentication, membership, grant, publication state and expiry.
Knowledge of an ID/hash, invitation possession or a manager role is insufficient. A grant for one
version cannot follow a later replacement. Revocation prevents subsequently admitted chunks but
cannot retrieve already delivered bytes. Any in-flight revocation window must be bounded and documented.

Short-lived device credentials, pairing and peer-specific transfer delegation are separate desktop
milestones. No global Agent registration token, Session in a URL, SSH private key or reusable classroom
invite is a substitute for a download capability. HTTPS identity verification or a reviewed SSH tunnel
remains necessary on a LAN; discovery never authenticates a target.

## Resource and disk safety

Bound actual disk/hash workers independently of HTTP requests. A dispatched worker retains its permit
until it finishes even if its caller stops waiting. Separately cap active uploads/downloads, control
requests, chunk memory, bandwidth, reserved bytes and metadata/chunk counts. Slow or empty-frame
streams require transfer-specific idle and lifetime limits, not the text adapter's two-second deadline.

Linux storage must reject symlinks, hardlinks, special files, path traversal, replaced roots and
existing destinations, using descriptor-relative access and private permissions. Immutable chunk
storage should avoid a mandatory second complete assembled copy. Cross-owner deduplication is deferred
to avoid visibility and accounting leaks. Explicit retention and garbage collection cannot remove
published, granted, actively read or uncertain data by inferring absence from a failed query.

## Delivery order and acceptance

| Slice | Required evidence before the next slice |
|---|---|
| Transfer contract and streaming core | Files larger than the old Artifact limit; bounded buffers; exact lengths/digests; chunk boundaries, overflow and source-change rejection; private temporary-file safety |
| Persistent upload and publication | Confirmed chunks, quota races, disconnect/restart verification, duplicate/conflicting chunks, filesystem/metadata failure and unknown-commit handling |
| Publisher and recipient authorization | Cross-user/instance rejection, recreated accounts, expiry/revocation at real waits, exact-version grants and no legacy private-read relaxation |
| Central transport and client | Authenticated chunk round-trip, response/worker capacity, resumable interrupted transfers, safe saves and progress, then explicit UI/SDK and deployment wiring |
| Desktop transfers | Explicit pairing, authenticated peers, version negotiation, recipient-bound delegation and the same malicious-chunk/cancellation tests |

Include real disk-full/short-write faults, corrupted/missing chunks, dropped requests, post-restart
inspection, cleanup guards, and healthy controls after each fault. Benchmarks must measure throughput,
peak memory, worker occupancy and disk amplification for representative large files and multiple clients.
In-process or mocked transport tests do not establish browser persistence, real TLS/LAN performance,
deployment acceptance or desktop interoperability.

A proposal issue is required by [the contributor workflow](../../CONTRIBUTING.md) before implementation.
The manifest/stream and private-staging steps change no running listener or storage schema. The
upload-intent follow-up adds a separate fresh-only schema, without installing it in live data. All
these components and the independent chunk journal preserve Artifact/text limits, public API, release,
deployment data and maturity evidence. Whole-byte verification stays read-only. Separate immutable
publication metadata is not current authentication, permanent physical sealing or a download grant;
reviewed receipt-loss/restart reconciliation and physical space admission remain incomplete.
Centralized distribution is not complete until persistence,
authorization, transport and client gates above pass.
