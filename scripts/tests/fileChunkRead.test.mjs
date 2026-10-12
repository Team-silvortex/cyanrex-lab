import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Source composition/inventory only. Runtime file and explicit SQL tests establish behavior.
test("byte read retains exact original authorization across two source transactions", async () => {
  const s = code(await read("engine/src/services/auth_service/durable_source/file_chunk_read.rs"));
  for (const phrase of ["SessionFileChunkReadWorkspace", "SessionFileChunkReadRequest", "SessionFileChunkReadError", "read_session_file_chunk", "try_acquire_owned", "FileAccountRef::new(context.owner, context.account_id())", "grant.state() != FileGrantState::Active", "get_exact_in_transaction", "source_root_binding", "bind_upload(source_key) != root_key", "finish_retained_in_window", "read_bound_chunk", "context.resume", "refence_pending_in_transaction", "finish_in_window", "READ COMMITTED"]) assert.ok(s.includes(phrase), phrase);
  assert.equal([...s.matchAll(/tx\.commit\(\)/g)].length, 2);
  assert.ok(s.indexOf("try_acquire_owned") < s.indexOf("self.transaction"));
  assert.ok(s.indexOf("tx.commit") < s.indexOf(".read_bound_chunk"));
  assert.ok(s.indexOf(".read_bound_chunk") < s.indexOf("context.resume"));
  const final = s.slice(s.indexOf("context.resume"));
  assert.ok(final.lastIndexOf("verify_pending_in_transaction") < final.indexOf("finish_in_window"));
  assert.doesNotMatch(final.slice(final.indexOf("finish_in_window"), final.indexOf("tx.commit")), /sqlx::query|select_target|verify_/);
  assert.doesNotMatch(final.slice(final.lastIndexOf("??;") + 3), /\.await|sqlx::query|\.staging\./);
  assert.doesNotMatch(s, /Serialize|Deserialize|into_vec|READ WRITE|file_publisher_grant_store|file_upload_store|file_upload_chunk_journal|role_refs|deployment_granted|install_empty|std::(?:fs|net|env)|tokio::fs|reqwest/);
});

test("byte snapshots do not expose constructors cloning or serialized payloads", async () => {
  const source = code(await read("engine/src/services/auth_service/durable_source/file_chunk_read.rs"));
  const dto = source.slice(source.indexOf("pub struct SessionFileChunkRead {"), source.indexOf("#[derive(Debug, Clone, Copy"));
  for (const phrase of ["proof: BufferedBoundChunkRead", "fn bytes(&self) -> &[u8]", "<redacted>"]) assert.ok(dto.includes(phrase), phrase);
  assert.doesNotMatch(dto, /pub (?:file|index|proof|bytes):|Serialize|Deserialize|derive\([^)]*Clone|into_vec|pub fn new/);
  const readLayer = code(await read("engine/src/services/file_transfer/staging/read.rs"));
  assert.ok(readLayer.includes("OwnedSemaphorePermit"));
  assert.doesNotMatch(readLayer, /Serialize|Deserialize|derive\([^)]*Clone|into_vec|pub fn/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /SessionFileChunkRead|read_session_file_chunk/, path);
});

test("original metadata refences neither acquire separate pools nor recapture observations", async () => {
  for (const path of ["file_recipient_grant_store", "file_upload_publication"]) {
    const s = code(await read(`engine/src/services/${path}/transaction.rs`));
    assert.ok(s.includes("refence_pending_in_transaction"));
    assert.ok(s.includes("pending.schema.verify"));
    assert.ok(s.includes("FOR SHARE"));
    assert.ok(s.includes("current != pending.expected"));
    assert.doesNotMatch(s, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|Serialize|Deserialize/);
  }
});

test("chunk read runner explicitly executes its complete wired ignored inventory", async () => {
  const names = s => [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/session_file_chunk_read_tdd.rs");
  const ignored = names(main);
  for (const file of await readdir(new URL("engine/tests/session_file_chunk_read/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/session_file_chunk_read/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  assert.ok(ignored.length >= 8);
  const runner = await read("scripts/test-session-file-chunk-read.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test session_file_chunk_read_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-session-file-chunk-read.sh"));
});

test("bilingual byte-read docs preserve snapshot and unmounted transport boundaries", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-chunk-read.md`);
    for (const phrase of ["SessionFileChunkReadWorkspace", "SessionFileChunkReadRequest", "SessionFileChunkReadError", "read_session_file_chunk", "READ COMMITTED", "READ ONLY", "4 MiB", "0.5.6", "0.3.0", "test-session-file-chunk-read.sh"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-download-manifest"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-chunk-read.md"), `${lang}: ${file}`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileChunkRead"));
  }
});
