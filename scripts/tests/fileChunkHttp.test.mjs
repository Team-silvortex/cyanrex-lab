import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/platform_http/file_chunk_read/";

// Composition and inventory only. Rust frame tests and explicit real SQL establish behavior.
test("binary chunk GET retains cheap gates and exactly one existing source command", async () => {
  const s = code(await read(`${dir}handlers.rs`));
  const order = ["endpoint(&request", "security::credentials", "admission::acquire", "security::request_bytes", ".read_session_file_chunk", "body::first_frame_deadline", "contract::response_headers", "body::response"];
  let previous = -1;
  for (const phrase of order) {
    const current = s.indexOf(phrase);
    assert.ok(current > previous, `ordered ${phrase}`);
    previous = current;
  }
  assert.equal([...s.matchAll(/\.read_session_file_chunk\(/g)].length, 1);
  for (const phrase of ["/platform/v1/file-chunks", "Method::GET", "method_not_allowed(\"GET\")", '"range"', '"if-"', "file_scope()", "Response::new"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /sqlx|\.begin\(|\.commit\(|\.staging|std::(?:fs|net|env)|tokio::fs|reqwest|Set-Cookie/);
});

test("binary query and headers keep bounded trusted version projection without payload copies", async () => {
  const s = code(await read(`${dir}contract.rs`));
  const helper = code(await read("engine/src/platform_http/file_selectors.rs")).split("#[cfg(test)]")[0];
  for (const phrase of ["MAX_QUERY_BYTES: usize = 512", '[""; N]', "query.contains(['%', '+'])", "value.contains('=')", "!fields[index].is_empty()", "fields[index] = value"]) assert.ok(helper.includes(phrase), phrase);
  for (const phrase of ["file_selectors::parse(", "file_selectors::decimal(", "MAX_FILE_TRANSFER_BYTES", "MAX_FILE_TRANSFER_CHUNKS", "FileVersionRef", "workspace\": scope", "chunk.chunk_sha256()", "application/octet-stream", "x-cyanrex-file-transfer-version", "x-cyanrex-file-id", "x-cyanrex-file-version-id", "x-cyanrex-chunk-index", "x-cyanrex-chunk-sha256", "x-cyanrex-file-sha256", "chunk.bin"]) assert.ok(s.includes(phrase), phrase);
  const call = s.match(/file_selectors::parse\(\s*query,\s*\[([\s\S]*?)\]\s*,?\s*\)\?/);
  assert.ok(call);
  assert.deepEqual([...call[1].matchAll(/"(\w+)"/g)].map(m => m[1]), ["grant_id", "upload_id", "file_id", "version_id", "byte_length", "sha256", "chunk_index"]);
  assert.match(s, /byte_length > MAX_FILE_TRANSFER_BYTES \|\| chunk_index >= MAX_FILE_TRANSFER_CHUNKS as u64/);
  assert.doesNotMatch(s, /query\.(?:split|contains)|split_once|const MAX_QUERY_BYTES|fn decimal\(|chunk\.bytes\(\)\.(?:to_vec|to_owned)|Bytes::copy_from_slice|base64|\bsha2\b|std::fs|tokio::fs/);
});

test("binary data owns both leases beyond EOF and first-frame expiry has no detached timer", async () => {
  const s = code(await read(`${dir}body.rs`));
  for (const phrase of ["FIRST_FRAME_TIMEOUT", "Duration::from_secs(10)", "chunk: SessionFileChunkRead", "OwnedSemaphorePermit", "self.chunk.bytes()", "Bytes::from_owner", "data: Option<Bytes>", "self.data.take()", "Instant::now() >= self.deadline", "drop(data)", "Frame::data(data)"]) assert.ok(s.includes(phrase), phrase);
  assert.ok(s.indexOf("chunk: SessionFileChunkRead") < s.indexOf("_http: OwnedSemaphorePermit"));
  assert.doesNotMatch(s, /\.to_vec\(|\.to_owned\(|copy_from_slice|Serialize|Deserialize|tokio::spawn|sleep\(|\.await|sqlx|std::fs|tokio::fs/);
});

test("binary adapter stays explicitly unmounted and does not widen older request or API contracts", async () => {
  const exports = await read("engine/src/platform_http/mod.rs");
  for (const phrase of ["mod file_chunk_read", "build_file_chunk_read_router", "FileChunkReadHttpState"]) assert.ok(exports.includes(phrase), phrase);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /file-chunks|FileChunkReadHttpState|build_file_chunk_read_router/, path);
  const state = code(await read(`${dir}mod.rs`));
  for (const phrase of ["config::validate(origin, &[capacity])?", "origin, 4", "Arc<Semaphore>", "security::response_headers"]) assert.ok(state.includes(phrase), phrase);
  const configuration = code(await read("engine/src/platform_http/config.rs"));
  assert.match(configuration, /security::validate_origin\(origin\)\?;[\s\S]*capacities\s*\.iter\(\)\s*\.any\([\s\S]*!\(1\.\.=16\)\.contains\(/);
  assert.match(state, /admission: Arc::new\(Semaphore::new\(capacity\)\)/);
  assert.ok(state.indexOf("config::validate(origin, &[capacity])?") < state.indexOf("Semaphore::new(capacity)"));
  assert.doesNotMatch(state, /install_empty|\.begin\(|\.acquire\(|SessionBind|TaskDefinition|Runner|reqwest|std::fs/);
});

test("binary HTTP runner executes the complete wired ignored inventory and docs delimit acceptance", async () => {
  const names = s => [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/file_chunk_read_http_tdd.rs");
  const ignored = names(main);
  for (const file of await readdir(new URL("engine/tests/file_chunk_read_http/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/file_chunk_read_http/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  assert.equal(ignored.length, 6);
  const runner = await read("scripts/test-file-chunk-http.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test file_chunk_read_http_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-chunk-http.sh"));
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-chunk-http.md`);
    for (const phrase of ["0.5.6", "0.3.0", "FileChunkReadHttpState", "build_file_chunk_read_router", "/platform/v1/file-chunks", "Bytes::from_owner", "read_session_file_chunk", "test-file-chunk-http.sh"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-chunk-read", "capability-maturity"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-chunk-http.md`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileChunkHttp"));
  }
});
