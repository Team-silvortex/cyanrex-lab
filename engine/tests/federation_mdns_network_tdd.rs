//! Explicit anonymous-network-namespace acceptance; never run these against the host LAN.
#![cfg(target_os = "linux")]
use cyanrex_engine::services::{
    federation_discovery::{lan::encode_txt, DiscoveryPolicy, PeerAdvertisement, ProtocolRange},
    federation_mdns::{
        browse_query, discover_once, MdnsConfig, MdnsError, MdnsSocket, VerifiedInterface,
        MDNS_GROUP, MDNS_PORT,
    },
};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::Ipv4Addr,
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn isolated(mutation: bool) {
    let parent =
        std::env::var("CYANREX_MDNS_TEST_PARENT_NETNS").expect("use the explicit netns runner");
    assert_ne!(
        fs::read_link("/proc/self/ns/net")
            .unwrap()
            .to_string_lossy(),
        parent
    );
    let interfaces = fs::read_to_string("/proc/self/net/dev").unwrap();
    let mut names: Vec<_> = interfaces
        .lines()
        .skip(2)
        .map(|line| line.split(':').next().unwrap().trim())
        .collect();
    names.sort();
    assert_eq!(names, vec!["crx0", "crx2", "lo"]);
    assert!(!fs::read_to_string("/proc/self/net/route")
        .unwrap()
        .lines()
        .skip(1)
        .any(|line| line.split_whitespace().nth(1) == Some("00000000")));
    let status = fs::read_to_string("/proc/self/status").unwrap();
    if !mutation {
        assert!(status
            .lines()
            .any(|line| line == "CapEff:\t0000000000000000"));
    } else {
        assert_eq!(std::env::var("CYANREX_MDNS_TEST_MUTATE").unwrap(), "1");
    }
}
fn config(name: &str, ip: &str, ms: u64) -> MdnsConfig {
    MdnsConfig::new(name, ip.parse().unwrap(), Duration::from_millis(ms)).unwrap()
}
fn browser(name: &str, ip: &str) -> MdnsSocket {
    MdnsSocket::open(&VerifiedInterface::inspect(&config(name, ip, 500)).unwrap()).unwrap()
}
fn rpc(request: serde_json::Value) -> serde_json::Value {
    let path = std::env::var("CYANREX_MDNS_TEST_PEER_SOCKET").unwrap();
    let mut stream = UnixStream::connect(path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    serde_json::to_writer(&mut stream, &request).unwrap();
    stream.write_all(b"\n").unwrap();
    let mut response = String::new();
    BufReader::new(stream)
        .take(32768)
        .read_line(&mut response)
        .unwrap();
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["ok"], true, "isolated peer fixture failed");
    response
}
struct FixtureSender {
    ip: String,
    port: u16,
    ttl: u32,
}
fn sender(ip: &str, port: u16, ttl: u32) -> FixtureSender {
    FixtureSender {
        ip: ip.into(),
        port,
        ttl,
    }
}
impl FixtureSender {
    fn send_to(&self, data: &[u8], destination: (impl ToString, u16)) -> std::io::Result<usize> {
        assert_eq!(destination.1, MDNS_PORT);
        rpc(
            serde_json::json!({"op":"send","ip":self.ip,"port":self.port,"ttl":self.ttl,
            "destination":destination.0.to_string(),"data":data}),
        );
        Ok(data.len())
    }
}
fn label(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for label in name.trim_end_matches('.').split('.') {
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
    out
}
fn rr(out: &mut Vec<u8>, owner: &str, kind: u16, ttl: u32, data: &[u8]) {
    out.extend(label(owner));
    out.extend(kind.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend(ttl.to_be_bytes());
    out.extend((data.len() as u16).to_be_bytes());
    out.extend(data);
}
fn reply(goodbye: bool) -> Vec<u8> {
    let mut out = vec![
        0,
        0,
        0x84,
        0,
        0,
        0,
        0,
        if goodbye { 1 } else { 4 },
        0,
        0,
        0,
        0,
    ];
    rr(
        &mut out,
        "_cyanrex._udp.local.",
        12,
        if goodbye { 0 } else { 60 },
        &label("peer._cyanrex._udp.local."),
    );
    if goodbye {
        return out;
    }
    let mut srv = vec![0, 0, 0, 0];
    srv.extend(4433u16.to_be_bytes());
    srv.extend(label("peer.local."));
    rr(&mut out, "peer._cyanrex._udp.local.", 33, 60, &srv);
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let raw=serde_json::to_vec(&serde_json::json!({"schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6","protocol":{"min":1,"max":3},
        "offered_capabilities":[],"required_capabilities":[],"issued_at":wall-1,"expires_at":wall+60,
        "hints":[{"kind":"direct","address":"10.23.0.2:4433"}]})).unwrap();
    let txt = encode_txt(&PeerAdvertisement::parse(&raw).unwrap()).unwrap();
    rr(&mut out, "peer._cyanrex._udp.local.", 16, 60, &txt);
    rr(&mut out, "peer.local.", 1, 60, &[10, 23, 0, 2]);
    out
}
fn policy() -> DiscoveryPolicy {
    DiscoveryPolicy::new(
        "a".repeat(64).parse().unwrap(),
        ProtocolRange::new(1, 3).unwrap(),
        vec![],
        vec![],
    )
    .unwrap()
}
fn no_packet(
    result: Result<Option<cyanrex_engine::services::federation_mdns::ReceivedMdns>, MdnsError>,
) {
    assert!(matches!(
        result,
        Ok(None) | Err(MdnsError::InvalidPacket) | Err(MdnsError::TooLarge)
    ));
}

#[test]
#[ignore = "requires explicit anonymous netns runner, never host LAN"]
fn selected_interface_receives_multicast_and_query_uses_the_pinned_source() {
    isolated(false);
    let mut receiver = browser("crx0", "10.23.0.1");
    let sender = sender("10.23.0.2", MDNS_PORT, 255);
    sender
        .send_to(b"selected", (MDNS_GROUP, MDNS_PORT))
        .unwrap();
    let packet = receiver.receive().unwrap().unwrap();
    assert_eq!(packet.bytes, b"selected");
    assert_eq!(packet.source, "10.23.0.2".parse::<Ipv4Addr>().unwrap());
    rpc(serde_json::json!({"op":"arm","replies":[]}));
    receiver.send_query(&browse_query()).unwrap();
    let query = rpc(serde_json::json!({"op":"wait"}));
    assert_eq!(
        query["queries"][0]["data"],
        serde_json::json!(browse_query())
    );
    assert_eq!(query["queries"][0]["source"], "10.23.0.1");
    assert!(matches!(
        receiver.send_query(b"arbitrary"),
        Err(MdnsError::InvalidPacket)
    ));
}

#[test]
#[ignore = "requires explicit anonymous netns runner, never host LAN"]
fn wrong_ttl_port_unicast_and_oversized_packets_never_become_observations() {
    isolated(false);
    let mut receiver = browser("crx0", "10.23.0.1");
    for (port, ttl) in [(MDNS_PORT, 254), (5354, 255)] {
        let sender = sender("10.23.0.2", port, ttl);
        sender.send_to(b"invalid", (MDNS_GROUP, MDNS_PORT)).unwrap();
        no_packet(receiver.receive());
    }
    let sender = sender("10.23.0.2", MDNS_PORT, 255);
    sender
        .send_to(b"unicast", ("10.23.0.1", MDNS_PORT))
        .unwrap();
    no_packet(receiver.receive());
    sender
        .send_to(&vec![0u8; 5000], (MDNS_GROUP, MDNS_PORT))
        .unwrap();
    no_packet(receiver.receive());
    sender.send_to(b"control", (MDNS_GROUP, MDNS_PORT)).unwrap();
    assert_eq!(receiver.receive().unwrap().unwrap().bytes, b"control");
}

#[test]
#[ignore = "requires explicit anonymous netns runner, never host LAN"]
fn another_interface_join_does_not_leak_into_the_selected_socket() {
    isolated(false);
    let mut selected = browser("crx0", "10.23.0.1");
    let mut other = browser("crx2", "10.24.0.1");
    let sender = sender("10.24.0.2", MDNS_PORT, 255);
    sender
        .send_to(b"other-interface", (MDNS_GROUP, MDNS_PORT))
        .unwrap();
    assert_eq!(other.receive().unwrap().unwrap().bytes, b"other-interface");
    no_packet(selected.receive());
}

struct FixtureWorker;
impl FixtureWorker {
    fn join(self) -> Result<(), ()> {
        let response = rpc(serde_json::json!({"op":"wait"}));
        assert_eq!(
            response["queries"][0]["data"],
            serde_json::json!(browse_query())
        );
        assert_eq!(response["queries"][0]["source"], "10.23.0.1");
        Ok(())
    }
}
fn responder(goodbye: bool) -> FixtureWorker {
    let mut replies = vec![reply(false)];
    if goodbye {
        replies.push(reply(true));
    }
    rpc(serde_json::json!({"op":"arm","replies":replies}));
    FixtureWorker
}

#[test]
#[ignore = "requires explicit anonymous netns runner, never host LAN"]
fn real_query_dns_txt_f0_cache_chain_closes_every_descriptor() {
    isolated(false);
    let baseline = fs::read_dir("/proc/self/fd").unwrap().count();
    let worker = responder(false);
    let peers = discover_once(
        &config("crx0", "10.23.0.1", 350),
        policy(),
        &AtomicBool::new(false),
    )
    .unwrap();
    worker.join().unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].source().instance(), "peer");
    assert_eq!(peers[0].candidate().claimed_node().as_str(), "b".repeat(64));
    assert_eq!(fs::read_dir("/proc/self/fd").unwrap().count(), baseline);
}

#[test]
#[ignore = "requires explicit anonymous netns runner, never host LAN"]
fn same_sender_goodbye_removes_only_the_discovered_hint() {
    isolated(false);
    let worker = responder(true);
    let peers = discover_once(
        &config("crx0", "10.23.0.1", 350),
        policy(),
        &AtomicBool::new(false),
    )
    .unwrap();
    worker.join().unwrap();
    assert!(peers.is_empty());
}

#[test]
#[ignore = "requires explicit anonymous netns runner, never host LAN"]
fn in_flight_cancellation_closes_without_a_background_receiver() {
    isolated(false);
    let baseline = fs::read_dir("/proc/self/fd").unwrap().count();
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let worker = thread::spawn(move || {
        thread::sleep(Duration::from_millis(80));
        flag.store(true, Ordering::Release);
    });
    let start = Instant::now();
    assert!(matches!(
        discover_once(&config("crx0", "10.23.0.1", 5000), policy(), &cancelled),
        Err(MdnsError::Cancelled)
    ));
    worker.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(fs::read_dir("/proc/self/fd").unwrap().count(), baseline);
}

#[test]
#[ignore = "requires explicit anonymous netns mutation fixture, never host LAN"]
fn selected_address_removal_invalidates_the_socket_without_fallback() {
    isolated(true);
    let baseline = fs::read_dir("/proc/self/fd").unwrap().count();
    let mut receiver = browser("crx0", "10.23.0.1");
    assert!(std::process::Command::new("ip")
        .args(["address", "del", "10.23.0.1/24", "dev", "crx0"])
        .status()
        .unwrap()
        .success());
    assert!(matches!(
        receiver.send_query(&browse_query()),
        Err(MdnsError::InterfaceChanged)
    ));
    assert!(matches!(
        receiver.receive(),
        Err(MdnsError::InterfaceChanged)
    ));
    drop(receiver);
    assert_eq!(fs::read_dir("/proc/self/fd").unwrap().count(), baseline);
}
