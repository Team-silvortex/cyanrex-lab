import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Composition and inventory only; real Rust cases establish byte and wait behavior.
test("whole upload observation reuses the original source and workers without journal or live authority", async () => {
  const source = codeOnly(await read("engine/src/services/file_upload_staging/whole.rs"));
  for (const pattern of [/pub async fn verify_file/, /\.admit\(\)/, /capture_pin/, /bind_upload/, /observe_bound_file/, /verify_pin/, /recheck_bound_file/]) {
    assert.match(source, pattern);
  }
  assert.doesNotMatch(source, /sqlx|PgPool|Serialize|Deserialize|FileUploadChunkJournal|JournaledFileUploadStaging|UploadChunkReceipt|PersistentChunkState|std::fs|tokio::fs|AuthService|pub fn restore/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /UploadFileObservation|UploadFileVerificationError|observe_bound_file|recheck_bound_file/, path);
  }
});

test("whole filesystem observation remains read only under the original fail fast worker gate", async () => {
  const worker = codeOnly(await read("engine/src/services/file_transfer/staging/work_whole.rs"));
  assert.match(worker, /work::admit/);
  assert.match(worker, /spawn_blocking/);
  const whole = codeOnly(await read("engine/src/services/file_transfer/staging/whole.rs"));
  assert.match(whole, /#\[cfg\(test\)\]\s*#\[path = "whole_tests\.rs"\]\s*mod tests;/);
  assert.match(whole.split("#[cfg(test)]")[0], /pub\(super\) fn recheck\(/);
  assert.match(whole, /verify_stream/);
  assert.match(whole, /bound_chunk_name/);
  assert.match(whole, /Snapshot/);
  for (const source of [worker, whole]) {
    assert.doesNotMatch(source, /sqlx|PgPool|FileUploadChunkJournal|copy_chunk_checked|stage_named|O_CREAT|create_new|sync_all|set_permissions|remove_file|remove_dir|reqwest|std::net/);
  }
  const wiring = codeOnly(await read("engine/src/services/file_transfer/staging/mod.rs"));
  assert.match(wiring, /pub\(crate\) async fn observe_bound_file/);
  assert.match(wiring, /pub\(crate\) async fn recheck_bound_file/);
  assert.doesNotMatch(wiring, /pub async fn (?:observe_bound_file|recheck_bound_file)/);
});

test("whole upload disposable SQL runner exactly inventories every wired ignored case", async () => {
  const ignored = [];
  const target = await read("engine/tests/file_upload_whole_tdd.rs");
  assert.match(target, /file_upload_whole\/mod\.rs/);
  for (const match of target.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
    ignored.push(`baseline::${match[1]}`);
  }
  const directory = "engine/tests/file_upload_whole";
  const wiring = await read(`${directory}/mod.rs`);
  for (const file of await readdir(new URL(directory, root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const source = await read(`${directory}/${file}`);
    for (const match of source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
      assert.match(wiring, new RegExp(`\\bmod ${module};`), `${module} must be wired`);
      ignored.push(`cases::${module}::${match[1]}`);
    }
  }
  const runner = await read("scripts/test-file-upload-whole.sh");
  const inventory = [...runner.matchAll(/\b(?:baseline|cases)::[\w:]+/g)].map(match => match[0]);
  assert.ok(ignored.length > 1);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /\b(?:rg|grep) -Fx/, /--ignored --exact --nocapture/, /--test file_upload_whole_tdd/]) {
    assert.match(runner, pattern);
  }
  assert.ok((await read(".github/workflows/ci.yml")).includes("run: bash scripts/test-file-upload-whole.sh"));
});

test("bilingual whole upload docs separate current bytes from historical receipt and publication", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const docs = await read(`docs/${lang}/large-file-whole-verification.md`);
    for (const phrase of ["FileUploadStaging::verify_file", "UploadFileObservation", "UploadFileVerificationError", "64 KiB", "16,384", "Unknown", "Confirmed", "test-file-upload-whole.sh"]) {
      assert.ok(docs.includes(phrase), `${lang}: ${phrase}`);
    }
    await assertDocumentationReference(`docs/${lang}/README.md`, `docs/${lang}/large-file-whole-verification.md`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileUploadWhole"));
  }
});
