import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const probeCliFiles = ["probe_arguments.rs", "probe.rs", "probe_output.rs"]
  .map(name => `engine/src/bin/cyanrex-discovery/${name}`);
async function sources(directory, excluded = []) {
  const result = [];
  for (const item of await readdir(new URL(`${directory}/`, root), { withFileTypes: true })) {
    const file = `${directory}/${item.name}`;
    if (excluded.includes(file)) continue;
    if (item.isDirectory()) result.push(...await sources(file, excluded));
    else if (item.name.endsWith(".rs")) result.push(await read(file));
  }
  return result;
}

test("federation discovery remains a pure prepared service, not a mounted peer authority", async () => {
  const directory = "engine/src/services/federation_discovery";
  const files = await sources(directory);
  assert.ok(files.length > 0);
  const source = files.join("\n");
  assert.match(await read("engine/src/services/mod.rs"), /pub mod federation_discovery;/);
  assert.match(source, /pub struct UnverifiedPeerCandidate/);
  assert.doesNotMatch(source, /\b(?:tokio|reqwest|sqlx|hyper|axum|AuthService|SessionToken|SystemTime|TcpListener|UdpSocket)\b/);
  assert.doesNotMatch(source, /std::(?:fs|env)|\.connect\(|\.bind\(|\.resolve\(/);
  for (const file of ["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs"])
    assert.doesNotMatch(await read(file), /federation_discovery|UnverifiedPeerCandidate/, file);
});

test("LAN preparation documents the compact TXT and local source cache without claiming mDNS acceptance", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const source = await read(`docs/${locale}/federation-discovery.md`);
    for (const phrase of ["F1a", "LanCandidateCache", "interface_epoch", "MAX_TXT_RECORD_BYTES", "ClockRollback"])
      assert.ok(source.includes(phrase), `${locale}: ${phrase}`);
  }
});

test("the explicit native mDNS adapter never joins application or platform startup", async () => {
  const source = (await sources("engine/src/services/federation_mdns")).join("\n");
  assert.match(source, /pub fn discover_once/);
  assert.match(source, /IP_MULTICAST_ALL/);
  assert.match(source, /IP_PKTINFO/);
  assert.match(source, /IP_RECVTTL/);
  assert.doesNotMatch(source, /\b(?:TcpStream|TcpListener|reqwest|AuthService|SessionToken|PgPool)\b/);
  for (const file of ["federation_mdns_packet_tdd", "federation_mdns_runtime_tdd",
    "federation_mdns_socket_tdd", "federation_mdns_network_tdd"])
    assert.match(await read(`engine/tests/${file}.rs`), /#!\[cfg\(target_os = "linux"\)\]/, file);
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs"])
    assert.doesNotMatch(await read(file), /federation_mdns|discover_once/, file);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const name of ["F1b", "discover_once", "IP_MULTICAST_ALL", "test-federation-mdns-netns.sh"])
      assert.ok(text.includes(name), `${locale}: ${name}`);
  }
});

test("federation direction documents distinguish F0 from future discovery and pairing", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const source = await read(`docs/${locale}/federation-discovery.md`);
    assert.match(source, /https:\/\/github\.com\/Team-silvortex\/cyanrex-lab\/issues\/4/);
    for (const stage of ["F0", "F1", "F2", "F3", "F4", "F5"])
      assert.ok(source.includes(stage), `${locale}: missing stage ${stage}`);
    for (const name of ["NodeFingerprint", "UnverifiedPeerCandidate", "DiscoveryPolicy", "WorkspaceGrant"])
      assert.ok(source.includes(name), `${locale}: missing boundary ${name}`);
    assert.match(await read(`docs/${locale}/reference-index.md`), /\]\(federation-discovery\.md\)/);
  }
});

test("the explicit F1c discovery CLI keeps process gates, inert output and opt-in isolated networking", async () => {
  const main = await read("engine/src/bin/cyanrex-discovery/main.rs");
  const source = (await sources("engine/src/bin/cyanrex-discovery")).join("\n");
  assert.match(main, /prepare_platform_process/);
  assert.match(main, /verify_before_runtime/);
  assert.match(main, /verify_current_thread/);
  assert.ok(main.indexOf("prepare_platform_process(&") < main.indexOf("let peers = discover_once("));
  assert.doesNotMatch(source, /\b(?:AuthService|SessionToken|TcpStream|TcpListener|UdpSocket|tokio|reqwest|Command)\b/);
  assert.doesNotMatch(source, /std::env::var|std::thread|\.connect\(|with_public_direct|with_relays/);
  assert.match(await read("engine/src/bin/cyanrex-discovery/output.rs"), /MAX_OUTPUT_BYTES: usize = 256 \* 1024/);
  for (const target of ["args", "output", ""]) {
    const name = target ? `federation_discovery_cli_${target}_tdd` : "federation_discovery_cli_tdd";
    assert.match(await read(`engine/tests/${name}.rs`), /#!\[cfg\(target_os = "linux"\)\]/);
  }
  const network = await read("engine/tests/federation_discovery_cli_network_tdd.rs");
  assert.match(network, /#!\[cfg\(target_os = "linux"\)\]/);
  assert.ok((network.match(/#\[test\]/g) || []).length > 0);
  assert.equal((network.match(/#\[test\]/g) || []).length, (network.match(/#\[ignore/g) || []).length);
  assert.match(await read("scripts/test-federation-mdns-netns.sh"), /--with-cli/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const name of ["F1c", "cyanrex-discovery", "advertisement_expires_at", "snapshot_only", "--with-cli"])
      assert.ok(text.includes(name), `${locale}: ${name}`);
  }
});

test("F1d announcement encoding stays pure and cannot widen the explicit browse socket", async () => {
  const announcement = await read("engine/src/services/federation_mdns/announcement.rs");
  assert.match(announcement, /pub struct MdnsAnnouncement/);
  assert.match(announcement, /pub fn encode_at\b/);
  assert.match(announcement, /pub fn encode_goodbye\b/);
  assert.doesNotMatch(announcement,
    /\b(?:tokio|reqwest|hyper|axum|sqlx|libc|AuthService|SessionToken|WorkspaceGrant|PgPool|TcpStream|TcpListener|UdpSocket|MdnsSocket|VerifiedInterface|SystemTime|Instant|Command)\b/);
  assert.doesNotMatch(announcement, /std::(?:fs|env|thread|time|process)|\.(?:connect|bind|resolve|select|send_query|send_to|recv_from)\s*\(/);
  const module = await read("engine/src/services/federation_mdns/mod.rs");
  assert.match(module, /mod announcement;/);
  assert.match(module, /pub use announcement::(?:\{[^}]*\bMdnsAnnouncement\b[^}]*\}|MdnsAnnouncement);/s);

  const socket = await read("engine/src/services/federation_mdns/socket.rs");
  const start = socket.indexOf("pub fn send_query(");
  const end = socket.indexOf("pub fn receive(", start);
  assert.ok(start >= 0 && end > start, "the existing query and receive method boundaries remain explicit");
  const sendQuery = socket.slice(start, end);
  const fixedQuery = /if\s+bytes\s*!=\s*super::browse_query\(\)\.as_slice\(\)\s*\{\s*return Err\(MdnsError::InvalidPacket\);\s*\}/;
  assert.match(sendQuery, fixedQuery);
  assert.ok(sendQuery.search(fixedQuery) < sendQuery.indexOf("libc::sendmsg("),
    "reject arbitrary bytes before the one query send");
  assert.doesNotMatch(socket, /\bMdnsAnnouncement\b|\bencode_(?:at|goodbye)\s*\(/);
  assert.deepEqual(socket.match(/pub fn (?:send|announce|advertise)[a-z_]*\(/g), ["pub fn send_query("]);

  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs"])
    assert.doesNotMatch(await read(file), /\bMdnsAnnouncement\b|\bencode_goodbye\b|federation_mdns::announcement/, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery", probeCliFiles)).join("\n"),
    /\bMdnsAnnouncement\b|\bencode_goodbye\b|federation_mdns::announcement/);
  assert.match(await read("engine/tests/federation_mdns_announcement_tdd.rs"), /#!\[cfg\(target_os = "linux"\)\]/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const name of ["F1d", "MdnsAnnouncement", "encode_at", "encode_goodbye"])
      assert.ok(text.includes(name), `${locale}: ${name}`);
  }
});

test("F1e probe preparation observes claims without granting uniqueness or a sending path", async () => {
  const probe = await read("engine/src/services/federation_mdns/announcement/probe.rs");
  for (const method of ["encode_probe_at", "observe_probe_packet"])
    assert.match(probe, new RegExp(`pub fn ${method}\\b`));
  const observation = probe.match(/pub enum ProbeObservation\s*\{([^}]*)\}/s);
  assert.ok(observation, "the public result remains an observation, not a name-ownership permit");
  const variants = observation[1].replace(/\/\/[^\n]*/g, "").split(",").map(value => value.trim()).filter(Boolean);
  assert.deepEqual(variants, ["NoRelevantClaim", "ExistingResponse", "CompetingProbe"]);
  const forbidden = /\b(?:tokio|reqwest|hyper|axum|sqlx|libc|AuthService|SessionToken|WorkspaceGrant|PgPool|TcpStream|TcpListener|UdpSocket|MdnsSocket|MdnsConfig|VerifiedInterface|SystemTime|Instant|Command|DiscoveryPolicy|UnverifiedPeerCandidate)\b/;
  assert.doesNotMatch(probe, forbidden);
  assert.doesNotMatch(probe,
    /std::(?:fs|env|thread|time|process)|\.(?:connect|bind|resolve|select|send_query|send_to|recv_from)\s*\(|\bparse_response\s*\(/);

  const wire = await read("engine/src/services/federation_mdns/wire.rs");
  assert.match(await read("engine/src/services/federation_mdns/mod.rs"), /^mod wire;/m);
  assert.doesNotMatch(wire, forbidden);
  assert.doesNotMatch(wire,
    /\b(?:PeerAdvertisement|MdnsAnnouncement|ProbeObservation|LanCandidateCache)\b|std::(?:fs|env|thread|time|process)|\.(?:connect|bind|resolve|select|send_query|send_to|recv_from)\s*\(/);

  const probeApis = /\b(?:encode_probe_at|observe_probe_packet|ProbeObservation)\b/;
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/socket.rs"])
    assert.doesNotMatch(await read(file), probeApis, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"), probeApis);
  assert.match(await read("engine/tests/federation_mdns_probe_tdd.rs"), /#!\[cfg\(target_os = "linux"\)\]/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const name of ["F1e", "encode_probe_at", "observe_probe_packet", "NoRelevantClaim"])
      assert.ok(text.includes(name), `${locale}: ${name}`);
  }
});

test("F1f unicast reception is an explicit exact-local receive-only socket, never a browse or startup widening", async () => {
  const unicast = await read("engine/src/services/federation_mdns/socket/unicast.rs");
  const unicastCode = unicast.replace(/\/\/[^\n]*|\/\*[\s\S]*?\*\//g, "");
  const socket = await read("engine/src/services/federation_mdns/socket.rs");
  const module = await read("engine/src/services/federation_mdns/mod.rs");
  assert.match(unicast, /pub struct MdnsUnicastReceiver\s*\{/);
  assert.match(unicast, /\bfd:\s*OwnedFd\b/);
  assert.match(unicast, /\binterface:\s*VerifiedInterface\b/);
  assert.match(unicast, /pub fn open\(interface:\s*&VerifiedInterface\)/);
  assert.match(unicast, /pub fn receive\(&mut self\)/);
  assert.match(unicast, /sin_addr\s*=\s*in_addr\(interface\.local_address\(\)\)/);
  assert.match(unicast, /libc::bind\(/);
  assert.match(unicast, /IP_PKTINFO/);
  assert.match(unicast, /IP_RECVTTL/);
  assert.match(unicast, /IP_MULTICAST_ALL,\s*&0_i32/);
  assert.match(unicast, /verify_unchanged\(\)/);
  assert.match(unicast, /super::receive\(&self\.fd,\s*&self\.interface,\s*self\.interface\.local_address\(\)\)/);
  assert.doesNotMatch(unicastCode,
    /\b(?:SO_REUSEADDR|SO_REUSEPORT|INADDR_ANY|UNSPECIFIED|IP_ADD_MEMBERSHIP|IP_MULTICAST_IF|IP_MULTICAST_TTL|SO_BINDTODEVICE)\b/);
  assert.doesNotMatch(unicastCode, /\blibc::(?:send\w*|connect)\s*\(|\bpub(?:\([^)]*\))?\s+fn\s+(?:send\w*|connect|as_raw_fd|into_raw_fd)\s*\(/);
  assert.doesNotMatch(unicastCode, /\bimpl\s+(?:AsRawFd|IntoRawFd|AsFd)\b|\bpub\s+fd\s*:/);
  assert.deepEqual(unicastCode.match(/pub fn [a-z_]+\(/g), ["pub fn open(", "pub fn receive("]);
  assert.match(socket, /mod unicast;/);
  assert.match(module, /pub use socket::\{[^}]*\bMdnsUnicastReceiver\b[^}]*\};/s);

  const browseReceive = socket.slice(socket.indexOf("pub fn receive("), socket.indexOf("#[derive(Debug)]\nstruct Metadata"));
  assert.match(browseReceive, /receive\(&self\.fd,\s*&self\.interface,\s*GROUP\)/);
  assert.match(socket, /metadata\.destination\s*!=\s*expected_destination/);
  assert.match(socket, /sin_addr\s*=\s*in_addr\(GROUP\)/);
  assert.match(socket, /if\s+bytes\s*!=\s*super::browse_query\(\)\.as_slice\(\)/);

  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs"])
    assert.doesNotMatch(await read(file), /\bMdnsUnicastReceiver\b|socket::unicast/, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"),
    /\bMdnsUnicastReceiver\b|socket::unicast/);
});

test("F1g explicit probing has bounded typed private I/O and no publication or ownership authority", async () => {
  const runtime = await read("engine/src/services/federation_mdns/probing.rs");
  const io = await read("engine/src/services/federation_mdns/socket/probing.rs");
  const limits = await read("engine/src/services/federation_mdns/probing/limits.rs");
  const schedule = await read("engine/src/services/federation_mdns/probing/schedule.rs");
  const module = await read("engine/src/services/federation_mdns/mod.rs");
  assert.match(module, /pub use probing::\{probe_once, ProbeRunObservation\};/);
  const observation = runtime.match(/pub enum ProbeRunObservation\s*\{([^}]*)\}/s);
  assert.ok(observation);
  assert.deepEqual(observation[1].split(",").map(value => value.trim()).filter(Boolean),
    ["NoConflictObserved", "ExistingResponse", "CompetingProbe"]);
  assert.match(runtime, /cancelled\(cancellation\)\?;[\s\S]*Limits::new[\s\S]*ProbeIo::open/);
  assert.match(runtime, /getrandom\([\s\S]*GRND_NONBLOCK/);
  assert.match(runtime, /for _ in 0\.\.8/);
  assert.match(runtime, /remaining_wait/);
  assert.match(runtime, /limits\.admit/);
  assert.match(io, /let multicast = MdnsSocket::open\(interface\)\?;\s*let unicast = MdnsUnicastReceiver::open\(interface\)\?;/);
  assert.match(io, /fn send_probe\(\s*&self,\s*announcement: &MdnsAnnouncement,\s*now: u64,?\s*\)/);
  assert.ok(io.indexOf("encode_probe_at(now)") < io.indexOf("libc::sendmsg("));
  assert.ok(io.indexOf("check_mtu(bytes.len()") < io.indexOf("libc::sendmsg("));
  for (const name of ["IP_PMTUDISC_DO", "SIOCGIFMTU", "MSG_DONTWAIT", "libc::poll", "unicast_first"])
    assert.ok(io.includes(name), name);
  assert.doesNotMatch(io, /\bpub\s+fn\s+send|encode_at\(|encode_goodbye\(|libc::connect\(/);
  for (const source of [runtime, io])
    assert.doesNotMatch(source, /\b(?:AuthService|SessionToken|WorkspaceGrant|PgPool|TcpStream|TcpListener|tokio|reqwest)\b|std::(?:thread|process|env|fs)/);
  for (const source of [limits, schedule])
    assert.doesNotMatch(source, /\blibc\b|SystemTime|Instant|std::(?:thread|process|env|fs)|pub fn /);
  const entrypoints = /\b(?:probe_once|ProbeRunObservation|ProbeIo)\b/;
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs"])
    assert.doesNotMatch(await read(file), entrypoints, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery", probeCliFiles)).join("\n"), entrypoints);
  assert.match(await read("scripts/test-federation-mdns-netns.sh"), /--with-probe/);
  const network = await read("engine/tests/federation_mdns_probing_network_tdd.rs");
  assert.ok((network.match(/#\[test\]/g) || []).length > 0);
  assert.equal((network.match(/#\[test\]/g) || []).length, (network.match(/#\[ignore/g) || []).length);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const name of ["F1g", "probe_once", "ProbeRunObservation", "NoConflictObserved", "--with-probe"])
      assert.ok(text.includes(name), `${locale}: ${name}`);
  }
});

test("F1h finite retry history cannot accept observations as publication authority or bypass frozen lifetime", async () => {
  const controller = await read("engine/src/services/federation_mdns/controller.rs");
  const gate = await read("engine/src/services/federation_mdns/controller/gate.rs");
  const lifetime = await read("engine/src/services/federation_mdns/controller/lifetime.rs");
  const runtime = await read("engine/src/services/federation_mdns/probing.rs");
  const module = await read("engine/src/services/federation_mdns/mod.rs");
  assert.match(module, /pub use controller::\{MdnsProbeController, ProbeControllerState\};/);
  assert.deepEqual(controller.match(/pub fn [a-z_]+\(/g),
    ["pub fn new(", "pub fn state(", "pub fn attempts_started(", "pub fn retry_after(", "pub fn attempt(", "pub fn attempt_retained("]);
  assert.doesNotMatch(controller, /impl Clone for MdnsProbeController|pub fn (?:finish|publish|announce|set_|renew)/);
  assert.match(controller, /self\.gate\.begin\(elapsed\)\?;[\s\S]*VerifiedInterface::inspect/);
  for (const required of ["lifetime.check(elapsed, wall)", "gate.check_in_flight(elapsed)",
    "interface.verify_unchanged()", "super::probing::probe_guarded", "self.gate.close()"])
    assert.ok(controller.includes(required), required);
  assert.match(runtime, /pub\(super\) fn probe_guarded\(/);
  const guarded = runtime.slice(runtime.indexOf("pub(super) fn guarded_check"), runtime.indexOf("impl Run<"));
  assert.match(guarded, /guard\(\)\?;\s*let result = body\(\)\?;\s*guard\(\)\?;\s*Ok\(result\)/);
  const runCheck = runtime.slice(runtime.indexOf("fn check(&mut self)"), runtime.indexOf("fn observe("));
  assert.match(runCheck, /cancelled\(cancellation\)\?;\s*guarded_check\(\*guard, \|\|/);
  assert.match(runtime, /probe_guarded\(config, announcement, cancellation, &mut \|\| Ok\(\(\)\)\)/);
  for (const required of ["MAX_ATTEMPTS: u8 = 4", "Duration::from_secs(30)", "Duration::from_secs(5)", "CoolingDown", "checked_add(RETRY_DELAY)"])
    assert.ok(gate.includes(required), required);
  assert.match(lifetime, /elapsed >= self\.lease/);
  assert.match(lifetime, /wall >= self\.expiry/);
  for (const source of [gate, lifetime])
    assert.doesNotMatch(source, /\blibc\b|\b(?:SystemTime|Instant)\b|std::(?:thread|process|env|fs)|pub fn /);
  assert.doesNotMatch(controller,
    /\b(?:AuthService|SessionToken|WorkspaceGrant|PgPool|TcpStream|TcpListener|UdpSocket|tokio|reqwest|ProbeIo)\b|std::(?:thread|process|env|fs)|encode_at\(|encode_goodbye\(/);
  const names = /\b(?:MdnsProbeController|ProbeControllerState)\b/;
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs"])
    assert.doesNotMatch(await read(file), names, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"), names);
  const network = await read("engine/tests/federation_mdns_controller_network_tdd.rs");
  assert.ok((network.match(/#\[test\]/g) || []).length > 0);
  assert.equal((network.match(/#\[test\]/g) || []).length, (network.match(/#\[ignore/g) || []).length);
  assert.match(await read("scripts/test-federation-mdns-netns.sh"), /--with-controller/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1h", "MdnsProbeController", "CoolingDown", "retry_after", "--with-controller"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});

test("F1i quiet handoff retains original I/O and bounds without becoming a sender or controller permit", async () => {
  const runtime = await read("engine/src/services/federation_mdns/probing.rs");
  const retained = await read("engine/src/services/federation_mdns/probing/retained.rs");
  const bounds = (await read("engine/src/services/federation_mdns/probing/retained/bounds.rs")).split("#[cfg(test)]")[0];
  const window = await read("engine/src/services/federation_mdns/probing/retained/window.rs");
  const module = await read("engine/src/services/federation_mdns/mod.rs");
  assert.match(module, /pub use probing::\{probe_retained_once, MdnsRetainedProbe, RetainedProbeOutcome\};/);
  assert.match(runtime, /pub fn probe_retained_once<'a>\(/);
  assert.match(runtime, /run_probe\(config, announcement, cancellation, &mut \|\| Ok\(\(\)\)\)/);
  assert.match(runtime, /Ok\(run_probe\(config, announcement, cancellation, guard\)\?\.observation\(\)\)/);
  assert.match(runtime, /MdnsRetainedProbe::from_run\(\s*run,\s*io,\s*sample\.elapsed,?\s*\)/);
  assert.match(retained, /pub struct MdnsRetainedProbe<'a>\s*\{\s*live:\s*Option<Live<'a>>/);
  assert.deepEqual(retained.match(/pub fn [a-z_]+\(/g), ["pub fn review(", "pub fn close("]);
  for (const required of ["started: run.started", "ReviewBounds::from_probe(run.limits, quiet_at)", "announcement: run.announcement",
    "interface: run.interface.clone()", "self.live.take()", "limits: live.bounds.limits", "live.bounds.limits = run.limits",
    "run.drain(&mut live.io, 3)", "run.check()?", "self.live = Some(live)", "self.live = None"])
    assert.ok(retained.includes(required), required);
  assert.doesNotMatch(retained, /derive\([^)]*Clone|impl Clone|\bpub\s+(?:live|io|interface|limits|started)\s*:/);
  assert.doesNotMatch(retained,
    /VerifiedInterface::inspect|ProbeIo::open|Limits::new|encode_at\(|encode_goodbye\(|libc::|std::(?:thread|env|fs|process)|\b(?:AuthService|SessionToken|WorkspaceGrant|PgPool|TcpStream|TcpListener)\b/);
  for (const required of ["checked_add(Duration::from_millis(250))", "elapsed < self.last", "elapsed >= self.deadline", "self.closed = true"])
    assert.ok(window.includes(required), required);
  assert.doesNotMatch(window, /\b(?:SystemTime|Instant|libc)\b|std::(?:thread|env|fs|process)|pub fn /);
  assert.match(bounds, /limits,\s*window: ReviewWindow::new\(quiet_at\)\?/);
  assert.doesNotMatch(bounds, /Limits::new|\b(?:SystemTime|Instant|libc)\b|std::(?:thread|env|fs|process)|pub fn /);
  const names = /\b(?:probe_retained_once|MdnsRetainedProbe|RetainedProbeOutcome)\b/;
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs"])
    assert.doesNotMatch(await read(file), names, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"), names);
  const network = await read("engine/tests/federation_mdns_retained_network_tdd.rs");
  assert.ok((network.match(/#\[test\]/g) || []).length > 0);
  assert.equal((network.match(/#\[test\]/g) || []).length, (network.match(/#\[ignore/g) || []).length);
  assert.match(await read("scripts/test-federation-mdns-netns.sh"), /--with-retained/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1i", "probe_retained_once", "MdnsRetainedProbe", "RetainedProbeOutcome", "--with-retained"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});

test("F1j explicit controller handoff preserves named original bounds without creating a sender", async () => {
  const controller = await read("engine/src/services/federation_mdns/controller.rs");
  assert.match(controller, /pub fn attempt_retained\(/);
  const retained = await read("engine/src/services/federation_mdns/probing/retained.rs");
  const controlSources = (await sources("engine/src/services/federation_mdns/controller")).join("\n");
  assert.match(controlSources, /struct ControllerReviewBounds/);
  const bounds = (await read("engine/src/services/federation_mdns/controller/review.rs")).split("#[cfg(test)]")[0];
  for (const required of ["origin: Instant", "lifetime: Lifetime", "watermark: completed", "self.origin.elapsed()",
    "elapsed < self.watermark", "self.lifetime.check(elapsed, wall)", "elapsed >= MAX_LIFETIME", "self.closed = true"])
    assert.ok(bounds.includes(required), required);
  assert.doesNotMatch(bounds, /Lifetime::new|Instant::now|derive\([^)]*Clone|impl Clone/);
  assert.match(controller, /lifetime\.take\(\)/);
  assert.match(retained, /controller: Option<ControllerReviewBounds>/);
  assert.match(retained, /controller: None/);
  assert.match(retained, /controller\.check\(\)\?;/);
  assert.match(retained, /if live\.controller\.is_some\(\)\s*\{\s*return Err\(MdnsError::Closed\)/);
  assert.doesNotMatch(retained + controlSources, /Box\s*<\s*dyn\s+Fn|Arc\s*<\s*dyn\s+Fn/);
  assert.deepEqual(retained.match(/pub fn [a-z_]+\(/g), ["pub fn review(", "pub fn close("]);
  const legacyAttempt = controller.slice(controller.indexOf("pub fn attempt("), controller.indexOf("pub fn attempt_retained("));
  assert.match(legacyAttempt, /attempt_inner\(cancellation\)/);
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs"])
    assert.doesNotMatch(await read(file), /attempt_retained|ControllerReviewBounds/, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"),
    /attempt_retained|ControllerReviewBounds/);
  const runner = await read("scripts/test-federation-mdns-netns.sh");
  assert.match(runner, /--with-controller-retained/);
  assert.match(runner, /task_mdns_targets=\(--test federation_mdns_network_tdd\)/);
  assert.match(runner, /if \[\[ "\$task_mdns_with_controller_retained" == 1 \]\]; then\s*task_mdns_targets\+=\(--test federation_mdns_controller_retained_network_tdd\)/);
  const network = await read("engine/tests/federation_mdns_controller_retained_network_tdd.rs");
  assert.match(network, /#!\[cfg\(target_os = "linux"\)\]/);
  assert.ok((network.match(/#\[test\]/g) || []).length > 0);
  assert.equal((network.match(/#\[test\]/g) || []).length, (network.match(/#\[ignore/g) || []).length);
  const mutation = "a_renamed_controller_retained_interface_closes_and_releases_original_channels";
  assert.ok(runner.includes(`--exact ${mutation}`));
  assert.ok(network.includes(`fn ${mutation}()`));
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1j", "attempt_retained", "--with-controller-retained"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});

test("F1k probe CLI uses one gated attempt and fixed observations without publication or automatic modes", async () => {
  const main = await read("engine/src/bin/cyanrex-discovery/main.rs");
  assert.ok(main.includes('"probe-lan"'));
  const probe = await read(probeCliFiles[1]);
  for (const required of ["prepare_platform_process", "verify_before_runtime", "prepare_at(", "probe_once(", "verify_current_thread"])
    assert.ok(probe.includes(required), required);
  assert.ok(probe.indexOf("verify_before_runtime") < probe.indexOf("prepare_at("));
  assert.ok(probe.indexOf("prepare_at(") < probe.indexOf("probe_once("));
  assert.equal((probe.match(/\bprobe_once\(/g) || []).length, 1);
  const input = await read(probeCliFiles[0]);
  assert.match(input, /PeerAdvertisement::parse\(/);
  assert.match(input, /MdnsAnnouncement::new\(/);
  assert.match(input, /take\(24\)/);
  assert.doesNotMatch(input, /SystemTime|Instant|std::(?:fs|env|process|thread)|serde_json::(?:Value|from_value)/);
  const output = await read(probeCliFiles[2]);
  for (const required of ["unverified_lan_probe_observation", "observation_only", "name_ownership_verified",
    "publication_permitted", "announcement_sent", "no_conflict_observed", "existing_response", "competing_probe"])
    assert.ok(output.includes(required), required);
  const cli = (await sources("engine/src/bin/cyanrex-discovery")).join("\n");
  assert.doesNotMatch(cli, /\b(?:MdnsProbeController|MdnsRetainedProbe|RetainedProbeOutcome|ProbeIo|MdnsUnicastReceiver|MdnsSocket)\b/);
  assert.doesNotMatch(cli, /encode_at\(|encode_goodbye\(|send_probe\(|libc::|std::(?:fs|thread)|std::env::var|std::io::stdin/);
  const legacy = (await sources("engine/src/bin/cyanrex-discovery", probeCliFiles)).join("\n");
  assert.doesNotMatch(legacy, /\bprobe_once\(|\bProbeRunObservation\b|\bMdnsAnnouncement\b/);
  const network = await read("engine/tests/federation_probe_cli_network_tdd.rs");
  assert.ok((network.match(/#\[test\]/g) || []).length > 0);
  assert.equal((network.match(/#\[test\]/g) || []).length, (network.match(/#\[ignore/g) || []).length);
  const runner = await read("scripts/test-federation-mdns-netns.sh");
  assert.match(runner, /--with-probe-cli/);
  assert.match(runner, /task_mdns_targets=\(--test federation_mdns_network_tdd\)/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1k", "probe-lan", "--with-probe-cli", "publication_permitted"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});

test("F1l initial announcement scheduling stays caller-driven preparation without a publication path", async () => {
  const schedule = await read("engine/src/services/federation_mdns/announcement/schedule.rs");
  for (const method of ["initial_schedule", "step", "record_completion", "close"])
    assert.match(schedule, new RegExp(`pub fn ${method}\\b`));
  for (const name of ["MdnsAnnouncementSchedule", "AnnouncementStep", "Prepare", "Complete", "Wait"])
    assert.ok(schedule.includes(name), name);
  assert.match(schedule, /_?announcement: &'a MdnsAnnouncement/);
  assert.doesNotMatch(schedule, /#\[derive\([^)]*Clone[^)]*\)\]\s*pub struct MdnsAnnouncementSchedule|impl(?:<[^>]*>)? Clone for MdnsAnnouncementSchedule/);
  assert.doesNotMatch(schedule,
    /\b(?:SystemTime|Instant|libc|tokio|reqwest|UdpSocket|MdnsSocket|ProbeIo|MdnsRetainedProbe|RetainedProbeOutcome|ProbeRunObservation|MdnsProbeController|AuthService|SessionToken|WorkspaceGrant)\b/);
  assert.doesNotMatch(schedule,
    /std::(?:fs|env|thread|process)|\.(?:connect|bind|send_to|recv_from|send_query)\s*\(|\b(?:encode_at|encode_goodbye|encode_probe_at)\s*\(/);
  for (const name of ["MdnsAnnouncementSchedule", "AnnouncementStep"])
    assert.ok((await read("engine/src/services/federation_mdns/mod.rs")).includes(name), name);
  const prepared = /\b(?:initial_schedule|MdnsAnnouncementSchedule|AnnouncementStep)\b/;
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs", "engine/src/services/federation_mdns/probing.rs",
    "engine/src/services/federation_mdns/probing/retained.rs", "engine/src/services/federation_mdns/controller.rs",
    "engine/src/services/federation_mdns/socket.rs"])
    assert.doesNotMatch(await read(file), prepared, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"), prepared);
  assert.match(await read("engine/tests/federation_mdns_announcement_schedule_tdd.rs"), /#!\[cfg\(target_os = "linux"\)\]/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1l", "initial_schedule", "record_completion", "AnnouncementStep"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});

test("F1m response comparison is pure and separate from conservative probing or live publication", async () => {
  const conflict = await read("engine/src/services/federation_mdns/announcement/conflict.rs");
  assert.match(conflict, /pub fn observe_response_conflict\(/);
  const observation = conflict.match(/pub enum ResponseConflictObservation\s*\{([^}]*)\}/s);
  assert.ok(observation);
  assert.deepEqual(observation[1].split(",").map(value => value.trim()).filter(Boolean),
    ["NoConflictingRecord", "ConflictingRecord"]);
  assert.match(conflict, /wire::read_message\(bytes\)/);
  assert.match(conflict, /FrozenRecords::new\(self\)/);
  assert.match(conflict, /frozen\.match_record\(bytes, record\)/);
  assert.match(conflict, /conflicting \|= kind != RecordKind::Ptr/);
  const records = await read("engine/src/services/federation_mdns/announcement/records.rs");
  assert.match(records, /wire::complete_name\(bytes, record\.start \+ 6, record\.end\)/);
  assert.doesNotMatch(conflict + records,
    /\b(?:SystemTime|Instant|Duration|libc|tokio|reqwest|UdpSocket|MdnsSocket|ProbeIo|MdnsRetainedProbe|ProbeObservation|AuthService|SessionToken|WorkspaceGrant|DiscoveryPolicy)\b/);
  assert.doesNotMatch(conflict + records,
    /std::(?:fs|env|thread|process)|\b(?:decode_txt|parse_response|observe_probe_packet|encode_at|encode_goodbye)\s*\(|return Ok\(ResponseConflictObservation::ConflictingRecord\)/);
  assert.ok((await read("engine/src/services/federation_mdns/mod.rs")).includes("ResponseConflictObservation"));
  const comparison = /\b(?:observe_response_conflict|ResponseConflictObservation)\b/;
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs", "engine/src/services/federation_mdns/probing.rs",
    "engine/src/services/federation_mdns/probing/retained.rs", "engine/src/services/federation_mdns/controller.rs",
    "engine/src/services/federation_mdns/packet.rs", "engine/src/services/federation_mdns/socket.rs",
    "engine/src/services/federation_mdns/announcement/probe.rs", "engine/src/services/federation_mdns/announcement/schedule.rs"])
    assert.doesNotMatch(await read(file), comparison, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"), comparison);
  assert.match(await read("engine/tests/federation_mdns_response_conflict_tdd.rs"), /#!\[cfg\(target_os = "linux"\)\]/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1m", "observe_response_conflict", "ResponseConflictObservation", "NoConflictingRecord"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});

test("F1n query selection retains explicit context and shared validation without a sender", async () => {
  const query = await read("engine/src/services/federation_mdns/announcement/query.rs");
  assert.match(query, /pub fn plan_query\(/);
  for (const name of ["MdnsQueryRecordSet", "MdnsQueryContext", "MdnsQueryPlan"])
    assert.ok(query.includes(name) && (await read("engine/src/services/federation_mdns/mod.rs")).includes(name), name);
  for (const field of ["ptr", "srv", "txt", "a", "truncated", "unicast_question", "authority_records", "legacy_port", "direct_unicast", "no_questions"])
    assert.match(query, new RegExp(`pub ${field}: bool`));
  for (const required of ["wire::read_message(bytes)", "self.ttl_at(now)", "FrozenRecords::new(self)",
    "frozen.match_record(bytes, record)", "Section::Answer", "source_port == 0", "source_port != 5353",
    "direct_unicast: !multicast_destination", "no_questions: message.questions.is_empty()",
    "suppressed: known_suppressed.intersect(requested)", "MdnsQueryPlan::RequiresContext(context)"])
    assert.ok(query.includes(required), required);
  const analysis = query.slice(query.indexOf("pub(super) fn analyze_query("));
  assert.ok(analysis.includes("QueryAnalysis::RequiresContext(context)"));
  assert.ok(analysis.indexOf("frozen.match_record(bytes, record)") < analysis.indexOf("QueryAnalysis::RequiresContext(context)"));
  assert.match(query.slice(0, query.indexOf("pub(super) fn analyze_query(")),
    /self\.analyze_query\(bytes, source_port, multicast_destination, now\)/);
  assert.doesNotMatch(query,
    /\b(?:SystemTime|Instant|libc|tokio|reqwest|UdpSocket|MdnsSocket|ProbeIo|VerifiedInterface|AuthService|SessionToken|WorkspaceGrant)\b|std::(?:fs|env|thread|process)|\b(?:encode_at|encode_goodbye|parse_response|decode_txt)\s*\(/);
  const names = /\b(?:plan_query|MdnsQueryPlan|MdnsQueryContext|MdnsQueryRecordSet)\b/;
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs", "engine/src/services/federation_mdns/probing.rs",
    "engine/src/services/federation_mdns/probing/retained.rs", "engine/src/services/federation_mdns/controller.rs",
    "engine/src/services/federation_mdns/packet.rs", "engine/src/services/federation_mdns/socket.rs",
    "engine/src/services/federation_mdns/announcement/probe.rs", "engine/src/services/federation_mdns/announcement/schedule.rs"])
    assert.doesNotMatch(await read(file), names, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"), names);
  assert.match(await read("engine/tests/federation_mdns_query_plan_tdd.rs"), /#!\[cfg\(target_os = "linux"\)\]/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1n", "plan_query", "RequiresContext", "suppressed"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});

test("F1o response preparation reanalyzes raw input once and encodes only bounded frozen records", async () => {
  const response = await read("engine/src/services/federation_mdns/announcement/response.rs");
  const query = await read("engine/src/services/federation_mdns/announcement/query.rs");
  assert.match(response, /pub fn prepare_query_response\(/);
  for (const name of ["NoMatchingQuestion", "AllDirectAnswersSuppressed", "RequiresContext", "Packet(Vec<u8>)"])
    assert.ok(response.includes(name), name);
  assert.equal(response.match(/self\.analyze_query\(/g)?.length, 1);
  assert.match(response, /bytes: &\[u8\]/);
  assert.doesNotMatch(response, /(?:plan|requested|suppressed): (?:&)?MdnsQuery(?:Plan|RecordSet)/);
  assert.match(query, /pub\(super\) enum QueryAnalysis/);
  assert.equal(query.match(/wire::read_message\(bytes\)/g)?.length, 1);
  assert.equal(query.match(/self\.ttl_at\(now\)/g)?.length, 1);
  for (const required of ["requested.difference(known_suppressed)", "srv: answers.ptr", "txt: answers.ptr",
    "a: answers.ptr || answers.srv", ".difference(known_suppressed)", ".difference(answers)",
    "self.frozen_record(&mut packet, kind, class, ttl)", "Packet(Vec::new())", "Ok(MdnsQueryResponse::Packet(packet.0))"])
    assert.ok(response.includes(required), required);
  assert.ok(response.indexOf("AllDirectAnswersSuppressed") < response.indexOf("let dependencies"));
  assert.doesNotMatch(response,
    /\b(?:SystemTime|Instant|libc|tokio|reqwest|UdpSocket|MdnsSocket|ProbeIo|VerifiedInterface|AuthService|SessionToken|WorkspaceGrant)\b|std::(?:fs|env|thread|process)|\b(?:ttl_at|plan_query|read_message|encode_at|encode_goodbye|parse_response|decode_txt)\s*\(/);
  const prepared = /\b(?:prepare_query_response|MdnsQueryResponse)\b/;
  assert.ok((await read("engine/src/services/federation_mdns/mod.rs")).includes("MdnsQueryResponse"));
  for (const file of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/src/platform_host/mod.rs", "engine/src/bin/cyanrex-platform/main.rs",
    "engine/src/services/federation_mdns/runtime.rs", "engine/src/services/federation_mdns/probing.rs",
    "engine/src/services/federation_mdns/probing/retained.rs", "engine/src/services/federation_mdns/controller.rs",
    "engine/src/services/federation_mdns/packet.rs", "engine/src/services/federation_mdns/socket.rs",
    "engine/src/services/federation_mdns/announcement/probe.rs", "engine/src/services/federation_mdns/announcement/schedule.rs"])
    assert.doesNotMatch(await read(file), prepared, file);
  assert.doesNotMatch((await sources("engine/src/bin/cyanrex-discovery")).join("\n"), prepared);
  assert.match(await read("engine/tests/federation_mdns_query_response_tdd.rs"), /#!\[cfg\(target_os = "linux"\)\]/);
  for (const locale of ["en", "zh-CN"]) {
    const text = await read(`docs/${locale}/federation-discovery.md`);
    for (const required of ["F1o", "prepare_query_response", "AllDirectAnswersSuppressed", "NoMatchingQuestion"])
      assert.ok(text.includes(required), `${locale}: ${required}`);
  }
});
