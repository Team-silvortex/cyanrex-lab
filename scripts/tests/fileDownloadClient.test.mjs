import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_chunk_client/";

// Composition/inventory only; actual Rust HTTP/files/jobs/SQL establish behavior.
test("ordered download validates static input before a separate clone-shared flow gate", async () => {
  const s = code(await read(`${dir}mod.rs`));
  const method = s.slice(s.indexOf("pub async fn fetch_and_assemble"));
  for (const phrase of ["manifest.workspace() != self.inner.scope", "token.len() != 36", "id.to_string() == token", "FileDownloadError::InvalidInput", "download::run"]) assert.ok(method.includes(phrase), phrase);
  assert.ok(method.indexOf("FileDownloadError::InvalidInput") < method.indexOf("download::run"));
  for (const phrase of ["let original_staging = staging.clone()", "staging: original_staging", "downloads: Arc<Semaphore>", "downloads: Arc::new(Semaphore::new(config.capacity))", "FileDownloadError", "ChunkStagingError", "Duration::from_secs(60)"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(method, /sqlx|std::fs|reqwest|SessionBind|tokio::spawn/);
});

test("ordered download uses acknowledged sequential calls and then original staging with no continuation", async () => {
  const s = code(await read(`${dir}download.rs`));
  let previous = -1;
  for (const phrase of [".downloads", "try_acquire_owned", "for index in 0..manifest.chunk_sha256().len() as u32", "request::fetch_with_flow", "Some(flow.clone())", "FileDownloadError::Chunk { index, error }", ".staging", ".assemble_staged_file_leased(manifest, Some(flow))", "FileDownloadError::Assembly"]) {
    const current = s.indexOf(phrase);
    assert.ok(current > previous, `ordered ${phrase}`);
    previous = current;
  }
  assert.doesNotMatch(s, /tokio::(?:spawn|task)|join_all|FuturesUnordered|\.collect|Vec<|remove_|rename|sleep|retry|acquire_owned\(\)\.await/);
  assert.equal((s.match(/request::fetch_with_flow/g) || []).length, 1);
  const request = code(await read(`${dir}request.rs`));
  assert.match(request, /fetch_with_flow\(inner, token, grant_id, upload_id, manifest, index, None\)/);
  assert.ok(request.includes("Some(permit), flow"));
});

test("whole flow references enter actual save and assembly owners without changing legacy None paths", async () => {
  const receiver = code(await read("engine/src/services/file_chunk_receiver/mod.rs"));
  assert.match(receiver, /receive_and_stage_with_flow\(manifest, index, response, caller, None\)/);
  assert.ok(receiver.includes("reader.retain_flow(flow)"));
  const cursor = code(await read("engine/src/services/file_chunk_receiver/leased.rs"));
  let previous = -1;
  for (const phrase of ["cursor: Cursor<Vec<u8>>", "_permit: OwnedSemaphorePermit", "_caller: Option<OwnedSemaphorePermit>", "_flow: Option<Arc<OwnedSemaphorePermit>>"]) {
    const current = cursor.indexOf(phrase);
    assert.ok(current > previous, phrase);
    previous = current;
  }
  const staging = code(await read("engine/src/services/file_transfer/staging/mod.rs"));
  assert.match(staging, /assemble_staged_file_leased\(manifest, None\)/);
  assert.ok(staging.includes("work_assemble::run(&self.inner, manifest, flow)"));
  const worker = code(await read("engine/src/services/file_transfer/staging/work_assemble.rs")).split("#[cfg(test)]")[0];
  assert.match(worker, /spawn_blocking\(move \|\| \{\s*let _flow = flow;\s*let _permit = permit;\s*assemble::run/);
  assert.doesNotMatch(worker, /remove_|rename|timeout|retry/);
});

test("download regression target and opt-in Source runner have exact distinct inventories", async () => {
  const names = s => [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/file_download_client_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  const ignored = [];
  let defaults = 0;
  for (const file of await readdir(new URL("engine/tests/file_download_client/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const s = await read(`engine/tests/file_download_client/${file}`);
    const cases = names(s);
    ignored.push(...cases.map(name => `${module}::${name}`));
    defaults += (s.match(/#\[tokio::test\]/g) || []).length - cases.length;
    assert.match(main, new RegExp(`\\bmod ${module};`));
  }
  assert.equal(defaults, 11);
  assert.equal(ignored.length, 1);
  const runner = await read("scripts/test-file-download-client.sh");
  assert.deepEqual([...runner.matchAll(/\b\w+::postgres_\w+/g)].map(m => m[0]).sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test file_download_client_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-download-client.sh"));
});

test("ordered private downloading stays outside normal startup and has bilingual current boundaries", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /fetch_and_assemble|FileDownloadError|file_chunk_client/, path);
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-download-client.md`);
    for (const phrase of ["0.5.7", "0.3.0", "FileChunkClient", "fetch_and_assemble", "FileChunkClientConfig.capacity", "Arc<OwnedSemaphorePermit>", "total_timeout", "FileDownloadError", "AssembledFile", "test-file-download-client.sh", "fileDownloadClient", "106", "36"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-chunk-client", "large-file-chunk-receiver", "large-file-chunk-assembly", "capability-maturity"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-download-client.md`);
  }
});
