import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_chunk_receiver/";

// Composition and inventory only; actual Rust body/file/job and SQL tests establish behavior.
test("supplied-response receiver keeps checked receive before the existing exclusive staging job", async () => {
  const s = code(await read(`${dir}mod.rs`));
  const checked = s.slice(s.indexOf("async fn receive_checked"));
  let previous = -1;
  for (const phrase of ["manifest.workspace()", ".chunk(index)", "contract::validate", "try_acquire_owned", "receive::collect", "LeasedCursor::new", ".stage_chunk"]) {
    const current = checked.indexOf(phrase);
    assert.ok(current > previous, `ordered ${phrase}`);
    previous = current;
  }
  for (const phrase of ["Duration::from_secs(5)", "Duration::from_secs(60)", "Duration::from_secs(300)", "1..=16", "Arc<Semaphore>", "staging.scope()", "Body<Data = Bytes> + Send + 'static"]) assert.ok(s.includes(phrase), phrase);
  const metadata = s.slice(s.indexOf("pub struct ReceivedFileChunk"), s.indexOf("impl FileChunkReceiver"));
  assert.doesNotMatch(metadata, /pub fn (?:new|bytes|path|binding_key)|pub\s+\w+\s*:|Serialize|Deserialize|impl Clone/);
  for (const phrase of ["file(", "chunk_index(", "byte_length(", "chunk_sha256("]) assert.ok(metadata.includes(phrase), phrase);
});

test("receiver head is bounded with twelve exact singleton fields and forbidden transformations", async () => {
  const s = code(await read(`${dir}contract.rs`));
  for (const phrase of ["MAX_HEADERS: usize = 64", "MAX_HEADER_BYTES: usize = 8192", "StatusCode::OK", "headers.get_all", "values.next().is_some()", "spec.byte_length.to_string()", "spec.index.to_string()", "manifest.sha256()", "application/octet-stream", "chunk.bin"]) assert.ok(s.includes(phrase), phrase);
  for (const name of ["content-length", "content-type", "content-disposition", "x-cyanrex-file-transfer-version", "x-cyanrex-file-id", "x-cyanrex-file-version-id", "x-cyanrex-chunk-index", "x-cyanrex-chunk-sha256", "x-cyanrex-file-sha256", "cache-control", "pragma", "x-content-type-options", "content-encoding", "content-range", "transfer-encoding", "trailer", "set-cookie", "location", "etag", "last-modified", "retry-after"]) assert.ok(s.includes(`"${name}"`), name);
  assert.doesNotMatch(s, /to_lowercase|trim\(|parse::<|\.poll_frame|std::fs|tokio::fs|reqwest/);
});

test("receiver collects only bounded private bytes through real EOF and non-refreshing total time", async () => {
  const s = code(await read(`${dir}receive.rs`));
  for (const phrase of ["try_reserve_exact(length)", "FILE_TRANSFER_CHUNK_BYTES", "FILE_TRANSFER_BUFFER_BYTES", "Box::pin(body)", "poll_frame(cx)", "timeout_at", "total_deadline.min(idle_deadline)", "check_deadline", "hash.finalize()", "yield_now()", "data.is_empty()", "into_data()", "let Some(frame) = frame else"]) assert.ok(s.includes(phrase), phrase);
  assert.ok(s.indexOf("data.len() > length - bytes.len()") < s.indexOf("bytes.extend_from_slice(part)"));
  assert.equal([...s.matchAll(/total_deadline\s*=/g)].length, 1);
  assert.doesNotMatch(s, /\.size_hint\(|\.is_end_stream\(|http_body_util|to_bytes\(|tokio::spawn|std::fs|tokio::fs|reqwest/);
});

test("receiver buffer and control stay private and no active or runtime client is mounted", async () => {
  const lease = code(await read(`${dir}leased.rs`));
  for (const phrase of ["Cursor<Vec<u8>>", "OwnedSemaphorePermit", "impl Read for LeasedCursor", "self.cursor.read(buffer)"]) assert.ok(lease.includes(phrase), phrase);
  assert.ok(lease.indexOf("cursor: Cursor<Vec<u8>>") < lease.indexOf("_permit: OwnedSemaphorePermit"));
  for (const file of await readdir(new URL(dir, root))) {
    if (!file.endsWith(".rs") || file.includes("test")) continue;
    assert.doesNotMatch(code(await read(`${dir}${file}`)), /reqwest|sqlx|\.begin\(|\.commit\(|std::(?:fs|net|env)|tokio::fs|\.write_all\(|SessionBind|AppState/, file);
  }
  assert.match(await read("engine/src/services/mod.rs"), /#\[cfg\(unix\)\]\s*pub mod file_chunk_receiver;/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /FileChunkReceiver|receive_and_stage|file_chunk_receiver/, path);
});

test("receiver runner covers the complete ignored inventory and bilingual docs delimit acceptance", async () => {
  const names = s => [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/file_chunk_receiver_tdd.rs");
  const ignored = names(main);
  for (const file of await readdir(new URL("engine/tests/file_chunk_receiver/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/file_chunk_receiver/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  assert.equal(ignored.length, 3);
  const runner = await read("scripts/test-file-chunk-receiver.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test file_chunk_receiver_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-chunk-receiver.sh"));
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-chunk-receiver.md`);
    for (const phrase of ["0.5.6", "0.3.0", "FileChunkReceiver", "receive_and_stage", "ReceivedFileChunk", "stage_chunk", "EOF", "SHA-256", "64 KiB", "4 MiB", "test-file-chunk-receiver.sh"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-chunk-http", "large-file-staging", "capability-maturity"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-chunk-receiver.md"), `${lang}: ${file}`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileChunkReceiver"));
  }
});
