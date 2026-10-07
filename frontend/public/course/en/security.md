# Security and deployment boundaries

This guide covers the current runtime, local editing and general collaboration preparation. The
platform direction does not make the deployed Engine unprivileged or migrate its authentication.
For relationships between modules, see [architecture](architecture.md) and the [feature map](platform-network.md).

## Key Conclusion

Cyanrex Engine is a privileged kernel experimentation environment, not a multi-tenant safe sandbox.

Docker requires access to host/VM eBPF, tracefs, BTF, and bpffs. Authenticated users, including students,
can perform kernel experiments; teachers also control deployment settings. Do not expose Engine to
untrusted networks or treat application roles as kernel isolation.

## General platform security boundaries

| Surface | Current guarantee | Not a guarantee |
|---|---|---|
| Local task payload editor | Bounded text/JSON, parent-owned revisions and target-bound confirmations | Persistence, server identity, execution safety or isolated JS/TS workers |
| Direct Task/Artifact/Review stores | Explicit scope and trusted-owner filtering, bounded records and confirmed database writes | Authentication of a caller-supplied owner or historical Session provenance |
| Session resource commands | Current account incarnation, audited membership, namespace pins and final Session checks in one transaction | Teacher access to others' private content, cross-user Review authority or Task acceptance |
| Artifact publication and Task replacement | Immutable exact references; Draft changes check old/new inputs and expected revision | Atomic file/database rollback, automatic adoption or permission to delete newly published files on failure |
| Domain catalogue | Exact configured definition metadata | Trusted package installation, typed evidence validation by admission alone, or policy execution |
| Telemetry and business events | Separate live event history and transactional resource outboxes | Reliable dispatch, exactly-once workflow delivery or a complete security audit from the Events page |

Preparatory installers accept only explicit empty namespaces. Task storage schema 2 rejects schema 1;
do not edit a version number, drop data or rerun bootstrap to bypass refusal. Live teaching tables,
credentials, private Artifact roots and rollback material are not synthetic test fixtures. Retain
uncertain outcomes for reviewed reconciliation; timeout or lost acknowledgement is not proof of rollback.

For source-owned private work, see [Task inputs](session-task-inputs.md), [Draft replacement](session-task-revisions.md)
and [private Reviews](session-review-commands.md). These paths are not yet public browser APIs, so
their service tests do not establish live HTTP/CSRF or deployment acceptance. Application roles and
source-level abstractions still do not isolate the shared Linux kernel.

## Default Protections

The protections in this section apply to the existing runtime unless a preparation-layer contract is
explicitly named. They must not be treated as interchangeable authentication or storage policies.

- Docker published ports default to `127.0.0.1`. The native/WSL launcher currently sets
  `ENGINE_HOST=0.0.0.0`; verify actual listeners and restrict access before starting that mode on a LAN.
- First startup generates random DB, deployment-teacher, and TOTP secrets (legacy `CYANREX_ADMIN_*` keys).
- `.env` permissions are `0600` and ignored by git.
- User registration and OTP bootstrap are disabled by default.
- eBPF checks, semantic completion, script list/save/delete, and session-bound operations are available to authenticated users.
- Teachers own module changes, system settings, Runner Agent management and teaching; students cannot
  use these management APIs. Old administrator identities are compatible teacher identities.
- Public registration cannot select a role or claim a configured teacher/legacy-admin name. The seeded
  deployment teacher cannot delete itself, preventing a student-only, unmanaged instance.
- Temporary lockout after repeated login failures.
- Global and per-user Runner capacity, source size, and execution time limits are enforced for eBPF tasks.
- Runner status reports `shared_kernel`; local quotas do not claim tenant isolation.
- Local check/completion caches are owner-scoped. Their source workspaces use `0700` on Unix and
  cleanup guards run on return or cancellation; failed cleanup is logged. These are hygiene measures,
  not an independent-kernel boundary.
- DB stores session token hashes, not raw usable tokens.
- Cookie-authenticated `/ws/events` handshakes also enforce the configured Origin/Referer policy;
  CORS headers alone do not protect WebSocket connections. Native clients must send an allowed Origin.
  The existing `CYANREX_ALLOW_MISSING_ORIGIN` override also affects this endpoint; keep it disabled on
  a LAN unless its security tradeoff is explicitly accepted. See [Event Stream Recovery](event-stream.md).
- Event live queues use the authenticated owner rather than a client-supplied identity, preventing
  cross-owner queue eviction. Capacity is per active owner; this is not a global connection/memory quota
  or a tenant-isolation boundary. The existing Origin/session policy and shared-kernel limits still apply.

These measures reduce accidental exposure, but they do not make the privileged Engine a shared safe runtime.

### Teacher-authority upgrade

All existing teacher allowlist entries now grant deployment management, not merely teaching reads.
Review both `CYANREX_TEACHER_USERNAMES` and `CYANREX_ADMIN_USERNAMES` before deploying this change.
The default deployment account is the teacher in personal use; this never promotes the first public
registrant, trusts a client-supplied role, bypasses TOTP, or changes kernel/Agent privileges. Configure
additional teachers only after verifying an existing account's owner; see the [teacher guide](teacher-guide.md).

### Accidental-action safeguards

The UI asks for an explicit target/impact review before kernel runs, detach, destructive deletion,
draft replacement, administrative state changes and remote compiler selection. Bulk deletion/cleanup
requires a typed phrase; the default focus is Cancel. In-flight duplicate clicks are blocked, and failures
require checking state before a new confirmation. Event deletion freezes its time cutoff and does not
claim that the bounded visible count is the deletion total. These controls are browser-side ergonomics,
not server authorization, idempotency keys, transactional rollback or kernel isolation. A dispatched
request may complete even if the browser leaves the page. API clients still rely on the existing Engine
session, teacher/student role, CSRF, password and OTP policies. Role consolidation does not replace
these server-side checks.

### Sessions and database outages

Logout ends the current session, not every device's session or running kernel experiments. The browser
opens the login page only after an HTTP-success response explicitly confirms logout. Rejection, an
invalid response, timeout or network failure instead shows that the session may still be active and
requires a manual retry. Repeated clicks are blocked while waiting. Navigating away discards late UI
callbacks, but does not undo a request already received by Engine.

For PostgreSQL-backed authentication, logout, password change and account deletion require confirmed
storage writes. Failures return `503` without clearing the session cookie or publishing a memory-only
mutation. Session removal and account deletion commit in one transaction. Password/account writes use
the verified normalized username and credential snapshot; zero affected rows are rejected rather than
reported as success. A concurrent password change cannot silently overwrite newer credentials.
Logout with no affected row succeeds only after confirming the session is already absent.
Account deletion also verifies no session survived, including a suppressed cascade.

Login admitted while PostgreSQL is active must insert exactly one session and recheck its verified
credentials in the same transaction before publishing a cache token/cookie. A deleted/recreated
account or concurrent password change cannot accept the old pending login. An unconfirmed login write
returns `503` without a new cookie or memory-only success; cancellation before commit rolls back.
An already committed but unacknowledged login can leave a session until expiry. This does not remove
the existing read/registration fallback or provide distributed revocation.

A `503` or lost response is not proof that nothing committed: restore storage and verify the current
account/session state before explicitly retrying. A failed write alone does not disable persistence;
if an earlier read failure already latched permanent auth DB fallback, restore PostgreSQL and restart
Engine before these mutations. This also applies to temporary accounts registered during that outage.
Intentionally memory-only instances keep volatile account/session behavior. Do not switch a durable
deployment into memory-only mode merely to bypass an authentication storage error.

When PostgreSQL reports a missing or expired session, Engine also removes its memory fallback entry.
A missing account lookup removes that cached account and its cached sessions. Those observed
invalidations cannot reappear merely because a subsequent query fails. The existing volatile memory
fallback still cannot discover revocations or credential changes made elsewhere while the database is
unavailable; this is not a distributed revocation guarantee or a replacement for incident recovery.

The [C1-H source adapter](collaboration-password-change.md), included in 0.4.7, separately supports atomic
self-service password change and all-session revocation with current Session/password/TOTP checks.
It is not wired to the live endpoint described above, does not restore retired identity permissions,
and must not share activated source tables with the legacy live writer.

[C1-I bootstrap](collaboration-bootstrap.md) is explicit trusted-operator code for a fresh namespace,
not an unauthenticated setup endpoint or first-registration claim. It rejects existing/partially installed
schemas, commits initial identity/grants/audits together, and issues no Session. Lost acknowledgement
requires reconciliation, not deleting data or automatically creating a replacement manager.

The separate [C1-J local CLI](collaboration-provisioning.md) restricts endpoints to Unix sockets or
literal loopback, ignores ambient database/runtime settings, and requires private owned inputs plus
an explicit reviewed target digest. TOTP is written only after confirmed commit into an exclusively
reserved private file, never the console. Database commit and file delivery are not atomic: failure
can leave a committed account with missing/partial enrollment material. Preserve evidence and reconcile;
catalog-only `inspect` is neither a healthy-authority check nor a secret recovery or replay mechanism.

[C1-K reconciliation](collaboration-reconciliation.md) reads source/registry/full audit histories in
one bounded read-only snapshot. It rejects filtered/incompatible layouts and reports aggregate counts,
not credentials, token digests or individual identities. A consistent snapshot can precede an in-flight
commit; it neither proves an uncertain command's outcome nor authorizes reinitialization, secret
recovery or protected actions. Existing authorization still needs fresh Session checks and row locks.

The [durable Session-source follow-up](collaboration-auth-source.md), included in 0.5.1, pins namespace and
auth-table identity for login, validation and logout, including login's password-worker gap and
post-write path drift. Only one effective private schema and unfiltered permanent source tables are
accepted; no registry, migration or live authentication switch is implied. This prevents an empty
shadow read from falsely confirming logout, but is not public-issuer readiness.

Prepared registration, login, password rotation and bootstrap now share a process-local gate of four
dispatched password jobs and twenty admitted jobs in total. A blocking job retains its capacity after
caller cancellation/timeout until it actually finishes; work waiting for a slot stays inside the
caller's existing deadline. Separately, prepared account reads and pre-admission verification enforce
the existing Argon2id v19/m19456/t2/p1/32-byte profile, including bounded PHC text, decoded salt and
rejection of duplicate/unknown parameters. Other profiles return `InvalidRecord`, without credential
rewrites or implicit Session revocation. This does not limit old live AuthService work, other processes,
wall-clock time or process RSS, and does not kill running computation. Timing enumeration
and ingress/session limits remain gates before untrusted Session issuance.

The 2026-10-07 login follow-up included in 0.5.2 gives a genuinely missing account the same supported password
profile through one synthetic verification on the shared gate, after source-pin rechecks and confirmed
commit of the lookup transaction. It never substitutes for corrupt credentials or a source/database
error. Login still separately requires the original real account, so matching the public synthetic
record or registering that name during verification cannot issue a Session or consume an OTP.
Input and attempt limits remain earlier checks, and dispatched work retains its permits on cancellation.
This removes the missing-account KDF shortcut, not every timing difference or enumeration channel;
there is no constant-time promise, new public issuer, live-auth switch or schema change.

Prepared login and rotation also recheck OTP freshness behind the writer fence and after the last
SQL wait before requesting commit. Expiry at that point rejects issuance or rolls back rotation.
The schema-2 follow-up also consumes a monotonic per-account counter in the same transaction as
Session issuance or password replacement/revocation. Current full credentials and the old watermark
guard the update; post-write checks and a bound exact-counter recheck precede COMMIT. Independent
sources cannot both commit the same step. Password changes preserve consumption; re-login needs a
later OTP. Collision handling rejects any matching consumed counter, not all future appearances of
the same six digits. Pure proposals alone are not durable state or account authority.

Only fresh explicit installation/bootstrap creates schema 2; existing schema 1 is rejected unchanged.
There is no counter reset, secret recovery, automatic migration or live AuthService switch. A lost
COMMIT reply may leave consumption committed without delivering a token: do not assume rollback or
reset state to retry. This is neither freshness at acknowledgement nor protection against every clock
rollback. Version fencing applies to cooperating prepared writers, not legacy or privileged SQL
writers; they must remain isolated. See [the full OTP contract](collaboration-auth-source.md).

The explicit `prune_expired_sessions` maintenance included in 0.5.2 selects at most 128 expired rows under the
source writer fence and one fresh database cutoff. It validates the complete batch before deletion,
compares returned full rows and confirms all selected digests are absent in the original pinned source,
including attempted reinsertion with future expiry. SQL rejects selected candidates' infinite or
out-of-range timestamps before decoding on this maintenance path. Corrupt/orphaned candidates fail
rather than being repaired. Only a confirmed commit returns the batch count; an empty or partial count
is not proof of global emptiness or a bound on Session storage. Cancellation or lost acknowledgement is not rollback
evidence. The primitive reads no credentials/OTP state and its explicit mutations target only Sessions;
these checks do not audit arbitrary trigger effects throughout the database. There is no automatic
schedule, CLI/HTTP entry or login/read cleanup side effect. Active-Session quotas and maintenance
admission/recovery remain separate policies; see [the cleanup contract](collaboration-auth-source.md).

The 2026-10-07 Session-expiry follow-up included in 0.5.2 rejects infinite and out-of-range `expires_at` values
inside SQL before the shared prepared Session reader decodes them. Validation, login writeback and its
calling guards receive `InvalidRecord`, not a panic or silent absence; normal finite expiry remains
unchanged. Read-only reconciliation keeps the range/expiry comparison in SQL, fetches only an expiry
boolean and returns `InvalidSource` for bad values, without exposing raw expiry. Registry timestamps
were outside that stage and are covered by the separate follow-up below. It changes no
dependency, live issuer, schema or public API; see [the Session contract](collaboration-auth-source.md).

The subsequent 2026-10-07 [registry guard](collaboration-reconciliation.md), also included in 0.5.2, checks stored
retirement and both identity/policy audit times in SQL before decoding. Legitimate NULL retirement
remains valid, while an independent validity flag rejects corrupt non-null retirement; mandatory audit
times cannot become missing receipts. Head/history/replay/append-readback and reconciliation paths
retain explicit errors rather than panic, skip or repair records. This changes no age/order rule,
dependency, schema or live authorization boundary and is not blanket timestamp-safety assurance.

### Browser diagnostic boundary

Browser inline compiler caches and pending checks belong to one editor mount and exact Engine/target/
source/header context, not a process-global pool. Leaving/re-entering the editor discards that cache;
cancelled callbacks cannot publish stale diagnostics. This does not observe cookies changed in another
tab, revoke sessions, erase saved drafts or create an authorization boundary. Diagnostic requests reject
redirects and request no-store; queue/lease ownership is still enforced by Engine. Timeouts and best-effort
job cancellation do not prove server-side rollback. See [bug hunt 03](functional-network-bug-hunt-03.md).

Semantic providers also reject foreign/disposed models and have editor-owned requests/cache; manual
header checks cancel on source or context changes. Completion and selected-header reads have ten-second
whole-request limits; manual checks use the local 20-second limit. All three request paths reject redirects
and request no-store. A failed metadata refresh displays the last successful list explicitly, never a
claim that no headers are selected. Client context revisions only invalidate UI work: Engine still chooses
the session owner and current headers, and teachers retain exclusive header mutation authority. There is
no atomic header snapshot, cross-tab revocation detection, remote completion or automatic execution added.
See [bug hunt 04](functional-network-bug-hunt-04.md).

### Browser runtime boundary

Manual run/detach requests include credentials, request no-store and reject redirects. A local admission
guard prevents concurrent dispatch from one controller; it is not cross-tab idempotency. Navigation aborts
browser waiting; editing hides old results, and neither action unloads an already dispatched program.
The 330-second mutation and 20-second inventory limits are active-browser waiting bounds, not proof of
server rollback. Uncertain results require review and never trigger an automatic retry or detach.
Only explicit `clean: true` confirms cleanup; a failed/malformed inventory is not empty success, and stale
inventory disables cleanup controls until refreshed. Engine still enforces session ownership, CSRF and
Runner authorization. The explicit `pin_path: null` bulk action retains its existing current-account scope,
including attachments added before execution, as disclosed in its typed confirmation; it is not a frozen
server snapshot. No new session-revocation detection or kernel inspection is added. See
[bug hunt 05](functional-network-bug-hunt-05.md).

Breakpoint session/line filtering, strict trace-marker parsing and model-owned highlights prevent stale
or malformed observations from being attributed to the current draft. They do not authenticate a kernel
producer: session markers in a shared trace log are not secrets or a tenant boundary. Only the Engine's
session/Origin checks and owner-scoped event subscription confer access. Event recovery snapshot reads
also request no-store and reject redirects; malformed frames retain a possible-gap notice, not a claim
of complete recovery. No new revocation, replay guarantee or automatic kernel action is added. See
[bug hunt 06](functional-network-bug-hunt-06.md).

### Event history and acknowledgement boundary

Event export/deletion and unread/acknowledgement requests retain credentials, request no-store and reject
redirects. Browser waiting is bounded to 20 seconds for export/deletion and ten seconds for unread state;
navigation cancellation is not server rollback. Failed or malformed deletion/acknowledgement responses
are never treated as success, and mutations are not automatically retried after failure. The unchanged
mark-read endpoint covers ALL events for the session owner, including rows outside the current filters
or 200-row view; the UI explicitly discloses this. It is not a per-record or durable acknowledgement.
MIME/filename checks prevent accidental wrong-format downloads, not spreadsheet formula sanitization,
archive authenticity, complete event replay or a new payload byte budget. See [bug hunt 07](functional-network-bug-hunt-07.md).

Engine event mutations now wait for earlier queued writes and retain per-owner admission through admitted
SQL work and cache publication. This closes local ordering/cancellation races without changing session,
CSRF or teacher authority. The private replacement helper rejects foreign-owner records. Queue overflow
and storage deadlines warn and latch volatile fallback; successful read/deletion HTTP responses still do
not promise durable revocation of events across restart or other processes. Do not use this stream as an
exactly-once security audit log. Transactions protect settings/trim and replacement, not the whole
publication/HTTP/recovery lifecycle. See [bug hunt 08](functional-network-bug-hunt-08.md).

### Learning data boundary

Learning record reads fail visibly on corrupt/unreadable storage instead of showing an empty classroom.
Successful learning projections and their storage errors are no-store. New local learning directories
and snapshots use Unix 0700/0600 and exclusive random temporary files with failure cleanup. Keep existing
data-directory ownership and parent permissions restricted; old directories are not automatically chmodded,
and this does not provide crash recovery or hostile-parent path protection. See [learning storage](learning-storage.md).

### Classroom connection boundary

Classroom onboarding uses a separate [discovery/invitation boundary](classroom-connection.md).
Discovery labels and IDs are not certificates; verify the exact HTTPS teacher origin independently.
Never send the shared Agent bootstrap token to students. Invitations are opt-in, short-lived,
name-bound and atomically single-use, with no automatic login or teacher promotion. Compatibility
checks the join protocol and required capabilities, not merely matching product patch numbers.
SSH management stays in the native operator CLI with strict host checking and explicit target review;
it cannot be triggered by a student invitation or discovered command. Neither entry changes kernel
isolation or listener exposure, or removes the need for trusted ingress and per-student isolation.

### Runner Agent credential boundary

Remote Agent registration is disabled by default. Enabling it requires a secret
`CYANREX_RUNNER_AGENT_TOKEN` of at least 32 characters. Keep the endpoints on a private management
network or behind TLS and source restrictions; never place the token in frontend code, logs, or a
repository. Rotate it when a node is lost or compromised.

The pre-shared Bearer token is accepted only for registration. Registration returns a distinct
256-bit Agent credential once with `Cache-Control: no-store`; re-registering the ID rotates it.
Heartbeat and job endpoints require HMAC-SHA256 over method, path, Agent ID, Unix timestamp, nonce,
and body hash. The timestamp has a bounded freshness window and each nonce is accepted once.

A holder of the bootstrap token can still re-register and impersonate an Agent ID, so it remains a
high-value secret. Jobs have per-claim lease tokens, deadlines, cancellation acknowledgement, and
size-limited results. The editor's local compiler remains the default; explicitly selected remote
checks carry source to a capability-matched Agent but never load or return the object. The public
backend inventory omits labels and kernel details, remote jobs are bound to the authenticated user,
and each user is limited to two active checks. `/ebpf/run` remains local. Signed transport proves
which registered credential sent a result; it does not prove that compilation was honest. Consider
mutual TLS, node-bound keys, and attestation across a stronger trust boundary.

Lost leases observed before execution are discarded rather than executed or acknowledged as cancelled;
the polling loop remains available for later jobs. This does not interrupt compilation already in progress.
Unclaimed user checks expire after a 35-second wait on the next queue access, releasing source and user
quota; staff-managed unowned jobs keep their explicit cancellation policy. Response buffering rejects
more than 640 KiB while streaming, even without a declared length, before reading the whole response.

The bundled standalone Agent fails closed on non-loopback plain HTTP unless an explicit insecure-lab
override is set. It disables HTTP redirects and environment proxies, caps response bodies, and can
read the bootstrap secret from a mounted file. Compile mode is opt-in and rejected for
`shared_kernel`; it invokes only an absolute Clang path with fixed arguments, clears the environment,
uses private disposable workspaces, bounds process resources and captured output, and deletes the
object after hashing it. Source preflight also rejects non-literal or path-escaping include forms.
Run the Agent unprivileged without host PID mode, kernel mounts, Docker socket, or Linux
capabilities. Clang processes untrusted input, so retain a container or VM boundary.

The optional managed Compose profile applies that baseline automatically: numeric unprivileged UID,
read-only root filesystem, no published ports, all capabilities dropped, `no-new-privileges`, a
`noexec` temporary workspace, and PID/CPU/memory caps. Its manager writes the bootstrap value to a
mode-0600 host file mounted as a Docker Secret and never prints it. The profile remains disabled
until the teacher explicitly starts it. A dedicated internal network permits Engine control
traffic but blocks direct access to the default database/frontend network and external networks.

## Recommended Isolation

For multiple students submitting their own code, the chosen next-stage target is an unprivileged
teaching control service plus an exclusive Linux VM per active student. Desktops may host these VMs;
centralized execution hardware is optional. This target is not yet implemented. See
[Linux Desktop and LAN Classroom Isolation](classroom-isolation.md) for ownership, recovery, and the
limits of trusting results from student-administered machines. Do not enable remote loading by simply
giving the current compile-only Agent elevated privileges.

### Personal Computer

One instance per student. Windows users should use WSL2, Linux users choose native Linux or Docker. This is preferred.

### Remote Personal VM

One VM per student, accessed via SSH tunnel. A VM can be fully destroyed after class.

### Centralized Server

Do not share one Engine among multiple untrusted students. If resources are limited, isolate with VMs/microVMs,
independent lab nodes, and limit network, CPU, memory, and lifetime per node.

## Prohibited Operations

- Do not set `CYANREX_BIND_ADDRESS` to `0.0.0.0` and expose directly to public internet.
- Do not commit or share `docker/.env`.
- Do not mount Docker socket into Engine.
- Do not disable auth or treat public registration as classroom isolation.
- Do not allow unknown source programs from students.
- Do not run production workloads on the same trust boundary as teaching Engine.
- Do not treat verifier as a full malware sandbox.

## Remote Access

Prefer SSH tunnel:

```bash
ssh -L 3000:127.0.0.1:3000 \
    -L 8080:127.0.0.1:8080 \
    USER@SERVER
```

Long-running services should use TLS reverse proxy, source restrictions, and host firewall, and set
`CYANREX_SECURE_COOKIES=true`. Enable Secure Cookie only when browser access is HTTPS.

Current Compose uses `CYANREX_BIND_ADDRESS` for the published database, Engine, and frontend ports.
Changing it to a LAN address exposes all three, not just the browser entrance. Keep internal ports
private and verify listener/firewall behavior; dedicated classroom ingress configuration is pending.

### Optional persistence warning tuning

The following variables are optional and only affect diagnostics tuning for high-throughput event classes:

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED` (default: `true`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT` (default: `80`)
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT` (default: `40`)
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS` (default: `10000`)

## Pre/Post-Class Checklist

Before class:

- Update dependencies and run `npm audit`, `cargo audit`.
- Verify random secrets and loopback binding.
- Snapshot or backup lab VMs.
- Run through all labs in dry-run mode.
- Ensure host is not running sensitive production workloads.

After class:

- Detach all eBPF attachments.
- Stop the lab stack.
- Clean temporary accounts/credentials.
- Check for stale files under bpffs.
- Prefer destroying and recreating VMs in shared environments instead of reusing unknown states.

## Dependency Audit Policy

- Frontend dependency floors are Next.js 15.5.24, sharp 0.35.5 and source-map-js 1.2.2. These retain fixes for the upstream
  [Windows server](https://github.com/vercel/next.js/security/advisories/GHSA-p293-qw3h-jr36),
  [AVIF optimization](https://github.com/vercel/next.js/security/advisories/GHSA-2xp9-vwfh-vxw4) and
  [sharp/libheif](https://github.com/lovell/sharp/security/advisories/GHSA-rgj7-g3m4-5g8c) advisories.
  The sharp update also addresses the [bundled librsvg issue](https://github.com/advisories/GHSA-wq5f-xc86-pv6w),
  and source-map-js addresses [indexed-map amplification](https://github.com/advisories/GHSA-68fv-2mgg-jv7q).
  Offline regression checks require the patched sharp/source-map-js overrides and all locked copies,
  including cross-platform sharp 0.35.5/libvips 1.3.4 packages; live audit remains a separate check.
  Rebuild and redeploy to apply updated dependencies; editing the lockfile does not update a running
  container. A custom global libheif must also be at least 1.23.2; an npm audit is not host-library acceptance.
  The 2026-10-07 repaired candidate's production audit has no moderate/high/critical findings, but retains
  one low DOMPurify finding. This is not a claim that all dependencies or deployed libraries are safe.
- We run `cargo audit` for Rust backend dependencies and track accepted exceptions in
  `scripts/security-audit-exceptions.json`.
- The locked rustls dependency is at least 0.23.45, addressing the TLS 1.3 handshake encryption-level
  validation issue [RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html).
  Rebuild and redeploy Engine and Runner Agent binaries/images; changing the lockfile alone does not
  patch a running process. Offline preflight checks this known version floor; live audit checks for
  additional advisories.
- There are currently no accepted advisory exceptions. The former `rsa`/`sqlx-mysql` dependency
  chain is no longer in the lockfile.
- Each accepted advisory has a review deadline and must be re-evaluated before it expires.

## Security Incident Response

If you suspect credential leakage or unknown programs:

1. Immediately disconnect lab node network.
2. Stop Engine.
3. Do not keep using the same password after only an in-app change.
4. Export required logs and attachment details.
5. Destroy single-use VM and rebuild.
6. Rotate DB, admin, TOTP, and SSH credentials.
7. Check host workloads for secondary impact.
