import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/platform_http/file_download_manifest/";

// Composition/inventory only; compiled Rust bodies and explicit SQL establish actual behavior.
test("manifest HTTP freezes an independent bounded host state and cached original scope", async () => {
  const s = code(await read(`${dir}mod.rs`));
  for (const phrase of ["FileDownloadManifestHttpState", "SessionFileDownloadWorkspace", "security::validate_origin", "origin, 4", "1..=16", "Arc<Semaphore>", "Semaphore::new(capacity)", "security::response_headers", "/platform/v1/file-manifests", ".fallback"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /Serialize|Deserialize|install_empty|\.begin\(|\.acquire\(|SessionBind|std::(?:fs|net|env)|tokio::spawn|reqwest/);
  const source = code(await read("engine/src/services/auth_service/durable_source/file_download.rs"));
  assert.match(source, /pub\(crate\) fn file_scope\(&self\)[^{]*\{\s*self\.publication\.scope\(\)\s*\}/);
  assert.match(await read("engine/src/platform_http/mod.rs"), /#\[cfg\(unix\)\]\s*mod file_download_manifest;/);
});

test("manifest HTTP accepts only six bounded raw exact-version selectors without request attribution", async () => {
  const s = code(await read(`${dir}contract.rs`)).split("#[cfg(test)]")[0];
  for (const phrase of ["MAX_QUERY_BYTES: usize = 512", "[None; 6]", "query.contains(['%', '+'])", "value.contains('=')", "fields[index].replace(value)", "MAX_FILE_TRANSFER_BYTES", "FileVersionRef", '"workspace": scope', "SessionFileDownloadRequest"]) assert.ok(s.includes(phrase), phrase);
  const keys = [...s.matchAll(/"(\w+)" => \d/g)].map(m => m[1]);
  assert.deepEqual(keys, ["grant_id", "upload_id", "file_id", "version_id", "byte_length", "sha256"]);
  assert.ok(s.includes("length.starts_with('0')") && s.includes("byte.is_ascii_digit()"));
  assert.doesNotMatch(s, /chunk_index|percent_decode|serde_urlencoded|owner_id|recipient_id|namespace|std::fs|sqlx|tokio::spawn/);
});

test("manifest HTTP composes cheap gates one current Source call and ordinary JSON admission", async () => {
  const s = code(await read(`${dir}handlers.rs`));
  let previous = -1;
  for (const phrase of ["endpoint(&request", "security::credentials", "admission::acquire", "handle(state", "admission::response", "security::request_bytes", ".get_session_file_download_manifest", "response::manifest"]) {
    const current = s.indexOf(phrase);
    assert.ok(current > previous, `ordered ${phrase}`);
    previous = current;
  }
  assert.equal((s.match(/\.get_session_file_download_manifest\(/g) || []).length, 1);
  for (const phrase of ["Method::GET", 'method_not_allowed("GET")', '"range"', '"if-"', "file_scope()", "request, true"]) assert.ok(s.includes(phrase), phrase);
  const projection = code(await read(`${dir}response.rs`)).split("#[cfg(test)]")[0];
  for (const phrase of ["serde_json::to_vec(manifest)", "MAX_FILE_TRANSFER_MANIFEST_BYTES", "encoded.len()", "Body::from(encoded)", "CONTENT_TYPE", "CONTENT_LENGTH", "application/json"]) assert.ok(projection.includes(phrase), phrase);
  for (const text of [s, projection]) assert.doesNotMatch(text, /sqlx|\.begin\(|\.commit\(|std::(?:fs|net|env)|tokio::(?:fs|spawn)|reqwest|Bytes::from_owner|first_frame_deadline|read_session_file_chunk|set-cookie|content-disposition/);
});

test("manifest HTTP exact opt-in runner matches every wired SQL case and private contract inventory", async () => {
  const names = s => [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/file_download_manifest_http_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  const ignored = [];
  let defaults = 0;
  for (const file of await readdir(new URL("engine/tests/file_download_manifest_http/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_download_manifest_http/${file}`);
    const module = file.replace(/\.rs$/, "");
    const cases = names(s);
    defaults += (s.match(/#\[tokio::test\]/g) || []).length - cases.length;
    if (cases.length || s.includes("#[tokio::test]")) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  assert.equal(defaults, 12);
  assert.equal(ignored.length, 8);
  const runner = await read("scripts/test-file-download-manifest-http.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test file_download_manifest_http_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-download-manifest-http.sh"));
  const units = (await Promise.all(["contract_tests", "error_tests", "response_tests"].map(name => read(`${dir}${name}.rs`)))).join("\n");
  assert.equal((units.match(/#\[(?:tokio::)?test\]/g) || []).length, 10);
});

test("manifest HTTP stays unmounted and current bilingual notes preserve authorization boundaries", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /file-manifests|FileDownloadManifestHttpState|build_file_download_manifest_router/, path);
  const client = code(await read("engine/src/services/file_chunk_client/download.rs"));
  assert.doesNotMatch(client, /file-manifests|get_session_file_download_manifest/);
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-manifest-http.md`);
    for (const phrase of ["0.5.7", "0.3.0", "FileDownloadManifestHttpState", "SessionFileDownloadWorkspace", "build_file_download_manifest_router", "/platform/v1/file-manifests", "get_session_file_download_manifest", "FileTransferManifest", "Content-Length", "READ ONLY", "1152 KiB", "not_attempted", "unconfirmed", "test-file-download-manifest-http.sh", "fileManifestHttp", "106", "38"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-download-manifest", "large-file-download-client"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-manifest-http.md"), `${lang}: ${file}`);
  }
});
