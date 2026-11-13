use cyanrex_engine::services::{
    federation_discovery::PeerAdvertisement,
    federation_mdns::{MdnsAnnouncement, MdnsConfig},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddrV4, UdpSocket},
    os::unix::net::UnixStream,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub(super) fn network_case(expected_udp: u64, case: impl FnOnce()) {
    verify_network();
    let status = fs::read_to_string("/proc/self/status").unwrap();
    for name in ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"] {
        let value = capability(&status, name);
        assert_eq!(value, 0);
    }
    assert!(status.lines().any(|line| line == "NoNewPrivs:\t1"));
    measured(expected_udp, case);
}

pub(super) fn network_mutation_case(expected_udp: u64, case: impl FnOnce()) {
    verify_network();
    assert_eq!(std::env::var("CYANREX_MDNS_TEST_MUTATE").unwrap(), "1");
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let net_admin = 1 << 12;
    for name in ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"] {
        let value = capability(&status, name);
        assert_eq!(
            value & !net_admin,
            0,
            "only owned-netns NET_ADMIN is allowed"
        );
    }
    for name in ["CapPrm", "CapEff", "CapBnd"] {
        assert_eq!(capability(&status, name), net_admin);
    }
    assert!(status.lines().any(|line| line == "NoNewPrivs:\t1"));
    measured(expected_udp, case);
}

fn capability(status: &str, name: &str) -> u64 {
    let value = status
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name}:\t")))
        .unwrap();
    u64::from_str_radix(value, 16).unwrap()
}

fn verify_network() {
    assert_eq!(
        std::env::var("CYANREX_MDNS_TEST_WITH_CONTROLLER").unwrap(),
        "1"
    );
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
}

fn measured(expected_udp: u64, case: impl FnOnce()) {
    assert_no_listener();
    let descriptors = fd_count();
    let (udp, tcp) = network_counts();
    case();
    assert_eq!(fd_count(), descriptors, "controller leaked a descriptor");
    assert_eq!(network_counts(), (udp + expected_udp, tcp));
    assert_no_listener();
}

pub(super) fn assert_no_listener() {
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
    // SAFETY: the fixed monotonic clock writes only this live, correctly sized timespec.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) },
        0
    );
    (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64
}

pub(super) fn config() -> MdnsConfig {
    MdnsConfig::new("crx0", "10.23.0.1".parse().unwrap(), Duration::from_secs(2)).unwrap()
}

pub(super) fn announcement(ip: &str, now: u64) -> MdnsAnnouncement {
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
        "node-one",
        "host-one",
        SocketAddrV4::new(ip.parse::<Ipv4Addr>().unwrap(), 4433),
        &advertisement,
        now,
        60,
    )
    .unwrap()
}

pub(super) fn local(now: u64) -> MdnsAnnouncement {
    announcement("10.23.0.1", now)
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
    assert_eq!(response["ok"], true, "owned controller peer fixture failed");
    response
}

pub(super) fn arm(after: u8, destination: &str, bytes: &[u8]) {
    rpc(json!({"op":"arm-probes", "reply_after":after,
        "destination":destination,"data":bytes}));
}

pub(super) fn finish(expected: usize, reference: &[u8]) -> Vec<u64> {
    let response = rpc(json!({"op":"wait-probes"}));
    let queries = response["queries"].as_array().unwrap();
    assert_eq!(queries.len(), expected, "unexpected probe or announcement");
    let mut times = Vec::new();
    for query in queries {
        assert_eq!(query["source"], "10.23.0.1");
        assert_eq!(query["port"], 5353);
        assert_eq!(query["ttl"], 255);
        let bytes: Vec<u8> = serde_json::from_value(query["data"].clone()).unwrap();
        assert_eq!(canonical_probe(&bytes), canonical_probe(reference));
        times.push(query["ns"].as_u64().unwrap());
    }
    for pair in times.windows(2) {
        assert!(pair[1] - pair[0] >= 240_000_000);
    }
    times
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

pub(super) fn occupy_local_port() -> UdpSocket {
    UdpSocket::bind("10.23.0.1:5353").unwrap()
}
