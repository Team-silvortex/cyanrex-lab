import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_chunk_client/";
const ordered = (s, words) => {
  let last = -1;
  for (const word of words) {
    const at = s.indexOf(word);
    assert.ok(at >= 0 && at > last, `present and ordered ${word}`);
    last = at;
  }
};

// Static composition/inventory only; actual native HTTP, file jobs and SQL prove behavior.
test("controlled delivery consumes separate opaque cancellation without changing existing errors", async () => {
  const s = code(await read(`${dir}mod.rs`));
  assert.match(s, /pub async fn fetch_granted_and_save_controlled\(\s*&self,\s*token: &str,\s*grant_id: FileGrantId,\s*upload_id: FileUploadId,\s*expected_file: &FileVersionRef,\s*target: &FileSaveTarget,\s*observer: FileDeliveryObserver,\s*cancellation: FileDeliveryCancellation,?\s*\) -> Result<SavedFile, FileControlledSaveError>/);
  assert.ok(s.includes("controlled::run("));
  const controlled = code(await read(`${dir}controlled.rs`)).split("\n#[cfg(test)]")[0];
  assert.match(controlled, /pub enum FileControlledSaveError\s*\{[\s\S]*?Cancelled,[\s\S]*?Delivery\(FileGrantedSaveError\),/);
  const old = s.slice(s.indexOf("pub enum FileChunkClientError"), s.indexOf("struct Inner"));
  assert.doesNotMatch(old, /FileControlledSaveError|FileDeliveryCancellation|Cancelled/);
  assert.equal((await read("engine/src/services/file_transfer/staging/save.rs")).match(/```compile_fail/g)?.length, 3);
});

test("only an explicit oneshot unit request cancels and sender drop permanently disarms waiting", async () => {
  const s = code(await read(`${dir}cancel.rs`));
  for (const phrase of ["pub struct FileDeliveryCancelHandle", "sender: oneshot::Sender<()>", "pub struct FileDeliveryCancellation", "receiver: oneshot::Receiver<()>", "pub fn channel() -> (Self, FileDeliveryCancellation)", "oneshot::channel()", "pub fn cancel(self)", "self.sender.send(())", "pub(super) async fn requested(self)"]) assert.ok(s.includes(phrase), phrase);
  assert.match(s, /if self\.receiver\.await\.is_err\(\)\s*\{\s*std::future::pending::<\(\)>\(\)\.await;/);
  assert.doesNotMatch(s, /derive|Clone|Debug|Serialize|Deserialize|pub\s+(?:sender|receiver):|pub (?:async )?fn (?:poll|requested)|try_recv|loop|spawn|Semaphore|Arc<|String|FileSaveTarget|reqwest|sqlx|FnMut|FnOnce|std::fs/);
});

test("pure preflight precedes one biased cancellation-first selection over the original observed core", async () => {
  const delivery = code(await read(`${dir}delivery.rs`)).split("\n#[cfg(test)]")[0];
  const preflight = delivery.slice(delivery.indexOf("pub(super) fn validate_inputs("), delivery.indexOf("pub(super) async fn run("));
  ordered(preflight, ["manifest_inputs_valid(inner, token, expected_file)", "validate_save_target(target)", "FileGrantedSaveError::InvalidInput", "Ok(())"]);
  assert.doesNotMatch(preflight, /try_acquire|manifest::fetch|spawn|std::fs|tokio::fs/);
  const s = code(await read(`${dir}controlled.rs`)).split("\n#[cfg(test)]")[0];
  ordered(s, ["delivery::validate_inputs(inner, token, expected_file, target)", ".map_err(FileControlledSaveError::Delivery)?", "wait_for_delivery(", "delivery::run_observed(", "tokio::select!", "biased;", "cancellation.requested() => Err(FileControlledSaveError::Cancelled)", "result = delivery => result.map_err(FileControlledSaveError::Delivery)"]);
  assert.equal((s.match(/tokio::select!/g) || []).length, 1);
  assert.doesNotMatch(s, /try_acquire|Semaphore|spawn|Atomic|try_recv|std::fs|tokio::fs|remove_|rename\(|reqwest|sqlx|retry|timeout|sleep/);
  for (const path of ["engine/src/services/file_transfer/staging/work_save.rs", "engine/src/services/file_transfer/staging/work_assemble.rs", "engine/src/services/file_chunk_receiver/leased.rs"]) assert.doesNotMatch(code(await read(path)), /FileDeliveryCancelHandle|FileDeliveryCancellation|FileControlledSaveError|FileDeliveryObserver/, path);
});

test("actual cancellation defaults and existing Source cases retain independent output and authorization checks", async () => {
  const main = await read("engine/tests/file_delivery_cancel_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  let count = 0;
  for (const file of await readdir(new URL("engine/tests/file_delivery_cancel/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_delivery_cancel/${file}`);
    const tests = (s.match(/#\[(?:tokio::)?test\]/g) || []).length;
    count += tests;
    assert.doesNotMatch(s, /#\[ignore/);
    if (tests) assert.match(main, new RegExp(`\\bmod ${file.replace(/\.rs$/, "")};`));
  }
  assert.equal(count, 10);
  const baseline = await read("engine/tests/file_delivery_cancel/baseline.rs");
  ordered(baseline, ["checked(", "result.expect("]);
  for (const phrase of ["fetch_granted_and_save_controlled", "drop(handle)", "FILE_TRANSFER_CHUNK_BYTES + 17", "progress.changed()", "observe_output", "source_entries", "selected_entries", "verify_chunk", "assert_request"]) assert.ok(baseline.includes(phrase), phrase);
  const source = await read("engine/tests/file_delivery/source.rs");
  for (const phrase of ["fetch_granted_and_save(", "fetch_granted_and_save_observed(", "fetch_granted_and_save_controlled(", "handle.cancel()", "drop(handle)", "FileControlledSaveError::Delivery", "FileControlledSaveError::Cancelled", "assert_saved_copy", "recipient_delta", "f.sharing.snapshot()", "take_statuses"]) assert.ok(source.includes(phrase), phrase);
  assert.equal((source.match(/#\[ignore/g) || []).length, 2);
  assert.ok((await read("scripts/test-file-delivery.sh")).includes("--test file_delivery_tdd"));
});

test("cancellation stays unmounted with current bilingual cooperative and retained-work boundaries", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /fetch_granted_and_save_controlled|FileDeliveryCancelHandle|FileDeliveryCancellation|FileControlledSaveError/, path);
  for (const lang of ["en", "zh-CN"]) {
    const s = await read(`docs/${lang}/large-file-delivery-cancellation.md`);
    for (const phrase of ["0.5.7", "0.3.0", "2026-10-13", "FileDeliveryCancelHandle", "FileDeliveryCancellation", "FileControlledSaveError", "cancel(self)", "Delivery(InvalidInput)", "Cancelled", "0400", "file_delivery_cancel_tdd", "fileDeliveryCancel", "106", "50", "8"]) assert.ok(s.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-delivery-client", "large-file-delivery-progress"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-delivery-cancellation.md`);
  }
});
