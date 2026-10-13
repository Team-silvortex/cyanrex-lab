import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const client = "engine/src/services/file_chunk_client/";
const staging = "engine/src/services/file_transfer/staging/";
const ordered = (s, words) => {
  let last = -1;
  for (const word of words) {
    const current = s.indexOf(word);
    assert.ok(current >= 0 && current > last, `present and ordered ${word}`);
    last = current;
  }
};

// Static composition/inventory only. Native real HTTP/files/jobs/SQL establish behavior.
test("delivery adds an explicit borrowed target and independent errors without public target or transport authority", async () => {
  const s = code(await read(`${client}mod.rs`));
  assert.match(s, /pub async fn fetch_granted_and_save\(\s*&self,\s*token: &str,\s*grant_id: FileGrantId,\s*upload_id: FileUploadId,\s*expected_file: &FileVersionRef,\s*target: &FileSaveTarget,?\s*\)/);
  for (const phrase of ["FileGrantedSaveError", "Download(FileGrantedDownloadError)", "Save(FileSaveError)", "delivery::run"]) assert.ok(s.includes(phrase), phrase);
  const oldChunk = s.slice(s.indexOf("pub enum FileChunkClientError"), s.indexOf("pub enum FileDownloadError"));
  const oldGranted = s.slice(s.indexOf("pub enum FileGrantedDownloadError"), s.indexOf("pub enum FileGrantedSaveError"));
  assert.doesNotMatch(oldChunk + oldGranted, /FileSaveError|FileGrantedSaveError|SavedFile|Target/);
  assert.doesNotMatch(s, /Serialize|Deserialize|pub fn (?:path|bytes|token|origin|client)|sqlx|SessionBind|std::(?:env|fs)|tokio::spawn/);
});

test("delivery admits exactly once before metadata and transfers the surviving parent flow into real save work", async () => {
  const full = code(await read(`${client}delivery.rs`)).split("\n#[cfg(test)]")[0];
  const preflight = full.slice(full.indexOf("pub(super) fn validate_inputs("), full.indexOf("pub(super) async fn run("));
  ordered(preflight, ["manifest_inputs_valid(inner, token, expected_file)", "validate_save_target(target)", "FileGrantedSaveError::InvalidInput", "Ok(())"]);
  assert.doesNotMatch(preflight, /try_acquire|manifest::fetch|std::fs|tokio::fs|spawn/);
  const start = full.indexOf("async fn run_inner(");
  assert.ok(start >= 0, "actual shared delivery core exists");
  const s = full.slice(start);
  ordered(s, ["validate_inputs(inner, token, expected_file, target)?", ".downloads", "try_acquire_owned", "manifest::fetch", "download::run_with_flow", "Arc::clone(&flow)", "save_assembled_file_leased", "Some(flow)"]);
  assert.equal((s.match(/try_acquire_owned/g) || []).length, 1);
  assert.doesNotMatch(s, /fetch_granted_and_assemble\(|\.save_assembled_file\(|Semaphore::new|spawn_blocking|tokio::spawn|std::fs|remove_|rename\(|retry|\.display_name/);
  const mod = code(await read(`${staging}mod.rs`));
  assert.match(mod, /save_assembled_file_leased\([\s\S]*?flow: Option<Arc<OwnedSemaphorePermit>>/);
  assert.ok(mod.includes("target, None") || /target,\s*None/.test(mod));
  const work = code(await read(`${staging}work_save.rs`));
  const production = work.slice(work.indexOf("pub(super) async fn run("), work.indexOf("\n#[cfg(test)]\npub(super) async fn run_with"));
  ordered(production, ["work::admit(inner)", "manifest.clone()", "Arc::clone(proof)", "Arc::clone(target)", "spawn_blocking(move ||", "let _flow = flow", "let _permit = permit"]);
  assert.ok(production.includes("save_io::run("), "real production save, not a test run_with prefix");
});

test("delivery wires actual defaults and composed Source cases with exact native runner inventory", async () => {
  const main = await read("engine/tests/file_delivery_tdd.rs");
  const source = await read("engine/tests/file_delivery/source.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  let defaults = 0;
  const ignored = [];
  for (const file of await readdir(new URL("engine/tests/file_delivery/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_delivery/${file}`), module = file.replace(/\.rs$/, "");
    const names = [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
    const count = (s.match(/#\[(?:tokio::)?test\]/g) || []).length;
    defaults += count - names.length;
    const nested = module === "distribution_lifetime";
    if (count) assert.match(nested ? source : main, new RegExp(`\\bmod ${module};`));
    if (nested) assert.match(source, /#\[path = "distribution_lifetime\.rs"\]/);
    ignored.push(...names.map(name => `${nested ? "source::" : ""}${module}::${name}`));
  }
  const https = await read("engine/tests/file_delivery/source_https/mod.rs");
  assert.match(source, /#\[path = "source_https\/mod\.rs"\]\s*mod source_https;/);
  let combined = 0;
  for (const file of await readdir(new URL("engine/tests/file_delivery/source_https/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_delivery/source_https/${file}`);
    const module = file.replace(/\.rs$/, "");
    const names = [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_openssl_\w+)\(/g)].map(m => m[1]);
    const count = (s.match(/#\[(?:tokio::)?test(?:\([^\]]*\))?\]/g) || []).length;
    assert.equal(count, names.length, "combined Source TLS cases require explicit external execution");
    if (count) assert.match(https, new RegExp(`\\bmod ${module};`));
    combined += count;
    ignored.push(...names.map(name => `source::source_https::${module}::${name}`));
  }
  assert.equal(combined, 3);
  assert.equal(defaults, 10, "exact real cases, not helper-only files");
  assert.equal(ignored.length, 6);
  const runner = await read("scripts/test-file-delivery.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test file_delivery_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-delivery.sh"));
  const baseline = await read("engine/tests/file_delivery/baseline.rs");
  ordered(baseline, ["checked(", "result.expect("]);
  for (const phrase of ["fetch_granted_and_save", "observe_output", "FILE_TRANSFER_CHUNK_BYTES + 17", "source_entries", "selected_entries", "assert_request"]) assert.ok(baseline.includes(phrase), phrase);
});

test("delivery keeps Source authorization and original output observation in real network fixtures", async () => {
  const s = await read("engine/tests/file_delivery/source.rs");
  for (const phrase of ["fetch_granted_and_save", "publish(f", "assert_private_whole", "assert_saved_copy", "f.sharing.snapshot()", "recipient_delta", "take_statuses", "pause.add_permits", "FileGrantedSaveError::Download"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(code(s), /ignore = "[^"]*(?:placeholder|todo)/);
});

test("delivery remains unmounted and current bilingual notes retain private target and cancellation boundaries", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /fetch_granted_and_save|FileGrantedSaveError/, path);
  for (const lang of ["en", "zh-CN"]) {
    const s = await read(`docs/${lang}/large-file-delivery-client.md`);
    for (const word of ["0.5.7", "0.3.0", "2026-10-13", "fetch_granted_and_save", "FileSaveTarget", "SavedFile", "FileVersionRef", "FileGrantedSaveError", "FileGrantedDownloadError", "FileSaveError", "0700", "file_delivery_tdd", "test-file-delivery.sh", "fileDelivery", "106", "44"]) assert.ok(s.includes(word), `${lang}: ${word}`);
    for (const doc of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-download-client", "large-file-manifest-client", "large-file-save"]) assert.ok((await read(`docs/${lang}/${doc}.md`)).includes("large-file-delivery-client.md"), `${lang}: ${doc}`);
  }
});
