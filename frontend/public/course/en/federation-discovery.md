# Federated nodes and discovery

[简体中文](../zh-CN/federation-discovery.md)

Direction accepted **2026-11-13**, [proposal #4](https://github.com/Team-silvortex/cyanrex-lab/issues/4).
Cyanrex is moving toward a decentralized collaboration and teaching platform: people and organizations
operate their own nodes and workspaces; code remains one optional Task payload. Teaching authority
stays local to its teaching domain, not a global owner of peers or their private resources.

**F0 and F1a in 0.6.6 are pure Rust preparation modules**, not a running federation service.
F1a adds compact TXT data handling and a bounded local candidate cache; product version is 0.6.6.
The separate F1b adapter adds an explicitly called Linux IPv4 single-query browser;
it is not mounted in either application. Device pairing, traversal, relay transport and cross-authority
workspace access are subsequent slices.

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
explicit operation, not a scheduler or automatic retry. There is no CLI/UI or host-startup mounting.

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

## Delivery stages

| Stage | Scope | State |
|---|---|---|
| F0 | Bounded announcements, compatibility and policy-filtered unverified hints | Pure preparation module; not mounted |
| F1 | Opt-in LAN mDNS/DNS-SD adapter, bounded candidate cache, expiry and interface controls | F1a TXT/cache plus F1b explicit Linux single-query adapter; broader LAN/interop and user entry remain |
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

The [architecture map](architecture-map.md), [platform network](platform-network.md) and
[security guide](security.md) retain the current host and authorization boundaries. F0 adds no route,
listener, configuration flag, schema, permission, migration, runtime dependency or deployed service;
the same limits apply to F1a. F1b alone can open its explicitly requested multicast socket; existing
host routes, authentication, storage, dependencies and application startup remain unchanged.
