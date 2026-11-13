# Federated nodes and discovery

[简体中文](../zh-CN/federation-discovery.md)

Direction accepted **2026-11-13**, [proposal #4](https://github.com/Team-silvortex/cyanrex-lab/issues/4).
Cyanrex is moving toward a decentralized collaboration and teaching platform: people and organizations
operate their own nodes and workspaces; code remains one optional Task payload. Teaching authority
stays local to its teaching domain, not a global owner of peers or their private resources.

**F0 and F1a in 0.6.6 are pure Rust preparation modules**, not a running federation service.
F1a adds compact TXT data handling and a bounded local candidate cache; product version is 0.6.7.
The separate F1b adapter adds an explicitly called Linux IPv4 single-query browser;
it is not mounted in either application. Device pairing, traversal, relay transport and cross-authority
workspace access are subsequent slices.
Included in source 0.6.7, F1c adds a separate explicit `cyanrex-discovery` command, not startup integration.
F1d adds pure announcement/goodbye wire preparation, not a running announcer.
F1e adds pure probe bytes and conservative per-packet claim observation, not name availability.
F1f adds a separate explicit exact-local unicast receiver, not a running probe controller.
F1g adds an explicit bounded three-probe library attempt, not an announcer or ownership permit.
F1h adds finite caller-driven retries with frozen lifetime/interface and conflict cooldown, not publication.
F1i adds a separate original-socket quiet continuation with bounded receive-only review, not announcements.
Older `Unreleased` labels below preserve implementation stages and dated evidence, not pending source inclusion.

## Trust and responsibility

The intended connection sequence keeps four separate decisions:

```text
Untrusted discovery hint → protocol/capability selection → node-key proof + explicit pairing
                                                        → separately scoped WorkspaceGrant
```

Discovery can report a compatible candidate without making it trustworthy. A fingerprint in a message
is a claim, not proof that the sender possesses the key. Pairing must eventually prove that identity
and record approval and revocation. An independently reviewed `WorkspaceGrant` must still authorize
each permitted resource operation; neither a connection nor a paired node implies membership.

`AuthorityId` identifies an existing resource namespace, not a certificate, network endpoint or node
key. A foreign claimed authority in F0 never bypasses the current Source's same-authority checks.
Existing classroom invites, Cookie Sessions, private Tasks and Runner bootstrap credentials keep
their present meaning. Runner registration is not peer pairing; classroom links are not mDNS.

## F0 contract

Implementation: `engine/src/services/federation_discovery/`. The caller supplies bytes, local policy
and an explicit current time. There is no clock lookup, network, database, filesystem or key generation.

| Value | Meaning |
|---|---|
| `NodeFingerprint` | Canonical 64-lowercase-hex claimed node-key digest; independent of `AuthorityId` |
| `PeerAdvertisement` | Strict bounded schema-1 metadata, decoded through `parse` and serialized through `encode` |
| `ProtocolRange` | Positive inclusive local or advertised version range |
| `DiscoveryPolicy` | Local fingerprint, versions, supported/required capabilities and explicit hint opt-ins |
| `UnverifiedPeerCandidate` | Selected protocol, mutual capabilities and allowed hints; no authority or dispatch method |
| `ConnectionHint` | Canonical IP socket literal or claimed relay name; only selected candidates passed local policy |

The wire fields are `schema_version`, `node_fingerprint`, `authority_id`, `protocol: {min, max}`,
`offered_capabilities`, `required_capabilities`, `issued_at`, `expires_at` and `hints`.
Each hint is either `{kind: "direct", address: "192.168.1.7:4433"}` or
`{kind: "relay", relay_id: "lab.home"}`. Selection retains only operator-approved relay identifiers;
parsing alone approves none and never imports a relay URL. Advertisements carry no workspace names, member lists,
content, credentials or invitation secrets. Unknown and duplicate fields are rejected at every level.

| Bound | F0 rule |
|---|---|
| Encoded announcement | At most 4,096 bytes, including whitespace |
| Connection hints | 1–8 distinct hints; no port zero, DNS, URL, scope/zone or mapped IPv6 |
| Capabilities | Each list at most 16 canonical distinct qualified names; required must be offered |
| Lease | Positive JSON-safe whole Unix seconds, duration 1–300 seconds |
| Freshness | Expiry strictly after caller time; issue time at most 5 seconds ahead |
| Version agreement | Highest intersection; never below the local minimum |
| Capability agreement | Both sides' requirements must be supported; output contains only mutual capabilities |

Parsing and selection reject invalid values, self announcements and incompatible peers with fixed
input-free errors. Freshness is rechecked on selection. A short unsigned lease is not replay protection,
authenticated liveness or a durable registration lease.

F2 must define canonical public-key encoding and fingerprint derivation before promoting a claim
to verified identity; F0 neither generates a key nor verifies a digest against one.

`DiscoveryPolicy::new` permits only private direct hints by default. `with_public_direct` explicitly
allows public-address candidates; `with_relays` explicitly allows bounded operator-selected names.
Selection orders private direct hints before public candidates, then approved relay names; order
inside each group is stable. An empty allowed set fails rather than silently changing policy.

The static address filter accepts RFC1918 IPv4 and IPv6 ULA as private hints and rejects loopback,
unspecified, multicast, link-local/metadata, documentation and other excluded special-use ranges.
The optional public class is a conservative literal-address candidate, not proof of assignment,
same-LAN membership, reachability or a complete SSRF defense. A later transport must bind an approved
interface/egress policy and the proven node key before using any hint.

## F1a LAN data ingestion

`federation_discovery::lan` composes the existing F0 parser/selector without adding sockets or a
discovery dependency. `encode_txt` and `decode_txt` handle **one raw DNS-SD TXT RDATA**, not a DNS
packet, PTR/SRV/address resolution, or multiple-record assembly. `MAX_TXT_RECORD_BYTES` is **1,200**;
the envelope uses `v=1`, `c=1..6` and `d0` through `d5` with at most **200** data bytes per chunk.
All non-final chunks have exactly 200 bytes. Fields may arrive in any order, keys are ASCII
case-insensitive, and duplicate/unknown/missing fields fail. Length prefixes include every field;
truncated records and empty chunks fail. Reassembled JSON still passes the complete F0 parser.
An otherwise valid F0 announcement too large for one record fails instead of losing fields or
being split across records. These are application-profile limits, not new limits on F0.

`LanCandidateCache` fixes the local policy, **1–8** approved interface incarnations and **1–64**
entries at construction. `LanSource` identifies a local `interface_epoch` plus an instance alias
of **1–63** lowercase ASCII letters/digits/hyphens, with alphanumeric ends. An epoch is a trusted
adapter-supplied incarnation, not an advertised ID, stable OS ifindex or interface verification.
Aliases are source labels, never identity; the full 64-hex fingerprint cannot fit in one DNS label.

| Cache event | Behavior |
|---|---|
| Same claim from different sources | Keep separate; never merge by fingerprint or authority |
| Exact canonical duplicate at one live source | Report Duplicate without extending either deadline |
| Changed announcement at a live source | SourceConflict; preserve the original entry |
| Full cache | Capacity; do not silently evict a live entry |
| Removal | Remove only the exact local epoch/alias; this is not peer or grant revocation |
| Expiry | Remove at the earlier initial record TTL, F0 expiry or corresponding monotonic deadline |
| Clock rollback | Clear and close permanently with ClockRollback; later operations return Closed |
| Stop | Clear and close; no restart or restoration of that cache |

The caller supplies `CacheTime` (wall and monotonic whole seconds); no actual clock is read.
Every valid time sample advances the watermark, even if admission later fails; overflow cannot undo
observed time or reset the event budget by returning to an older sample.
Record TTL must be **1–120 seconds**; a future adapter must use the minimum relevant record TTL
and map TTL-zero goodbyes to exact-source removal. Observe/remove share **64 events per monotonic
second**, including invalid announcements and absent-source removals; snapshot reads do not consume
that budget. This fixed-window admission limit is not network-ingress, CPU or bandwidth protection.
Snapshots recheck F0 compatibility and both deadlines and return only `UnverifiedPeerCandidate`
values with their source/deadline. A snapshot is not continued validity or permission to connect.

The actual backend must constrain interfaces **before** opening sockets or joining multicast,
bind received records/removals to their verified local source, limit packet parsing/queues and
wait for owned workers to stop. Interface changes or event-stream failure must close its cache.
`mdns-sd` was inspected but not added: its current constructor starts before interface filtering,
so post-start disable/enable is not an atomic pre-start allowlist ([official source](https://docs.rs/mdns-sd/0.21.7/src/mdns_sd/service_daemon.rs.html)).
F1a neither constructs that daemon nor claims live LAN discovery acceptance.

## F1b explicit Linux IPv4 browser

`engine/src/services/federation_mdns/` is separate from the pure F0/F1a modules.
`discover_once(config, policy, cancelled)` owns one socket for one fixed PTR query to the experimental
`_cyanrex._udp.local.` profile and a **1 ms–5 second** receive window. The wait may finish up to one
bounded receive interval later; it is not a real-time execution guarantee. It creates no thread,
registers no local service and connects to no discovered business port. Another call is another
explicit operation, not a scheduler or automatic retry. F1c provides a separate CLI, not UI or host-startup mounting.

`MdnsConfig` requires an exact interface name and RFC1918 local IPv4 address. Before socket creation,
the adapter checks the network-namespace identity, interface index, address, contiguous `/1..30`
netmask, UP/MULTICAST flags and non-loopback host address. Changes fail closed without selecting a
default interface. These are local metadata snapshots, not a proof of physical LAN or atomic immunity
to administrator changes. VPN interfaces may satisfy the same checks.

The socket binds **224.0.0.251:5353**, not wildcard or business ingress. Before joining only the
selected interface it sets `IP_MULTICAST_ALL=0`, `IP_PKTINFO`, `IP_RECVTTL`, the exact multicast output
interface and a hop limit of 255. Every send uses the same pinned interface/source again; arbitrary
payloads are rejected by `send_query`. Receive requires the selected index/local address, destination
group, source port 5353, private same-subnet non-self host and TTL 255; malformed, missing, duplicate
or truncated ancillary data fail. No capability or gateway changes are performed by the adapter.

This strict profile can miss legitimate same-link devices on another subnet. It handles only one
complete PTR → SRV/TXT → IPv4 A chain per instance **within one packet**, standard backward name
compression and bounded unknown records. It does not provide generic cross-packet DNS caching,
IPv6, SRV/A follow-up queries, continuous browsing/known-answer suppression or interoperability
acceptance against arbitrary mDNS implementations. Packets are at most **4,096 bytes**, at most
**64 records / 8 events**; only selected F0 direct hints matching the datagram sender, A and SRV
port can enter the cache. Same-alias goodbyes must also come from that admitted sender.

Before parsing, the receive loop admits at most **64 datagrams per monotonic second / 256 per run**;
invalid datagrams also spend the budget. The cache has its own event limit. Budget exhaustion,
interface change, socket/clock failure or cancellation returns an error, not a partial-success list.
An already-set cancellation flag prevents interface/socket work. In-flight cancellation is checked
around each at-most-50 ms receive and bounded parsing. Every return drops the owned descriptor and
clears the cache; returned snapshots remain unverified, time-limited metadata, never live authority.

Explicit isolated testing uses `scripts/test-federation-mdns-netns.sh`, not the host LAN. It builds
locked/offline and creates anonymous user/network namespaces and disposable virtual links, with no
host attachment or default route. Ordinary UDP cases run with zero effective capabilities; only the
separate interface-removal fixture has authority to alter its own disposable namespace. Lack of
namespace support is a failed precondition, never permission to fall back to the host network.
This is a Linux adapter fixture, not physical-LAN, Android, pairing or decentralized-deployment acceptance.

## F1c explicit discovery command

Build the Linux source tool included in 0.6.7 with
`cargo build --manifest-path engine/Cargo.toml --locked --bin cyanrex-discovery`, then inspect
`engine/target/debug/cyanrex-discovery --help`. It is not installed into the privileged Engine image
or invoked by either application's startup. F1c does not change existing F0/F1a or F1b behavior.

All policy values are mandatory in fixed order. This synthetic example must not be run unchanged:
use your exact non-root numeric identity/groups, reviewed interface/address and local compatibility
policy. Execution sends one multicast PTR query, not a service announcement. Do not use sudo.

```bash
engine/target/debug/cyanrex-discovery browse-lan \
  --uid 1000 --gid 1000 --groups 27,1000 \
  --interface enp3s0 --address 192.168.1.10 \
  --local-node aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --protocol 1:1 --timeout-ms 1000 --capabilities none --require none
```

The local fingerprint is a supplied exclusion claim, not a generated/proven key. Protocol is a
positive inclusive discovery range, not the product version. Capability CSVs have at most 16
distinct qualified names; `none` means an empty list and required names must also be supported.
These inputs do not implement a negotiated business capability. Input is exactly 21 arguments,
each at most 2,048 UTF-8 bytes and at most 16,384 bytes in total. Aliases, reordering, duplicates,
unknown flags and environment defaults fail. There are no public/relay, config-file, watch or join options.

Before interface/socket work, `prepare_platform_process` requires exact non-root identity/groups,
zero current capabilities and this process's no-new-privileges restriction. The command does not
drop privileges, read NSS/authentication configuration or access a database. It rechecks the calling
thread after discovery and before output. No Session, membership or resource authority is created.

Success emits complete buffered JSON plus newline, at most **256 KiB / 64 candidates**. The top
level has `schema_version: 1`, `kind: "unverified_lan_snapshot"`, `snapshot_only: true`, `verified: false`
and `candidates`. Items contain an instance, separate claimed node/authority, negotiated protocol,
compatible capabilities and inert IPv4 hints. `advertisement_expires_at` is only the F0 declaration,
not the shorter DNS TTL, liveness, reusable lease or connection permission. Internal epochs/monotonic
deadlines, raw packets/TXT/JSON and transport errors are not exported. Empty success means only that
this window yielded no compatible candidate, not a healthy network or absence of devices.

Exit codes are **0** success, **64** usage, **77** unsafe process, **74** discovery/output failure and
**69** unsupported platform. Failure never becomes empty success. Consumers must require a complete
document and successful exit: signal/write failure can leave truncated stdout. The **1–5,000 ms**
limit covers the network window, not setup, parsing or stdout backpressure. The single-process CLI
installs no worker, child or signal handler and preserves inherited signal dispositions; normal
SIGINT/SIGTERM defaults terminate it and close descriptors ([signal reference](https://man7.org/linux/man-pages/man7/signal.7.html)).

Default tests never discover on the host LAN. Real CLI tests explicitly use
`scripts/test-federation-mdns-netns.sh --with-cli`, with existing `getsubids`/`newuidmap`/`newgidmap`
authorization. Only one already-authorized subordinate UID/GID is mapped in a fresh user namespace;
the child's groups are cleared there before a nested nonzero, capability-free CLI runs in the
disposable network. Missing authorization fails; accounts are not edited and there is no host fallback
([unshare reference](https://man7.org/linux/man-pages/man1/unshare.1.html)).

## F1d pure announcement preparation

`federation_mdns::MdnsAnnouncement` freezes one supplied F0 declaration, instance label, host label,
IPv4 endpoint and requested TTL. Construction and `encode_at(now)` perform no I/O or clock lookup.
The two labels must independently be canonical lowercase ASCII letters/digits/hyphens, 1–63 bytes,
with alphanumeric ends; no suffix, normalization or use of a 64-character fingerprint as one label.
The endpoint must be the declaration's **only** hint: the same nonzero-port RFC1918 IPv4 socket.
Mixed, additional, public, relay or IPv6 hints fail as a whole rather than being silently filtered.
This checks claim consistency, not local interface ownership, name uniqueness or service availability.

Local publication is stricter than F0 reception: the supplied time must satisfy
`issued_at <= now < expires_at`, with a positive JSON-safe Unix second. Requested TTL is **1–120**;
all four record TTLs are `min(requested_ttl, expires_at - now)`. Each `encode_at` rechecks expiry
and rejects time before construction. It does not track successive samples: a later sender still
needs monotonic/rollback controls and fresh interface/time checks immediately before each send.
Neither encoding nor a retained packet refreshes F0 times or becomes a reusable sending lease.

The uncompressed packet has ID zero, flags `0x8400`, zero questions and **four Answer records**:
PTR to `<instance>._cyanrex._udp.local.`, SRV/TXT at that instance, and one A at `<host>.local.`.
PTR is shared IN; SRV/TXT/A carry cache-flush IN. These are only prepared wire fields, not evidence
that the uniqueness checks have run. SRV priority/weight are zero. The full original F0 passes the
existing **1,200-byte** TXT envelope; every packet append is bounded before copying to **4,096 bytes**.
Oversize fails without truncation. This packet cap is **not** a usable UDP payload/MTU guarantee.

`encode_goodbye()` preserves the same frozen owner/type/class/RDATA and order, changing only all
TTLs to zero, including after declaration expiry. It neither proves a previous announcement nor
authorizes removing another node. The current browser maps this to immediate exact-source candidate
removal; it does not implement the general mDNS one-second goodbye grace period.

There is no announce command, probe/conflict/rename state machine, query-response suppression,
schedule, sender, service registration or application startup change. `MdnsSocket::send_query` still
rejects every payload other than the fixed browse query. Actual publication must separately implement
name probing/conflict handling and its lifecycle before transmitting these unique-record assertions
([RFC 6762 §§8–10](https://www.rfc-editor.org/rfc/rfc6762.html#section-8)).
Default pure tests independently inspect fields and combine the bytes with the existing parser/cache;
that is internal-profile compatibility, not Avahi/Bonjour, physical-LAN or decentralized deployment acceptance.

## F1e probe preparation and conservative observation

`MdnsAnnouncement::encode_probe_at(now)` reuses F1d's frozen names, SRV/TXT/A data, time checks and
packet budget. It emits ID/flags zero, **two ANY/QU questions**, zero Answer/Additional records and
**three Authority records**: SRV/TXT at the instance, A at the host. Authority class is IN, without
cache-flush; shared PTR is not proposed for uniqueness. This is a prepared proposal, not a sent probe.

`observe_probe_packet(bytes)` uses a shared private DNS framing decoder, not `parse_response` or
candidate compatibility. It consumes the complete packet before returning a fixed `ProbeObservation`:

| Observation | Exact conservative meaning |
|---|---|
| `ExistingResponse` | An ID-zero response contains any IN record at either protected name, in any RR section |
| `CompetingProbe` | A query's related IN Authority record matches a same-name IN question of ANY or that record's type |
| `NoRelevantClaim` | This one decoded packet contains neither kind of relevant claim; **not** an available/owned name |

Even identical RDATA, unknown record types and TTL-zero responses produce `ExistingResponse`.
Their mere presence blocks an assumption of absence; this is not semantic RDATA validation,
authentication, an active-owner claim or a DNS tiebreak result. Related query Authority records
without a matching question fail. Query Answer known-answer caches and Additional records never
become simultaneous-probe authority; questions alone are not ownership claims.

The decoder retains RR sections and raw QU/cache-flush class bits. Existing backward-compression,
ASCII case-folded label and **4,096-byte / 8-question / 64-record** bounds remain; opaque RDATA is
framed, not interpreted for this observation. Truncation, trailing data, unsupported headers or bounds
fail rather than return `NoRelevantClaim`. The old response-only candidate parser keeps its original
header admission and complete-chain rules; a missing PTR chain cannot hide a claim from this observer.

There is no three-probe timer, initial random delay, retry/backoff, name change or winner/permit API.
The caller of any later live adapter must account for ingress provenance, every relevant response,
clock/expiry and stop behavior. RFC probe timing and conflict resolution remain requirements before
publication ([RFC 6762 §8](https://www.rfc-editor.org/rfc/rfc6762.html#section-8.1)). Crucially, the
current browse socket binds the multicast destination and rejects unicast: it cannot receive all QU
defensive replies and must **not** establish probing success from its timeout. F1f below separately
covers restricted unicast reception/port coexistence; F1g adds a separate bounded attempt below,
not independent interoperability. Neither the CLI nor socket gains a probe send path in F1e.

## F1f explicit receive-only unicast channel

Linux `MdnsUnicastReceiver::open(&VerifiedInterface)` opens a separate receive-only descriptor on
the selected **local RFC1918 IPv4 address:5353**. It does not replace or broaden the existing
multicast socket. It sets neither port-reuse option, joins no multicast group and exposes no send,
connect or raw-descriptor API. An existing same-address or wildcard listener makes opening fail
with `SocketUnavailable`, even if that listener enables reuse. There is no wildcard bind, takeover,
daemon reconfiguration or multicast-only fallback. Distinct exact local addresses and the existing
exact-group socket can coexist; that restricted behavior is not coexistence with every system
mDNS daemon ([RFC 6762 §15.1](https://www.rfc-editor.org/rfc/rfc6762.html#section-15.1)).

The two channels share bounded `recvmsg` framing and unique ancillary validation, but **not** a
union of destinations: the browser still requires 224.0.0.251, the new receiver requires its exact
local address. Both also require selected ingress index, selected `ipi_spec_dst`, TTL 255 and a
non-self RFC1918 same-subnet host on source port 5353. Empty/oversized/truncated packets, missing or
duplicate controls and changed interface/netns/address observations fail. Interface snapshots are
rechecked around open/receive, not an atomic guarantee against administrator changes. Dropping the
receiver releases its descriptor; no worker survives it.

`receive()` waits at most the existing 50 ms socket-timeout interval, subject to scheduling. This is
not a whole-flow deadline, packet-rate quota or admission controller. Returned bytes are not matched
to a sent QU question and do not prove identity, availability or name ownership. The type is not
mounted into `discover_once`, the CLI or either application's startup. F1g separately composes it;
continuous conflict/publication lifecycle and independent interoperability remain separate work.

The opt-in `scripts/test-federation-mdns-netns.sh --with-unicast` adds ten ignored-by-default cases
inside the same two disconnected owned namespaces; combine `--with-cli --with-unicast` to retain
all original discovery/CLI cases too. Coverage includes both open orders, exact-local interfaces,
port collisions/reuse/dual-stack wildcard refusal, malformed datagrams, descriptor/port release,
zero UDP sends/TCP opens and address removal. Synthetic announcement/probe bytes come only from the
owned test peer; no product probe is sent and no real LAN or system service is touched.

## F1g explicit one-attempt probing

`probe_once(config, announcement, cancelled)` is a separate Linux library entry, not a new CLI flag
or startup service. It requires a frozen announcement matching the selected local IPv4 and a
**1–2 second** whole-attempt timeout; prefer two seconds. Setup and scheduling count against it:
a one-second configuration can fail even on a quiet network. Both original exact-group and separate
exclusive exact-local sockets must open before any send; an occupied response port fails with zero
probes, without fallback or system-daemon reconfiguration. Every return drops both descriptors.

Nonblocking kernel entropy chooses a discrete uniform 0–250 ms initial wait, using bounded rejection
sampling with no weak fallback. Three proposals are spaced from actual successful send completion,
with at least 250 ms between sends and another 250 ms after the third; full-precision checks prevent
millisecond rounding from shortening either interval. Late scheduling never causes catch-up sends.
This follows the probe timing target, not a real-time guarantee ([RFC 6762 §8.1](https://www.rfc-editor.org/rfc/rfc6762.html#section-8.1)).

Private two-FD polling rotates first service, consumes at most one packet per channel per batch,
and shares **64 datagrams/monotonic second / 256 per attempt** before parsing, including rejected
packets. Waits are capped by the next timer, remaining timeout/initial lease and 50 ms. Send and final
boundaries drain both queues under the same budget; interrupted/faulted polling is an error, not an
empty queue. Cancellation, interface/netns snapshots, full wall-clock watermark and frozen monotonic
F0 expiry are checked around work. These are adjacent observations, not atomic protection from
administrator changes or cancellation between syscalls.

Packets consumed before the first send are not replies to this attempt. Afterwards, the strict F1e
observer stops on a relevant response or multicast competing proposal; unicast must be a response
matching the round's protected ANY/QU names. The complete attempt is at most two seconds. Rejected
post-send DNS/provenance packets terminate instead of becoming absence. Without kernel arrival
timestamps, a packet queued across the first-send boundary can conservatively cause a false conflict.
This restricted policy is not complete RFC header acceptance, dictionary tiebreak or arbitrary mDNS interoperability.

Only a private typed sender can encode the current proposal. It checks payload + 28 bytes against
the selected interface's current IPv4 MTU and enables kernel do-not-fragment policy; oversized
multi-record packets fail without sending or splitting ([RFC 6762 §17](https://www.rfc-editor.org/rfc/rfc6762.html#section-17)).
The public browser still sends only its fixed PTR query; the public unicast type stays receive-only.

`ProbeRunObservation` is only `NoConflictObserved`, `ExistingResponse` or `CompetingProbe`.
Even completed silence is not availability, identity, continued uniqueness or permission to publish.
No automatic retry, rename, backoff, announcement/goodbye, continuous defense, key proof or grant is
added by this entry. The separate F1h controller below adds bounded conflict cooldown, not publication;
independent calls do not provide a process-wide rate limit. Explicit `--with-probe` in the guarded
namespace test runner is test selection only, not a product command or physical-LAN acceptance.

## F1h finite explicit retry controller

`MdnsProbeController::new(config, announcement)` is pure preparation: it validates the frozen source
and 1–2 second per-attempt bound without clocks, entropy, interface lookup or sockets. The caller
explicitly invokes `attempt(cancelled)`; no thread, automatic retry, sleep or CLI/startup integration
is added. Each successful admission counts toward **at most four attempts**, including subsequent
interface/socket failures. Pre-cancellation, initial expiry and early cooldown rejection do not count.

Only `ExistingResponse` or `CompetingProbe` permits a later attempt, at least **five seconds after
completion**, not after the previous start. `CoolingDown` leaves the count and deadline unchanged,
checks the frozen lifetime but returns before interface, entropy or socket work. This conservative
constant wait follows the permitted always-delay simplification in
[RFC 6762 §8.1](https://www.rfc-editor.org/rfc/rfc6762.html#section-8.1); no lexicographic winner is inferred.
`NoConflictObserved` finishes permanently. Other errors, including cancellation, close permanently;
replacing a cancellation flag cannot revive the controller. The fourth conflict closes without a fifth try.

First actual use fixes a **30-second** monotonic whole-controller limit, absolute F0 expiry, a
full-precision monotonic lease and wall/elapsed watermarks. The first verified interface is pinned;
later attempts neither renew claims nor adopt a renamed/replaced interface. A private callback
rechecks these original bounds around active F1g I/O, not merely at attempt entry/return. These remain
adjacent pre/post observations, not real-time preemption: a bounded poll/syscall can cross a deadline
before the next check rejects its result. Every attempt retains F1g's own timeout and ingress budget;
the 256-packet allowance is per attempt, not one shared allowance across all four attempts.

`state()` is the last observed `Ready`/`CoolingDown`/`Finished`/`Closed` state, not idle expiry
monitoring. `retry_after()` is only a scheduling hint; `Some(Duration::ZERO)` grants nothing and
the next explicit call must pass fresh checks. Every attempt drops both sockets; cooldown and
finished states retain no FD, listener or sender. Drop sends no goodbye because nothing was announced.
Closing/reopening after quiet observation cannot safely bridge into publication; the copyable result
is not a permit. Announcements, continuous defense and withdrawal remain a separate retained-I/O design.

This limit belongs only to this object. Creating another controller or directly calling `probe_once`
bypasses its history; it is **not a process/host/global rate limiter**. No renaming, authentication,
pairing or grant is added. `--with-controller` selects isolated acceptance tests only.

## F1i retained quiet continuation

`probe_retained_once(config, announcement, cancelled)` is a separate explicit one-attempt entry.
It borrows the frozen announcement and uses the same three-probe runtime, not a supplied observation
or a repeated bind. `RetainedProbeOutcome` is `ExistingResponse`, `CompetingProbe` or
`Quiet(MdnsRetainedProbe)`. Only the quiet branch moves the **original two sockets**, their queues,
original start, timeout, lease, clock watermarks and spent ingress counters into the opaque handle.
The original interface snapshot is copied without rediscovery; no socket is closed/reopened at handoff.

The handle is non-`Clone`, with no public constructor, FD export, reconfiguration, renewal or sender.
`review(cancelled)` drains both existing channels nonblockingly, using post-send classification and
the same budgets. A fresh final check is required even after an empty poll. Quiet review is only
historical `NoConflictObserved`; any relevant claim, malformed/provenance-invalid input, cancellation,
interface or time failure permanently closes and drops both channels. Further review returns `Closed`.
`close()` is idempotent local release. Drop sends no fourth probe, announcement or goodbye.

A fixed **250 ms review window** begins at quiet completion; this is an application bound, not an
RFC ownership period. Repeated review never renews it. The original 1–2 second whole-attempt timeout,
full-precision F0 lease/absolute expiry and ingress limits remain active and may expire sooner.
Cancellation is sampled on explicit operations, not watched in the background. An idle expired
handle still owns its descriptors until review, close or Drop; retaining FDs does not retain validity.
Checks are adjacent observations, not real-time preemption or atomic protection against administrator changes.

Existing `probe_once` and F1h `attempt` still drop all sockets before returning; the local F1h guard
does **not** escape into a retained handle. The new independent entry inherits no controller retry
history or 30-second bound. Neither a quiet observation nor that controller can manufacture this
context. This is not process-wide limiting, a publication permit, a complete responder or an
announcement/defense/withdrawal lifecycle. Those require a separately bounded retained-I/O composition;
this short window cannot serve as an indefinite lease. No CLI/startup/pairing/grant is added.
`--with-retained` selects guarded isolated tests only, not a product networking option.

## Delivery stages

| Stage | Scope | State |
|---|---|---|
| F0 | Bounded announcements, compatibility and policy-filtered unverified hints | Pure preparation module; not mounted |
| F1 | Opt-in LAN mDNS/DNS-SD adapter, bounded candidate cache, expiry and interface controls | Source 0.6.7 includes F1a TXT/cache, F1b Linux single query, F1c CLI/F1d–e preparation/F1f unicast/F1g probing/F1h finite retries/F1i retained quiet review; publication lifecycle, broader interop and UI remain |
| F2 | Persistent node key, challenge proof, explicit invitation/approval, revocation and replay defense | Planned; no paired-node registry yet |
| F3 | Native direct connection and traversal, explicitly configured self-hosted encrypted relay fallback | Planned; no transport dependency selected |
| F4 | Narrow cross-authority workspace grants and separately verified resource commands | Planned; current same-authority rejection remains |
| F5 | Multiple-node/offline/relay failure acceptance, Android/browser bridging and operational controls | Planned; no decentralized deployment acceptance |

LAN discovery is local-link discovery, not authentication; see [RFC 6762](https://www.rfc-editor.org/rfc/rfc6762.html#section-21).
Cross-network direct connection cannot be guaranteed: [STUN alone is not a traversal solution](https://www.rfc-editor.org/rfc/rfc8489.html),
and some networks require an intermediate relay ([TURN](https://www.rfc-editor.org/rfc/rfc8656.html)).
F0/F1a remain pure; F1b is the restricted one-query profile above, not full mDNS or a traversal implementation.

The transport target is LAN first, cross-network direct when possible, then operator-configured
replaceable relays. No unique official directory, account service or relay is required by the design.
Unavailable paths must remain visibly offline. Encryption must reach the proven peer, not just the
relay; connection metadata can still be visible. Automatic UPnP/PCP/NAT-PMP port mapping, gateway
changes and public listeners are not implicit consent. Browser/Android connectivity needs its own
adapter and certificate acceptance, not an assumption that native UDP APIs are available.

## Validation and unchanged boundaries

`engine/tests/federation_discovery_tdd.rs` and its child modules cover strict wire decoding, version
and capability negotiation, lease boundaries, self claims, address classes, opt-ins and static errors.
`engine/tests/federation_lan_tdd.rs` covers source/cache/time/admission behavior;
`engine/tests/federation_lan_txt_tdd.rs` covers one-record framing and TXT → F0 → cache composition.
`scripts/tests/federationDiscovery.test.mjs` recursively guards the pure/unmounted source boundary
and paired documentation. These tests do not perform live discovery, network punching or authentication.
The separate `federation_mdns_*_tdd` targets cover the adapter and opt-in isolated UDP boundary;
the default run ignores the explicitly guarded network fixtures rather than touching the host LAN.

The **2026-11-13 Unreleased F1b scoped run** passed **49** adapter default cases (13 private units
and 36 integration cases), **105** selected existing F0/F1a/collaboration cases, and separately all
**7** real UDP cases in two isolated network namespaces. The common/format gate passed **884**
script checks plus its tooling and formatting checks. A failing goodbye-budget regression was added
before correcting its terminal error classification; rejection was not relaxed. These counts are
not a full backend/frontend/SDK gate or remote CI result; no physical LAN, pairing or traversal was
exercised. The runner and cleanup boundaries are documented in the [testing guide](testing-guide.md).

The **2026-11-13 Unreleased F1c scoped run** passed **40** new default CLI cases and **161** selected
existing discovery/process/collaboration cases (**201** total). Explicit namespace acceptance passed
**6** new CLI cases alongside the original **7** UDP cases; the zero-argument runner separately
repassed those same 7, not additional coverage. The common/format gate passed **885** script checks
plus tooling and formatting. Initial real CLI failures occurred before querying because of the
fixture's identity mapping; only that mapping was corrected, not the product process gate. Broken
help/diagnostic pipes also received failing regressions before the fixed-exit-code correction.
This is scoped local evidence, not a full backend/frontend/SDK gate, physical LAN or new remote CI result.

The **2026-11-13 Unreleased F1d scoped run** passed **30** new pure Rust cases (27 integration,
3 private writer units), with **201** selected existing cases: **231** total, no failures or ignored
cases. The existing **13** isolated discovery/CLI network cases also passed again; they did not
send F1d packets or establish announcement acceptance. The common/format gate passed **886** script
checks plus tooling/formatting. Missing-API/source guards first failed, and the packet writer's exact
budget tests failed before implementation. No full backend/frontend/SDK gate or new remote CI result
is claimed for this slice; announcement sending, name-conflict lifecycle and independent interop remain open.

The **2026-11-13 Unreleased F1e scoped run** passed **46** new pure Rust cases (44 probe integration,
2 private framing units) plus **231** selected existing cases: **277** total, no failures/ignored cases
or warnings. The old **13** isolated discovery/CLI cases regressed successfully after the decoder
extraction; no new probe or QU unicast was sent. The common/format gate passed **887** script checks
plus tooling/formatting. Missing-API/source guards first failed. Initial redundant QU/flush fields
produced two dead-code warnings; storing raw class values removed them without lint suppression.
This is not a full backend/frontend/SDK gate, new remote CI result, live probing or availability proof.

The **2026-11-13 Unreleased F1f scoped run** passed the new private destination/provenance unit
with the preceding **277** selected Rust cases: **278** total, no failures/ignored cases or warnings.
All **10** new real unicast cases passed alongside the original **13** discovery/CLI cases in
fresh disconnected namespaces (**23** distinct cases). The non-CLI `--with-unicast` selection also
passed the overlapping 17, and the unchanged zero-argument selection repassed the same original 7;
these reruns are not additional coverage. The default new target separately confirmed **10 ignored**
and no network execution. Missing-API, metadata and source-guard regressions failed before product
implementation; the common/format gate passed **888** script checks plus tooling/formatting.
No host LAN, application database, product probe, new remote CI or deployment was exercised.

The **2026-11-13 Unreleased F1g scoped run** passed **69** new private Rust cases (26 schedule,
32 limits, 9 classification/precision/preflight and 2 MTU/poll), plus the preceding **278** selected
cases: **347** total, no failures/ignored cases or warnings. **13** new real probe cases and the
original **23** discovery/CLI/unicast cases passed in fresh disconnected namespaces (**36** distinct
cases); `--with-probe` without CLI separately repassed the overlapping 20. The new target's default
run confirmed **13 ignored**, with no network execution. The common/format gate passed **889** script
checks plus tooling/formatting. Missing API, schedule/limits, sub-millisecond and interrupted-poll
regressions failed before their implementation. Review caught interrupted polling being treated as
an empty drain; the correction is hard failure, not retry or assertion relaxation. An initial new
source-guard regex was corrected for multiline formatting; the preceding eight guards stayed unchanged.
This is not full backend/frontend/SDK, physical-LAN, complete mDNS, announcement, pairing, deployment
or new remote CI acceptance. Test peers used only synthetic claims; no application database was used.

The **2026-11-13 Unreleased F1h scoped run** passed **58** new private Rust cases (37 retry gate,
14 fixed lifetime, 5 controller preflight and 2 internal guard ordering), alongside the preceding
**347** cases: **405** total, no failures/ignored cases or warnings. All **8** new controller network
cases and the original **36** passed together in fresh disconnected namespaces (**44** distinct);
`--with-controller` without CLI repassed the overlapping 15. Ordinary controller cases were
capability-free; only the separately guarded interface-rename case used owned-netns NET_ADMIN,
then the parent restored the original name before the unchanged address-removal cases. The new
target's default run confirmed **8 ignored**, no network. The common/format gate passed **890**
script checks plus tooling/formatting, all ten source guards and seven runner argument checks.
Missing API, gate/lifetime and cross-attempt active-guard regressions failed before implementation.
Review identified that entry/return checks alone missed original-lease expiry during a later attempt;
the private active-I/O guard now preserves that bound, without retrying or relaxing assertions.
This is per-object cooldown acceptance, not a full backend/frontend/SDK gate, process-wide rate
limiter, retained-I/O announcement lifecycle, physical LAN, pairing, deployment or new remote CI.

The **2026-11-13 Unreleased F1i scoped run** passed **38** new private Rust cases (15 review window,
16 original-bound handoff, 4 retained-entry preflight and 3 closed-handle/classification), plus the
preceding **405**: **443** total, no failures/ignored cases or warnings. All **11** new retained cases
and the original **44** passed together in fresh disconnected namespaces (**55** distinct);
`--with-retained` without CLI repassed the overlapping 18. Ten ordinary retained cases were
capability-free; one owned-netns NET_ADMIN rename case required closure and exact-name restoration
before original mutation cases. Queue occupancy, same socket inodes across reviews, exact traffic
counters and complete FD/port release were checked. The new target's default run confirmed **11
ignored**, no network. Common/format passed **891** script checks plus tooling/formatting; all eleven
source guards and eight runner argument checks passed. Missing-entry, window and original-bound
handoff regressions failed before implementation; the window Red/Green was tool output, not a saved
log. Review added a fresh final validity check after even an empty drain. A new source-guard regex
was corrected for multiline formatting; the previous ten stayed unchanged. This is short-lived
unannounced continuity, not controller handoff, publication, full backend/frontend/SDK, physical LAN,
authentication, deployment or new remote CI acceptance. No application database or host LAN was used.

The [architecture map](architecture-map.md), [platform network](platform-network.md) and
[security guide](security.md) retain the current host and authorization boundaries. F0 adds no route,
listener, configuration flag, schema, permission, migration, runtime dependency or deployed service;
the same limits apply to F1a. F1b, including an explicit F1c invocation, still opens only its requested
multicast socket; F1f's separate exact-local receiver requires an explicit library call. Existing
host routes, authentication, storage, dependencies and application startup remain unchanged;
F1g/F1h/F1i's separate explicit library calls add no caller authority or product startup integration.
