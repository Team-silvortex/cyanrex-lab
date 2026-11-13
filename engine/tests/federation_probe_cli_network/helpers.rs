use cyanrex_engine::services::{
    federation_discovery::PeerAdvertisement, federation_mdns::MdnsAnnouncement,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddrV4, UdpSocket},
    os::unix::{net::UnixStream, process::ExitStatusExt},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CANARY: &str = "synthetic.canaryf1kprivatevalue6a7c";

pub(super) fn network_case(expected_udp: u64, case: impl FnOnce()) {
    isolated();
    let descriptors = fs::read_dir("/proc/self/fd").unwrap().count();
    let counts = network_counts();
    case();
    assert_no_listener();
    assert_eq!(fs::read_dir("/proc/self/fd").unwrap().count(), descriptors);
    assert_eq!(network_counts(), (counts.0 + expected_udp, counts.1));
}

fn isolated() {
    assert_eq!(
        std::env::var("CYANREX_MDNS_TEST_WITH_PROBE_CLI").unwrap(),
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
        assert!(status
            .lines()
            .any(|line| line == format!("{name}:\t0000000000000000")));
    }
    let groups = status
        .lines()
        .find(|line| line.starts_with("Groups:"))
        .unwrap();
    assert_eq!(groups.split_whitespace().collect::<Vec<_>>(), ["Groups:"]);
    assert!(status.lines().any(|line| line == "NoNewPrivs:\t1"));
    assert_no_listener();
}

fn assert_no_listener() {
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
    drop(UdpSocket::bind("10.23.0.1:5353").unwrap());
}

pub(super) fn network_counts() -> (u64, u64) {
    let text = fs::read_to_string("/proc/self/net/snmp").unwrap();
    let lines: Vec<_> = text.lines().collect();
    let counter = |protocol: &str, field: &str| {
        let index = lines
            .iter()
            .position(|line| line.starts_with(protocol))
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
    (
        counter("Udp:", "OutDatagrams"),
        counter("Tcp:", "ActiveOpens"),
    )
}

pub(super) fn wall() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn monotonic_ns() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: the fixed monotonic clock writes this live, correctly sized timespec.
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) },
        0
    );
    (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64
}

pub(super) fn advertisement(ip: &str, now: u64) -> String {
    json!({
        "schema_version":1,"node_fingerprint":"a".repeat(64),
        "authority_id":"8b075b0b-8118-4c37-9224-337ba22f40c6",
        "protocol":{"min":1,"max":3},"offered_capabilities":[CANARY],
        "required_capabilities":[],"issued_at":now-1,"expires_at":now+60,
        "hints":[{"kind":"direct","address":format!("{ip}:4433")}]
    })
    .to_string()
}

pub(super) fn prepared(ip: &str, now: u64) -> MdnsAnnouncement {
    let claim = PeerAdvertisement::parse(advertisement(ip, now).as_bytes()).unwrap();
    MdnsAnnouncement::new(
        "node-one",
        "host-one",
        SocketAddrV4::new(ip.parse().unwrap(), 4433),
        &claim,
        now,
        60,
    )
    .unwrap()
}

pub(super) fn arguments(now: u64) -> Vec<String> {
    [
        "probe-lan",
        "--uid",
        "1000",
        "--gid",
        "1000",
        "--groups",
        "none",
        "--interface",
        "crx0",
        "--address",
        "10.23.0.1",
        "--timeout-ms",
        "2000",
        "--instance",
        "node-one",
        "--host",
        "host-one",
        "--port",
        "4433",
        "--ttl",
        "60",
        "--advertisement",
        &advertisement("10.23.0.1", now),
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub(super) struct Cli {
    child: Child,
}

pub(super) struct Captured {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) completed_ns: u64,
}

impl Cli {
    pub(super) fn start(arguments: &[String]) -> Self {
        let child = Command::new("unshare")
            .args([
                "--user",
                "--map-user=1000",
                "--map-group=1000",
                "--keep-caps",
                "setpriv",
                "--bounding-set=-all",
                "--inh-caps=-all",
                "--ambient-caps=-all",
                "--no-new-privs",
                env!("CARGO_BIN_EXE_cyanrex-discovery"),
            ])
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("owned probe CLI startup failed");
        Self { child }
    }

    pub(super) fn signal_after_first_probe(&mut self, sent_before: u64, signal: i32) {
        let started = Instant::now();
        loop {
            let sent = network_counts().0 - sent_before;
            if sent != 0 {
                assert_eq!(sent, 1, "CLI sent another packet before interruption");
                break;
            }
            assert!(self.child.try_wait().unwrap().is_none());
            assert!(started.elapsed() < Duration::from_secs(2));
            thread::sleep(Duration::from_millis(1));
        }
        assert!(self.child.try_wait().unwrap().is_none());
        // SAFETY: the unreaped Child owns this PID; terminate only the explicit test process.
        assert_eq!(
            unsafe { libc::kill(self.child.id().try_into().unwrap(), signal) },
            0
        );
    }

    pub(super) fn finish(mut self) -> Captured {
        let started = Instant::now();
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(started.elapsed() < Duration::from_secs(4));
            thread::sleep(Duration::from_millis(2));
        };
        let completed_ns = monotonic_ns();
        let mut stdout = Vec::new();
        self.child
            .stdout
            .take()
            .unwrap()
            .take(513)
            .read_to_end(&mut stdout)
            .unwrap();
        let mut stderr = Vec::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .take(513)
            .read_to_end(&mut stderr)
            .unwrap();
        assert!(stdout.len() <= 512 && stderr.len() <= 512);
        Captured {
            status,
            stdout,
            stderr,
            completed_ns,
        }
    }
}

impl Drop for Cli {
    fn drop(&mut self) {
        if self.child.try_wait().is_ok_and(|status| status.is_none()) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

impl Captured {
    pub(super) fn observation(&self, observation: &str) {
        assert!(self.status.success());
        assert!(self.stderr.is_empty());
        assert_eq!(self.stdout.last(), Some(&b'\n'));
        assert_eq!(self.stdout.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&self.stdout).unwrap(),
            json!({"schema_version":1,"kind":"unverified_lan_probe_observation",
                "observation_only":true,"verified":false,"name_ownership_verified":false,
                "publication_permitted":false,"announcement_sent":false,
                "observation":observation})
        );
        assert!(!String::from_utf8_lossy(&self.stdout).contains(CANARY));
    }

    pub(super) fn rejected(&self, code: i32) {
        assert_eq!(self.status.code(), Some(code));
        assert!(self.stdout.is_empty());
        assert!(!self.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&self.stderr).contains(CANARY));
    }

    pub(super) fn interrupted(&self, signal: i32) {
        assert_eq!(self.status.signal(), Some(signal));
        assert!(self.stdout.is_empty() && self.stderr.is_empty());
    }
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
    assert_eq!(response["ok"], true, "owned probe CLI peer fixture failed");
    response
}

pub(super) fn arm(after: u8, destination: &str, bytes: &[u8]) {
    rpc(json!({"op":"arm-probes","reply_after":after,
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
    // Peer receive timestamps are userspace samples, matching the existing F1g acceptance bound.
    for pair in times.windows(2) {
        assert!(
            pair[1] - pair[0] >= 240_000_000,
            "probes repeated too quickly"
        );
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
