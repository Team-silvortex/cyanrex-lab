//! Opt-in real CLI acceptance inside the reviewed anonymous namespace runner, never host LAN.
#![cfg(target_os = "linux")]

use cyanrex_engine::services::{
    federation_discovery::{lan::encode_txt, PeerAdvertisement},
    federation_mdns::{browse_query, MAX_PACKETS_PER_RUN},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{net::UnixStream, process::ExitStatusExt},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const AUTHORITY: &str = "8b075b0b-8118-4c37-9224-337ba22f40c6";
const MAX_CAPTURE: usize = 4096;

fn isolated() {
    assert_eq!(std::env::var("CYANREX_MDNS_TEST_WITH_CLI").unwrap(), "1");
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
    assert_no_mdns_listener();
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
    let value: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["ok"], true, "isolated CLI peer fixture failed");
    value
}

fn arm(replies: Vec<Vec<u8>>) {
    rpc(json!({"op":"arm", "replies":replies}));
}

fn query_completed() {
    let response = rpc(json!({"op":"wait"}));
    assert_eq!(
        response["queries"],
        json!([{
            "source":"10.23.0.1", "data":browse_query()
        }])
    );
}

fn assert_no_mdns_listener() {
    assert!(!fs::read_to_string("/proc/self/net/udp")
        .unwrap()
        .lines()
        .skip(1)
        .any(|line| line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.rsplit(':').next())
            == Some("14E9")));
}

#[derive(Clone, Copy)]
struct NetworkCounts {
    sent: u64,
    received: u64,
    active_tcp: u64,
}
impl NetworkCounts {
    fn read() -> Self {
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
                .parse()
                .unwrap()
        };
        Self {
            sent: counter("Udp:", "OutDatagrams"),
            received: counter("Udp:", "InDatagrams"),
            active_tcp: counter("Tcp:", "ActiveOpens"),
        }
    }
    fn only_queries(self, count: u64) {
        let after = Self::read();
        assert_eq!(
            after.sent,
            self.sent + count,
            "unexpected non-query UDP traffic"
        );
        assert_eq!(
            after.active_tcp, self.active_tcp,
            "unexpected TCP connection"
        );
        assert_no_mdns_listener();
    }
}

struct Cli {
    child: Child,
}
struct Captured {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}
impl Cli {
    fn start(interface: &str, timeout: &str) -> Self {
        let arguments = [
            "browse-lan",
            "--uid",
            "1000",
            "--gid",
            "1000",
            "--groups",
            "none",
            "--interface",
            interface,
            "--address",
            "10.23.0.1",
            "--local-node",
            &"a".repeat(64),
            "--protocol",
            "1:3",
            "--timeout-ms",
            timeout,
            "--capabilities",
            "peer.identity",
            "--require",
            "none",
        ];
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
            .expect("owned CLI startup failed");
        Self { child }
    }
    fn signal(&mut self, signal: i32) {
        assert!(
            self.child.try_wait().unwrap().is_none(),
            "CLI ended before the signal boundary"
        );
        // SAFETY: the unreaped Child owns this PID; only its explicit OS termination is requested.
        assert_eq!(
            unsafe { libc::kill(self.child.id().try_into().unwrap(), signal) },
            0
        );
    }
    fn finish(mut self) -> Captured {
        let started = Instant::now();
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                started.elapsed() < Duration::from_secs(7),
                "owned CLI exceeded its deadline"
            );
            thread::sleep(Duration::from_millis(5));
        };
        let mut stdout = Vec::new();
        self.child
            .stdout
            .take()
            .unwrap()
            .take((MAX_CAPTURE + 1) as u64)
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
        assert!(stdout.len() <= MAX_CAPTURE && stderr.len() <= 512);
        Captured {
            status,
            stdout,
            stderr,
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
    fn snapshot(&self) -> Value {
        assert!(self.status.success());
        assert!(self.stderr.is_empty());
        assert_eq!(self.stdout.last(), Some(&b'\n'));
        assert_eq!(self.stdout.iter().filter(|byte| **byte == b'\n').count(), 1);
        serde_json::from_slice(&self.stdout).unwrap()
    }
    fn rejected(&self, code: i32) {
        assert_eq!(self.status.code(), Some(code));
        assert!(self.stdout.is_empty());
        assert_eq!(self.stderr, b"LAN discovery failed; no snapshot emitted\n");
    }
}

fn label(name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for part in name.trim_end_matches('.').split('.') {
        bytes.push(part.len().try_into().unwrap());
        bytes.extend_from_slice(part.as_bytes());
    }
    bytes.push(0);
    bytes
}
fn rr(bytes: &mut Vec<u8>, owner: &str, kind: u16, data: &[u8]) {
    bytes.extend(label(owner));
    bytes.extend(kind.to_be_bytes());
    bytes.extend(1_u16.to_be_bytes());
    bytes.extend(60_u32.to_be_bytes());
    bytes.extend(u16::try_from(data.len()).unwrap().to_be_bytes());
    bytes.extend(data);
}
fn reply() -> (Vec<u8>, u64) {
    let expires = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 60;
    let raw = serde_json::to_vec(
        &json!({"schema_version":1,"node_fingerprint":"b".repeat(64),
        "authority_id":AUTHORITY,"protocol":{"min":2,"max":4},
        "offered_capabilities":["peer.identity"],"required_capabilities":[],
        "issued_at":expires-61,"expires_at":expires,
        "hints":[{"kind":"direct","address":"10.23.0.2:4433"}]}),
    )
    .unwrap();
    let txt = encode_txt(&PeerAdvertisement::parse(&raw).unwrap()).unwrap();
    let mut bytes = vec![0, 0, 0x84, 0, 0, 0, 0, 4, 0, 0, 0, 0];
    rr(
        &mut bytes,
        "_cyanrex._udp.local.",
        12,
        &label("cli-peer._cyanrex._udp.local."),
    );
    let mut srv = vec![0, 0, 0, 0];
    srv.extend(4433_u16.to_be_bytes());
    srv.extend(label("cli-peer.local."));
    rr(&mut bytes, "cli-peer._cyanrex._udp.local.", 33, &srv);
    rr(&mut bytes, "cli-peer._cyanrex._udp.local.", 16, &txt);
    rr(&mut bytes, "cli-peer.local.", 1, &[10, 23, 0, 2]);
    (bytes, expires)
}
fn snapshot(candidates: Value) -> Value {
    json!({"schema_version":1,"kind":"unverified_lan_snapshot","snapshot_only":true,
        "verified":false,"candidates":candidates})
}

#[test]
#[ignore = "requires --with-cli anonymous netns runner, never host LAN"]
fn real_cli_emits_one_complete_unverified_candidate_without_business_traffic() {
    isolated();
    let counts = NetworkCounts::read();
    let (reply, expires) = reply();
    arm(vec![reply]);
    let child = Cli::start("crx0", "350");
    query_completed();
    let output = child.finish();
    assert_eq!(
        output.snapshot(),
        snapshot(json!([{
            "verified":false,"instance":"cli-peer","claimed_node_fingerprint":"b".repeat(64),
            "claimed_authority_id":AUTHORITY,"negotiated_protocol":3,
            "compatible_capabilities":["peer.identity"],"connection_hints":["10.23.0.2:4433"],
            "advertisement_expires_at":expires
        }]))
    );
    counts.only_queries(1);
}

#[test]
#[ignore = "requires --with-cli anonymous netns runner, never host LAN"]
fn real_cli_empty_window_is_a_complete_empty_snapshot_not_an_error() {
    isolated();
    let counts = NetworkCounts::read();
    arm(vec![]);
    let child = Cli::start("crx0", "150");
    query_completed();
    assert_eq!(child.finish().snapshot(), snapshot(json!([])));
    counts.only_queries(1);
}

#[test]
#[ignore = "requires --with-cli anonymous netns runner, never host LAN"]
fn real_cli_missing_interface_is_a_fixed_failure_without_stdout_or_udp() {
    isolated();
    let counts = NetworkCounts::read();
    Cli::start("cli-missing", "350").finish().rejected(74);
    counts.only_queries(0);
}

fn interrupted(signal: i32) {
    isolated();
    let counts = NetworkCounts::read();
    let (reply, _) = reply();
    arm(vec![reply]);
    let mut child = Cli::start("crx0", "5000");
    query_completed();
    let started = Instant::now();
    while NetworkCounts::read().received == counts.received {
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "reply was not received before interruption"
        );
        thread::sleep(Duration::from_millis(5));
    }
    child.signal(signal);
    let output = child.finish();
    assert_eq!(output.status.signal(), Some(signal));
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    counts.only_queries(1);
}

#[test]
#[ignore = "requires --with-cli anonymous netns runner, never host LAN"]
fn real_cli_sigint_after_query_closes_without_publishing_partial_claims() {
    interrupted(libc::SIGINT);
}

#[test]
#[ignore = "requires --with-cli anonymous netns runner, never host LAN"]
fn real_cli_sigterm_after_query_closes_without_publishing_partial_claims() {
    interrupted(libc::SIGTERM);
}

#[test]
#[ignore = "requires --with-cli anonymous netns runner, never host LAN"]
fn real_cli_receive_budget_failure_discards_previously_observed_candidates() {
    isolated();
    let counts = NetworkCounts::read();
    let (reply, _) = reply();
    arm(vec![reply]);
    let child = Cli::start("crx0", "5000");
    query_completed();
    for _ in 0..=MAX_PACKETS_PER_RUN {
        rpc(json!({"op":"send","ip":"10.23.0.2","port":5353,"ttl":255,
            "destination":"224.0.0.251","data":[0]}));
    }
    child.finish().rejected(74);
    counts.only_queries(1);
}
