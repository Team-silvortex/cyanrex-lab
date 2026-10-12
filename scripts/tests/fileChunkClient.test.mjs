import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_chunk_client/";

// Composition/inventory only; Rust socket/file/job/TLS and explicit SQL tests prove behavior.
test("chunk client freezes canonical policy and owns a controlled transport without dangerous options", async () => {
  const s = code(await read(`${dir}policy.rs`));
  for (const phrase of ["ascii_serialization() != origin", "endpoint.username()", "endpoint.password()", "endpoint.path()", "endpoint.query()", "endpoint.fragment()", "parse::<IpAddr>()", "ip.is_loopback()", "origin.len() > 2048", "1..=65536", "Certificate::from_pem_bundle", "certificates.is_empty()", "add_root_certificate", "use_rustls_tls", "https_only", "redirect(reqwest::redirect::Policy::none())", "retry(reqwest::retry::never())", "no_proxy()", "no_gzip()", "no_brotli()", "no_deflate()", "no_zstd()", "referer(false)", "http1_only()", "pool_max_idle_per_host(0)", "connect_timeout"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /danger_accept|cookie_store|cookie_provider|\.proxy\(|\.timeout\(|std::(?:fs|env)|tokio::fs|\.send\(/);
  assert.ok(s.includes('endpoint.set_path("/platform/v1/file-chunks")'));
});

test("chunk client validates before admission and sends only typed selectors and sensitive native credentials", async () => {
  const s = code(await read(`${dir}request.rs`));
  let previous = -1;
  for (const phrase of ["manifest.workspace()", "manifest.chunk(index)", "token.len() != 36", "try_acquire_owned", "endpoint.set_query", "cookie.set_sensitive(true)", ".http", "timeout_at(deadline", "response.into()", ".receive_and_stage_leased"]) {
    const current = s.indexOf(phrase);
    assert.ok(current > previous, `ordered ${phrase}`);
    previous = current;
  }
  for (const phrase of ["grant_id={grant_id}", "upload_id={upload_id}", "file_id={}", "version_id={}", "byte_length={}", "sha256={}", "chunk_index={index}", "id.to_string() == token", "cyanrex_platform_session={token}", "x-cyanrex-platform-request", "application/octet-stream", "identity", "Some(permit)"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /\.header\((?:ORIGIN|AUTHORIZATION)|\.body\(|\.json\(|\.basic_auth\(|\.bearer_auth\(|tracing::|println!|log::|error\.to_string\(/);
});

test("chunk client separates header waiting from framed receiver collection and actual job ownership", async () => {
  const request = code(await read(`${dir}request.rs`));
  assert.match(request, /timeout_at\(deadline, request\.send\(\)\)/);
  assert.ok(request.includes("Instant::now() >= deadline"));
  assert.match(request, /Response<reqwest::Body>\s*=\s*response\.into\(\)/);
  assert.doesNotMatch(request, /\.chunk\(\)|\.bytes\(\)|\.bytes_stream\(\)|to_bytes\(|body\.collect\(|tokio::spawn|\.timeout\(/);
  const receiver = code(await read("engine/src/services/file_chunk_receiver/mod.rs"));
  for (const phrase of ["response, None", "pub(crate) async fn receive_and_stage_leased", "reader.retain_caller(caller)", "self.stage_checked"]) assert.ok(receiver.includes(phrase), phrase);
  const leased = code(await read("engine/src/services/file_chunk_receiver/leased.rs"));
  assert.ok(leased.indexOf("cursor: Cursor<Vec<u8>>") < leased.indexOf("_permit: OwnedSemaphorePermit"));
  assert.ok(leased.indexOf("_permit: OwnedSemaphorePermit") < leased.indexOf("_caller: Option<OwnedSemaphorePermit>"));
});

test("chunk client remains explicitly Unix and outside normal startup API SDK and UI composition", async () => {
  assert.match(await read("engine/src/services/mod.rs"), /#\[cfg\(unix\)\]\s*pub mod file_chunk_client;/);
  const s = code(await read(`${dir}mod.rs`));
  for (const phrase of ["Duration::from_secs(10)", "Duration::from_secs(5)", "Duration::from_secs(60)", "FileChunkReceiver::with_limits", "staging.scope()", "Arc<Semaphore>", "Semaphore::new(config.capacity)"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /pub fn (?:bytes|token|url|origin|client|path)|Serialize|Deserialize|sqlx|SessionBind|AppState|std::(?:fs|env)/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /FileChunkClient|file_chunk_client|fetch_and_stage/, path);
});

test("chunk client runner exactly wires SQL and TLS inventory with bilingual current boundaries", async () => {
  const names = s => [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/file_chunk_client_tdd.rs");
  const ignored = names(main);
  for (const file of await readdir(new URL("engine/tests/file_chunk_client/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/file_chunk_client/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  assert.equal(ignored.length, 2);
  const runner = await read("scripts/test-file-chunk-client.sh");
  const inventory = [...runner.matchAll(/\b\w+::(?:postgres_|openssl_)\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /command -v openssl/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test file_chunk_client_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-chunk-client.sh"));
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-chunk-client.md`);
    for (const phrase of ["0.5.6", "0.3.0", "FileChunkClient", "FileChunkClientConfig", "FileChunkTransport", "new_https", "new_loopback", "fetch_and_stage", "ReceivedFileChunk", "retry(reqwest::retry::never())", "Content-Length", "OpenSSL", "test-file-chunk-client.sh"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-chunk-http", "large-file-chunk-receiver", "capability-maturity"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-chunk-client.md"), `${lang}: ${file}`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileChunkClient"));
  }
});
