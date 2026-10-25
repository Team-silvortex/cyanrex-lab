import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_chunk_client/";

// Static composition/inventory only. Native Rust frames/HTTP/files/SQL establish behavior.
test("manifest client reuses private transport and distinct gates without changing old error variants", async () => {
  const s = code(await read(`${dir}mod.rs`));
  for (const phrase of ["manifest_endpoint: reqwest::Url", 'manifest_endpoint.set_path("/platform/v1/file-manifests")', "manifests: Arc<Semaphore>", "manifests: Arc::new(Semaphore::new(config.capacity))", "FileManifestClientError", "FileGrantedDownloadError", "fetch_manifest", "fetch_granted_and_assemble", "manifest_inputs_valid"]) assert.ok(s.includes(phrase), phrase);
  const old = s.slice(s.indexOf("pub enum FileDownloadError"), s.indexOf("pub enum FileManifestClientError"));
  assert.doesNotMatch(old, /Manifest|InvalidResponse|TimedOut|Unavailable|Granted/);
  const combined = s.slice(s.indexOf("pub async fn fetch_granted_and_assemble"), s.indexOf("fn manifest_inputs_valid"));
  let previous = -1;
  for (const phrase of ["manifest_inputs_valid", "FileGrantedDownloadError::InvalidInput", ".downloads", "try_acquire_owned", "manifest::fetch", "run_with_flow"]) {
    const position = combined.indexOf(phrase);
    assert.ok(position > previous, `ordered ${phrase}`);
    previous = position;
  }
  assert.doesNotMatch(s, /Serialize|Deserialize|pub fn (?:path|bytes|origin|client|token)|sqlx|SessionBind|std::(?:fs|env)|tokio::spawn/);
});

test("manifest GET carries only six typed selectors and sensitive native credentials", async () => {
  const s = code(await read(`${dir}manifest.rs`));
  let previous = -1;
  for (const phrase of ["manifest_inputs_valid", ".manifests", "try_acquire_owned", "endpoint.set_query", "cookie.set_sensitive(true)", "request.send()", "response.into()", "manifest_contract::validate", "manifest_receive::collect"]) {
    const position = s.indexOf(phrase);
    assert.ok(position > previous, `ordered ${phrase}`);
    previous = position;
  }
  for (const phrase of ["grant_id={grant_id}", "upload_id={upload_id}", "file_id={}", "version_id={}", "byte_length={}", "sha256={}", "cyanrex_platform_session={token}", "x-cyanrex-platform-request", "application/json", "identity", "body_started >= deadline", "Response<reqwest::Body>"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /chunk_index|\.header\((?:ORIGIN|AUTHORIZATION)|\.body\(|\.json\(|\.bytes\(|\.bytes_stream\(|\.chunk\(|\.timeout\(|tokio::spawn|tracing::|println!|error\.to_string\(/);
});

test("manifest contract and collector preserve bounded headers real frames raw strict parsing and exact version", async () => {
  const contract = code(await read(`${dir}manifest_contract.rs`)).split("#[cfg(test)]")[0];
  for (const phrase of ["MAX_HEADERS: usize = 64", "MAX_HEADER_BYTES: usize = 8192", "StatusCode::OK", "MAX_FILE_TRANSFER_MANIFEST_BYTES", "access-control-", "get_all", "values.next().is_some()", "is_ascii_digit", "application/json", "no-store", "no-cache", "nosniff", "content-encoding", "transfer-encoding", "trailer", "set-cookie", "content-disposition", "x-cyanrex-file-transfer-version", "x-cyanrex-file-sha256"]) assert.ok(contract.includes(phrase), phrase);
  const receive = code(await read(`${dir}manifest_receive.rs`)).split("#[cfg(test)]")[0];
  for (const phrase of ["try_reserve_exact", "poll_frame", "timeout_at", "check_deadline", "into_data", "data.is_empty()", "yield_now", "FILE_TRANSFER_BUFFER_BYTES", "FileTransferManifest::parse_json(&bytes)", "FileVersionRef::from_manifest", "bytes.len() != length"]) assert.ok(receive.includes(phrase), phrase);
  assert.doesNotMatch(receive, /serde_json::(?:Value|from_value)|size_hint\(|is_end_stream\(|\.collect\(|std::fs|tokio::spawn|verify_file|stage_chunk/);
  const core = code(await read(`${dir}download.rs`));
  const reused = core.slice(core.indexOf("pub(super) async fn run_with_flow"));
  assert.doesNotMatch(reused, /try_acquire_owned|manifest::fetch|file-manifests|tokio::spawn|remove_|rename/);
  assert.ok(reused.includes("Some(flow.clone())") && reused.includes("assemble_staged_file_leased(manifest, Some(flow))"));
});

test("manifest client has wired default cases and exact separate Source runner inventory", async () => {
  const names = s => [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/file_manifest_client_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  const ignored = [];
  let defaults = 0;
  for (const file of await readdir(new URL("engine/tests/file_manifest_client/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_manifest_client/${file}`);
    const module = file.replace(/\.rs$/, "");
    const cases = names(s);
    const count = (s.match(/#\[tokio::test\]/g) || []).length;
    defaults += count - cases.length;
    if (count) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  assert.equal(defaults, 16, "exact actual default matrix, not fixture-only inventory");
  assert.equal(ignored.length, 2);
  const runner = await read("scripts/test-file-manifest-client.sh");
  const inventory = [...runner.matchAll(/\b\w+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test file_manifest_client_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-manifest-client.sh"));
});

test("manifest client stays unmounted with current bilingual authorization and compatibility notes", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /fetch_manifest|fetch_granted_and_assemble|FileManifestClientError|FileGrantedDownloadError|file-manifests/, path);
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-manifest-client.md`);
    for (const phrase of ["0.5.7", "0.3.0", "FileChunkClient", "fetch_manifest", "fetch_granted_and_assemble", "fetch_and_assemble", "FileManifestClientError", "FileGrantedDownloadError", "FileVersionRef::from_manifest", "/platform/v1/file-manifests", "Content-Length", "1152 KiB", "16,384", "64 GiB", "test-file-manifest-client.sh", "fileManifestClient", "106", "40"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-download-manifest", "large-file-download-client", "large-file-manifest-http"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-manifest-client.md`);
  }
});
