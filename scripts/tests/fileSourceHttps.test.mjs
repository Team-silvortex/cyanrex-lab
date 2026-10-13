import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const dir = "engine/tests/file_delivery/source_https/";
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = s => s.replace(/\s+/g, "");

// Inventory and boundaries only. The six-case explicit runner executes actual SQL/TLS behavior.
test("combined Source HTTPS has three explicit external cases without duplicating old Source tests", async () => {
  const main = await read(`${dir}mod.rs`);
  const parent = await read("engine/tests/file_delivery/source.rs");
  assert.match(parent, /#\[path = "source_https\/mod\.rs"\]\s*mod source_https;/);
  assert.match(await read("engine/tests/file_delivery_tdd.rs"), /#!\[cfg\(unix\)\]/);
  const names = [];
  let count = 0;
  for (const file of await readdir(new URL(dir, root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`${dir}${file}`), module = file.replace(/\.rs$/, "");
    const ignored = [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_openssl_\w+)\(/g)].map(m => m[1]);
    const actual = (s.match(/#\[(?:tokio::)?test(?:\([^\]]*\))?\]/g) || []).length;
    count += actual;
    assert.equal(actual, ignored.length, "no default cases pretend to execute external boundaries");
    if (actual) {
      assert.match(main, new RegExp(`\\bmod ${module};`));
      assert.match(s, /tokio::test\(flavor\s*=\s*"multi_thread",\s*worker_threads\s*=\s*2\)/);
    }
    names.push(...ignored.map(name => `source::source_https::${module}::${name}`));
  }
  assert.equal(count, 3);
  const runner = await read("scripts/test-file-delivery.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(inventory.length, 6);
  assert.equal(new Set(inventory).size, 6);
  assert.deepEqual(inventory.filter(name => name.includes("::source_https::")).sort(), names.sort());
  for (const pattern of [/command -v openssl/, /CYANREX_TEST_DATABASE_URL:\?/, /--test file_delivery_tdd/, /--ignored --list/, /--ignored --exact --nocapture/, /grep -Fx/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-delivery.sh"));
});

test("native TLS receives original requests and delegates them to one real composed router", async () => {
  const fixture = code(await read(`${dir}fixture.rs`));
  for (const phrase of ["build_file_distribution_router", "FileDistributionHttpState", "self.source.workspace.clone()", "source.clone()", "FileChunkClientConfig::default()", "trusted_root_pem"]) assert.ok(fixture.includes(phrase), phrase);
  assert.doesNotMatch(fixture, /FileChunkTransport::LoopbackHttp|new_loopback/);
  const s = code(await read(`${dir}server.rs`));
  for (const phrase of ["router: Router", "router.oneshot(request)", "observed.method", "observed.uri", "observed.headers", "observed.body", "HoldAt::BeforeDispatch", "state.dispatched.fetch_add", "s_server", "-quiet", "-ign_eof"]) assert.ok(compact(s).includes(compact(phrase)), phrase);
  const flat = compact(s);
  assert.ok(flat.indexOf("gate.wait(&state.control)?", flat.indexOf("HoldAt::BeforeDispatch")) < flat.indexOf("router.oneshot(request)"));
  const parser = code(await read(`${dir}request.rs`));
  for (const phrase of ["headers.append", "value.set_sensitive(true)", "8192", "pending.drain"]) assert.ok(parser.includes(phrase), phrase);
  assert.doesNotMatch(s + parser, /Reply::good|bytes::manifest|bytes::byte|try_from_uri|build_file_chunk_read_router|build_file_download_manifest_router|danger_accept|sqlx::/);
});

test("actual response owners survive native writes and shutdown interrupts owned dispatch waits", async () => {
  const s = code(await read(`${dir}server.rs`));
  for (const phrase of ["response.into_parts()", "parts.status", "parts.headers", "body.size_hint().exact()", "body.frame()", "frame.into_data()", "drop(data)", "written != length", "prefix.is_some()", "until_stopped", "biased;", "Duration::from_secs(20)", "gate.release()", "process.kill()", "process.wait()", "worker.join()", "impl Drop for SourceTlsServer"]) assert.ok(compact(s).includes(compact(phrase)), phrase);
  assert.doesNotMatch(s, /flat_map|collect::<Vec|to_bytes\(|to_vec\(|tokio::spawn\(/);
  assert.ok(s.indexOf("process: Mutex::new(Some(process))") < s.indexOf("native_io::nonblocking(&incoming)"));
  const wait = s.slice(s.indexOf("pub async fn wait_responded"), s.indexOf("pub fn finish"));
  for (const phrase of ["Duration::from_secs(10)", "responded.load", "actual <= expected", "actual == expected", "yield_now().await"]) assert.ok(wait.includes(phrase), phrase);
  for (const [name, peer, count] of [["acceptance", "peer", 6], ["cancellation", "healthy", 2]]) {
    const testCase = await read(`${dir}${name}.rs`);
    const waiting = testCase.indexOf(`${peer}.wait_responded(${count}).await`);
    assert.ok(waiting >= 0 && waiting < testCase.indexOf(`let tls = ${peer}.finish()`));
  }
  const io = code(await read(`${dir}native_io.rs`));
  for (const phrase of ["watch::Sender<bool>", "send_replace(true)", "borrow_and_update", "libc::O_NONBLOCK", "libc::poll", "deadline", "received.fetch_add"]) assert.ok(io.includes(phrase), phrase);
  assert.doesNotMatch(io + s, /sudo|update-ca-certificates|std::env::var|println!/);
});

test("combined cases keep real grants bytes original ACK prefixes and selected-file oracles", async () => {
  const cases = (await Promise.all(["acceptance", "revocation", "cancellation", "support"].map(name => read(`${dir}${name}.rs`)))).join("\n");
  for (const phrase of ["FILE_TRANSFER_CHUNK_BYTES + 17", "publish(&f.source", "fetch_granted_and_save_controlled", "observe_output", "assert_output", "f.source.sharing.snapshot()", "recipient_delta", ".revoke(", "Kind::Chunk(1)", "HoldAt::BeforeDispatch", "HoldAt::Headers", "HoldAt::BodyPrefix(8)", "FileControlledSaveError::Cancelled", "FileDeliveryPhase::Complete", "verify_chunk", "ended(&mut progress)"]) assert.ok(cases.includes(phrase), phrase);
  assert.doesNotMatch(code(cases), /danger_accept|Reply::good|\.set_reply\(/);
});

test("Source HTTPS stays test-only with bilingual evidence distinct from release and deployment", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /SourceTlsServer|source_https|fileSourceHttps|large-file-source-https/, path);
  for (const lang of ["en", "zh-CN"]) {
    const s = await read(`docs/${lang}/large-file-source-https.md`);
    for (const phrase of ["0.5.8", "0.3.0", "2026-10-13", "OpenSSL", "BeforeDispatch", "Content-Length", "Bytes", "Unavailable", "TimedOut", "InvalidResponse", "Cancelled", "file_delivery_tdd", "test-file-delivery.sh", "fileSourceHttps", "106", "56"]) assert.ok(s.includes(phrase), `${lang}: ${phrase}`);
    for (const name of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-delivery-client", "large-file-delivery-https", "large-file-distribution-router"]) assert.ok((await read(`docs/${lang}/${name}.md`)).includes("large-file-source-https.md"), `${lang}: ${name}`);
  }
});
