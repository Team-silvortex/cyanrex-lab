import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_chunk_client/";
const ordered = (s, names) => {
  let last = -1;
  for (const name of names) {
    const at = s.indexOf(name);
    assert.ok(at >= 0 && at > last, `present and ordered ${name}`);
    last = at;
  }
};

// Composition/inventory only. Actual native HTTP/files/jobs and separate SQL prove behavior.
test("delivery progress adds a consumed opaque observer without changing original entry or errors", async () => {
  const s = code(await read(`${dir}mod.rs`));
  assert.match(s, /pub async fn fetch_granted_and_save_observed\([\s\S]*?target: &FileSaveTarget,\s*observer: FileDeliveryObserver,?\s*\) -> Result<SavedFile, FileGrantedSaveError>/);
  assert.match(s, /pub async fn fetch_granted_and_save\([\s\S]*?target: &FileSaveTarget,?\s*\) -> Result<SavedFile, FileGrantedSaveError>/);
  assert.ok(s.includes("delivery::run_observed("));
  const types = code(await read(`${dir}progress.rs`)).split("\n#[cfg(test)]")[0];
  for (const phrase of ["pub struct FileDeliverySnapshot", "phase: FileDeliveryPhase", "staged_chunks: u32", "staged_bytes: u64", "total_bytes: u64", "total_chunks: Option<u32>", "pub struct FileDeliveryObserver", "pub struct FileDeliveryProgress"]) assert.ok(types.includes(phrase), phrase);
  const opaque = types.slice(types.indexOf("pub struct FileDeliveryObserver"));
  assert.doesNotMatch(opaque, /derive\([^)]*(?:Clone|Debug|Serialize|Deserialize)|pub\s+(?:sender|receiver):|pub fn (?:report|send|subscribe|clone)/);
  assert.doesNotMatch(types, /Serialize|Deserialize|Vec<|String|HashMap|Arc<|OwnedSemaphorePermit|FileSaveTarget|FileVersionRef|reqwest|sqlx|std::fs|spawn|FnMut|FnOnce/);
  assert.equal((await read("engine/src/services/file_transfer/staging/save.rs")).match(/```compile_fail/g)?.length, 3);
});

test("one latest copied watch slot has no retained event queue or observer-controlled admission", async () => {
  const s = code(await read(`${dir}progress.rs`)).split("\n#[cfg(test)]")[0];
  for (const phrase of ["watch::channel(None)", "watch::Sender<Option<FileDeliverySnapshot>>", "watch::Receiver<Option<FileDeliverySnapshot>>", "send_replace(Some(FileDeliverySnapshot", "*self.receiver.borrow()", "self.receiver.changed().await.ok()?", "*self.receiver.borrow_and_update()"]) assert.ok(s.includes(phrase), phrase);
  assert.match(s, /pub fn channel\(\) -> \(Self, FileDeliveryProgress\)/);
  assert.match(s, /pub fn latest\(&self\) -> Option<FileDeliverySnapshot>/);
  assert.match(s, /pub async fn changed\(&mut self\) -> Option<FileDeliverySnapshot>/);
  assert.doesNotMatch(s, /pub[^\n]*(?:watch::|Ref<)|mpsc|unbounded|Semaphore|\.send\([^)]*\)\.await|sleep|timeout|Mutex|RwLock/);
});

test("the shared live core reports actual ACK prefixes before assembly and only original save ACK completes", async () => {
  const delivery = code(await read(`${dir}delivery.rs`)).split("\n#[cfg(test)]")[0];
  const start = delivery.indexOf("async fn run_inner(");
  assert.ok(start >= 0);
  const preflight = delivery.slice(delivery.indexOf("pub(super) fn validate_inputs("), delivery.indexOf("pub(super) async fn run("));
  ordered(preflight, ["manifest_inputs_valid(inner, token, expected_file)", "validate_save_target(target)", "FileGrantedSaveError::InvalidInput", "Ok(())"]);
  assert.doesNotMatch(preflight, /try_acquire|manifest::fetch|std::fs|tokio::fs|spawn/);
  const s = delivery.slice(start);
  ordered(s, ["validate_inputs(inner, token, expected_file, target)?", ".downloads", "try_acquire_owned", "FileDeliveryPhase::FetchingManifest", "manifest::fetch(", "FileDeliveryPhase::StagingChunks", "download::run_with_flow_observed(", "observer.as_ref()", "FileDeliveryPhase::Saving", ".save_assembled_file_leased(", "FileGrantedSaveError::Save)?", "FileDeliveryPhase::Complete", "Ok(saved)"]);
  assert.equal((s.match(/try_acquire_owned/g) || []).length, 1);
  const wrappers = delivery.slice(delivery.indexOf("pub(super) async fn run("), start);
  assert.match(wrappers, /target,\s*None/);
  assert.match(wrappers, /target,\s*Some\(observer\)/);
  const download = code(await read(`${dir}download.rs`));
  const begin = download.indexOf("pub(super) async fn run_with_flow_observed(");
  assert.ok(begin >= 0);
  ordered(download.slice(begin), ["for index in 0..manifest.chunk_sha256().len() as u32", "let staged = request::fetch_with_flow(", ".await", "FileDownloadError::Chunk { index, error })?", "staged.byte_length()", "FileDeliveryPhase::StagingChunks", "FileDeliveryPhase::Assembling", ".assemble_staged_file_leased(manifest, Some(flow))"]);
  assert.match(download.slice(0, begin), /run_with_flow_observed\([\s\S]*?flow,\s*None/);
  assert.doesNotMatch(s + download, /Semaphore::new|tokio::spawn|spawn_blocking|join_all|Vec<|remove_|rename\(|\.display_name|acquire_owned\(\)\.await/);
  for (const path of ["engine/src/services/file_transfer/staging/work_save.rs", "engine/src/services/file_transfer/staging/work_assemble.rs", "engine/src/services/file_chunk_receiver/leased.rs"]) assert.doesNotMatch(code(await read(path)), /FileDeliveryObserver|FileDeliveryProgress|FileDeliverySnapshot/, path);
});

test("actual progress default inventory and existing Source authorization exercise new and old operations", async () => {
  const main = await read("engine/tests/file_delivery_progress_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  let count = 0;
  for (const file of await readdir(new URL("engine/tests/file_delivery_progress/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_delivery_progress/${file}`);
    const tests = (s.match(/#\[(?:tokio::)?test\]/g) || []).length;
    count += tests;
    assert.doesNotMatch(s, /#\[ignore/);
    if (tests) assert.match(main, new RegExp(`\\bmod ${file.replace(/\.rs$/, "")};`));
  }
  assert.equal(count, 10);
  const baseline = await read("engine/tests/file_delivery_progress/baseline.rs");
  ordered(baseline, ["checked(", "result.expect("]);
  for (const phrase of ["fetch_granted_and_save_observed", "FILE_TRANSFER_CHUNK_BYTES + 17", "progress.changed()", "observe_output", "source_entries", "selected_entries", "verify_chunk", "assert_request"]) assert.ok(baseline.includes(phrase), phrase);
  const source = await read("engine/tests/file_delivery/source.rs");
  for (const phrase of ["fetch_granted_and_save(", "fetch_granted_and_save_observed(", "variant == 0", "ended_observer", "FileDeliveryPhase::Complete", "pre_chunk", "Some(pre_chunk)", "rejected_empty", "FileDeliveryPhase::FetchingManifest", "assert_saved_copy", "recipient_delta", "f.sharing.snapshot()", "take_statuses"]) assert.ok(source.includes(phrase), phrase);
  assert.equal((source.match(/#\[ignore/g) || []).length, 2);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-delivery.sh"));
});

test("progress remains local metadata with current bilingual limits and no normal-app SDK path", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /fetch_granted_and_save_observed|FileDeliveryObserver|FileDeliveryProgress|FileDeliverySnapshot/, path);
  for (const lang of ["en", "zh-CN"]) {
    const s = await read(`docs/${lang}/large-file-delivery-progress.md`);
    for (const phrase of ["0.5.7", "0.3.0", "2026-10-13", "channel()", "latest()", "changed().await", "staged_chunks()", "staged_bytes()", "total_bytes()", "total_chunks()", "FetchingManifest", "StagingChunks", "Assembling", "Saving", "Complete", "None", "file_delivery_progress_tdd", "fileDeliveryProgress", "106", "48"]) assert.ok(s.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-delivery-client", "large-file-user-save"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-delivery-progress.md`);
  }
});
