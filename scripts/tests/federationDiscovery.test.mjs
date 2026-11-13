import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
async function sources(directory) {
  const result = [];
  for (const item of await readdir(new URL(`${directory}/`, root), { withFileTypes: true })) {
    const file = `${directory}/${item.name}`;
    if (item.isDirectory()) result.push(...await sources(file));
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
