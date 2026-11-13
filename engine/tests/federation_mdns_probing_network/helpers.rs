use cyanrex_engine::services::{
    federation_discovery::{
        lan::{encode_txt, MAX_TXT_RECORD_BYTES},
        PeerAdvertisement,
    },
    federation_mdns::{MdnsAnnouncement, MdnsConfig, MdnsError, ProbeRunObservation},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    os::unix::net::UnixStream,
    sync::atomic::AtomicBool,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) fn network_case(expected_udp: u64, case: impl FnOnce()) {
    assert_eq!(std::env::var("CYANREX_MDNS_TEST_WITH_PROBE").unwrap(), "1");
    let parent = std::env::var("CYANREX_MDNS_TEST_PARENT_NETNS").unwrap();
    assert_ne!(
        fs::read_link("/proc/self/ns/net")
            .unwrap()
            .to_string_lossy(),
        parent
    );
    let devices = fs::read_to_string("/proc/self/net/dev").unwrap();
    let mut names: Vec<_> = devices
        .lines()
        .skip(2)
        .map(|line| line.split(':').next().unwrap().trim())
        .collect();
    names.sort();
    assert_eq!(names, ["crx0", "crx2", "lo"]);
    assert!(!fs::read_to_string("/proc/self/net/route")
        .unwrap()
        .lines()
        .skip(1)
        .any(|line| line.split_whitespace().nth(1) == Some("00000000")));
    let status = fs::read_to_string("/proc/self/status").unwrap();
    for name in ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"] {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}:\t")))
            .unwrap();
        assert_eq!(u64::from_str_radix(value, 16).unwrap(), 0);
    }
    assert!(status.lines().any(|line| line == "NoNewPrivs:\t1"));
    no_listener();
    let descriptors = fd_count();
    let (udp, tcp) = network_counts();
    case();
    assert_eq!(fd_count(), descriptors, "probe round leaked a descriptor");
    assert_eq!(network_counts(), (udp + expected_udp, tcp));
    no_listener();
}

fn no_listener() {
    for path in ["/proc/self/net/udp", "/proc/self/net/udp6"] {
        assert!(!fs::read_to_string(path)
            .unwrap()
            .lines()
            .skip(1)
            .any(|line| line
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.rsplit(':').next())
                == Some("14E9")));
    }
}

pub(super) fn fd_count() -> usize {
    fs::read_dir("/proc/self/fd").unwrap().count()
}

pub(super) fn network_counts() -> (u64, u64) {
    let source = fs::read_to_string("/proc/self/net/snmp").unwrap();
    let lines: Vec<_> = source.lines().collect();
    let field = |prefix: &str, field: &str| {
        let index = lines
            .iter()
            .position(|line| line.starts_with(prefix))
            .unwrap();
        let column = lines[index]
            .split_whitespace()
            .position(|value| value == field)
            .unwrap();
        lines[index + 1]
            .split_whitespace()
            .nth(column)
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    (field("Udp:", "OutDatagrams"), field("Tcp:", "ActiveOpens"))
}

pub(super) fn wall() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub(super) fn monotonic_ns() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a live, correctly sized timespec receives this fixed monotonic clock sample.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) },
        0
    );
    (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64
}

pub(super) fn config() -> MdnsConfig {
    MdnsConfig::new("crx0", "10.23.0.1".parse().unwrap(), Duration::from_secs(2)).unwrap()
}

pub(super) fn announcement(instance: &str, host: &str, ip: &str, now: u64) -> MdnsAnnouncement {
    let advertisement = PeerAdvertisement::parse(
        &serde_json::to_vec(&json!({
            "schema_version":1,"node_fingerprint":"b".repeat(64),
            "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
            "protocol":{"min":1,"max":3},"offered_capabilities":[],
            "required_capabilities":[],"issued_at":now-1,"expires_at":now+60,
            "hints":[{"kind":"direct","address":format!("{ip}:4433")}]
        }))
        .unwrap(),
    )
    .unwrap();
    MdnsAnnouncement::new(
        instance,
        host,
        SocketAddrV4::new(ip.parse::<Ipv4Addr>().unwrap(), 4433),
        &advertisement,
        now,
        60,
    )
    .unwrap()
}

pub(super) fn local(now: u64) -> MdnsAnnouncement {
    announcement("node-one", "host-one", "10.23.0.1", now)
}

pub(super) fn oversized_for_fixture_mtu(now: u64) -> MdnsAnnouncement {
    let offered: Vec<_> = (0..12)
        .map(|index| format!("peer.{}{:02}", "x".repeat(39), index))
        .collect();
    let advertisement = PeerAdvertisement::parse(
        &serde_json::to_vec(&json!({
            "schema_version":1,"node_fingerprint":"b".repeat(64),
            "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
            "protocol":{"min":1,"max":3},"offered_capabilities":offered,
            "required_capabilities":[],"issued_at":now-1,"expires_at":now+60,
            "hints":[{"kind":"direct","address":"10.23.0.1:4433"}]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(encode_txt(&advertisement).unwrap().len() <= MAX_TXT_RECORD_BYTES);
    let prepared = MdnsAnnouncement::new(
        &"n".repeat(63),
        &"h".repeat(63),
        "10.23.0.1:4433".parse().unwrap(),
        &advertisement,
        now,
        60,
    )
    .unwrap();
    assert!(prepared.encode_probe_at(now).unwrap().len() + 28 > 1500);
    prepared
}

fn rpc(request: Value) -> Value {
    let mut socket =
        UnixStream::connect(std::env::var_os("CYANREX_MDNS_TEST_PEER_SOCKET").unwrap()).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut bytes = serde_json::to_vec(&request).unwrap();
    bytes.push(b'\n');
    assert!(bytes.len() <= 32768);
    socket.write_all(&bytes).unwrap();
    let mut response = String::new();
    BufReader::new(socket)
        .take(32769)
        .read_line(&mut response)
        .unwrap();
    assert!(response.len() <= 32768 && response.ends_with('\n'));
    let response: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["ok"], true, "owned probing peer fixture failed");
    response
}

pub(super) fn arm(after: u8, destination: &str, bytes: &[u8]) {
    rpc(json!({"op":"arm-probes", "reply_after":after,
        "destination":destination,"data":bytes}));
}

pub(super) fn finish(expected: usize, reference: &[u8]) -> Vec<Value> {
    let response = rpc(json!({"op":"wait-probes"}));
    let queries = response["queries"].as_array().unwrap().clone();
    assert_eq!(
        queries.len(),
        expected,
        "unexpected later probe or announcement"
    );
    for query in &queries {
        assert_eq!(query["source"], "10.23.0.1");
        assert_eq!(query["port"], 5353);
        assert_eq!(query["ttl"], 255);
        let bytes: Vec<u8> = serde_json::from_value(query["data"].clone()).unwrap();
        assert_eq!(canonical_probe(&bytes), canonical_probe(reference));
    }
    for pair in queries.windows(2) {
        let elapsed = pair[1]["ns"].as_u64().unwrap() - pair[0]["ns"].as_u64().unwrap();
        assert!(elapsed >= 240_000_000, "probes repeated too quickly");
    }
    queries
}

fn skip_name(bytes: &[u8], offset: &mut usize) {
    loop {
        let length = bytes[*offset] as usize;
        *offset += 1;
        assert!(length <= 63, "prepared probes must not use compression");
        if length == 0 {
            break;
        }
        *offset += length;
        assert!(*offset <= bytes.len());
    }
}

fn canonical_probe(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() <= 4096);
    assert_eq!(&bytes[..12], &[0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0]);
    let mut canonical = bytes.to_vec();
    let mut offset = 12;
    for _ in 0..2 {
        skip_name(bytes, &mut offset);
        assert_eq!(&bytes[offset..offset + 4], &[0, 255, 0x80, 1]);
        offset += 4;
    }
    for kind in [33_u16, 16, 1] {
        skip_name(bytes, &mut offset);
        assert_eq!(&bytes[offset..offset + 2], &kind.to_be_bytes());
        assert_eq!(&bytes[offset + 2..offset + 4], &[0, 1]);
        let ttl = u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
        assert!((1..=60).contains(&ttl));
        canonical[offset + 4..offset + 8].fill(0);
        let length = u16::from_be_bytes(bytes[offset + 8..offset + 10].try_into().unwrap());
        offset += 10 + usize::from(length);
        assert!(offset <= bytes.len());
    }
    assert_eq!(offset, bytes.len());
    canonical
}

pub(super) fn run(
    announcement: &MdnsAnnouncement,
    cancelled: &AtomicBool,
) -> (Result<ProbeRunObservation, MdnsError>, u64) {
    let started = Instant::now();
    let result =
        cyanrex_engine::services::federation_mdns::probe_once(&config(), announcement, cancelled);
    let ended = monotonic_ns();
    assert!(started.elapsed() < Duration::from_millis(2250));
    (result, ended)
}

pub(super) fn occupy_local_port() -> UdpSocket {
    UdpSocket::bind("10.23.0.1:5353").unwrap()
}
