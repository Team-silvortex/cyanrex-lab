use cyanrex_engine::services::{
    federation_discovery::PeerAdvertisement,
    federation_mdns::{
        MdnsAnnouncement, MdnsConfig, MdnsError, MdnsUnicastReceiver, ReceivedMdns,
        VerifiedInterface,
    },
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddrV4},
    os::unix::net::UnixStream,
    time::Duration,
};

fn isolated(mutation: bool) {
    assert_eq!(
        std::env::var("CYANREX_MDNS_TEST_WITH_UNICAST").unwrap(),
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
    let status = fs::read_to_string("/proc/self/status").unwrap();
    for name in ["CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"] {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}:\t")))
            .unwrap();
        let value = u64::from_str_radix(value, 16).unwrap();
        if mutation {
            assert_eq!(value & !(1 << 12), 0, "mutation may hold only NET_ADMIN");
            if matches!(name, "CapEff" | "CapPrm" | "CapBnd") {
                assert_eq!(value, 1 << 12);
            }
        } else {
            assert_eq!(value, 0, "ordinary receiver driver must be capability-free");
        }
    }
    assert!(status.lines().any(|line| line == "NoNewPrivs:\t1"));
    if mutation {
        assert_eq!(std::env::var("CYANREX_MDNS_TEST_MUTATE").unwrap(), "1");
    }
    assert_no_mdns_listener();
}

pub(super) fn fd_count() -> usize {
    fs::read_dir("/proc/self/fd").unwrap().count()
}

fn assert_no_mdns_listener() {
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

fn network_counts() -> (u64, u64) {
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

pub(super) fn network_case(mutation: bool, case: impl FnOnce()) {
    isolated(mutation);
    let baseline = fd_count();
    let counts = network_counts();
    case();
    assert_eq!(
        fd_count(),
        baseline,
        "receiver or failed bind leaked a descriptor"
    );
    assert_eq!(
        network_counts(),
        counts,
        "receive-only case emitted UDP or opened TCP"
    );
    assert_no_mdns_listener();
}

pub(super) fn verified(name: &str, address: &str) -> VerifiedInterface {
    let config =
        MdnsConfig::new(name, address.parse().unwrap(), Duration::from_millis(200)).unwrap();
    VerifiedInterface::inspect(&config).unwrap()
}

pub(super) fn unicast(name: &str, address: &str) -> MdnsUnicastReceiver {
    MdnsUnicastReceiver::open(&verified(name, address)).unwrap()
}

fn rpc(request: Value) {
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
    assert_eq!(response["ok"], true, "owned unicast peer fixture failed");
}

pub(super) fn send(ip: &str, port: u16, ttl: u32, destination: &str, bytes: &[u8]) {
    rpc(json!({"op":"send", "ip":ip, "port":port, "ttl":ttl,
        "destination":destination, "data":bytes}));
}

pub(super) fn no_packet(result: Result<Option<ReceivedMdns>, MdnsError>) {
    assert!(matches!(
        result,
        Ok(None) | Err(MdnsError::InvalidPacket) | Err(MdnsError::TooLarge)
    ));
}

pub(super) fn announcement(instance: &str, host: &str) -> MdnsAnnouncement {
    let advertisement = PeerAdvertisement::parse(
        &serde_json::to_vec(&json!({
            "schema_version":1,"node_fingerprint":"b".repeat(64),
            "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
            "protocol":{"min":1,"max":3},"offered_capabilities":[],
            "required_capabilities":[],"issued_at":999,"expires_at":1060,
            "hints":[{"kind":"direct","address":"10.23.0.2:4433"}]
        }))
        .unwrap(),
    )
    .unwrap();
    MdnsAnnouncement::new(
        instance,
        host,
        SocketAddrV4::new(Ipv4Addr::new(10, 23, 0, 2), 4433),
        &advertisement,
        1000,
        60,
    )
    .unwrap()
}
