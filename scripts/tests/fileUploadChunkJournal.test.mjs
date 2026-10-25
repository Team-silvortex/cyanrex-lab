import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function sources(directory) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sources(path));
    else if (entry.name.endsWith(".rs")) result.push([path, await read(path)]);
  }
  return result;
}

// Inventory/composition only; real Rust cases establish committed ordering and byte behavior.
test("chunk journal stays independently installed and never becomes live authority or file IO", async () => {
  assert.match(codeOnly(await read("engine/src/services/mod.rs")), /#\[cfg\(unix\)\]\s*pub mod file_upload_chunk_journal;/);
  for (const [path, source] of await sources("engine/src/services/file_upload_chunk_journal")) {
    assert.doesNotMatch(codeOnly(source), /AuthService|ChunkStaging|reqwest|spawn_blocking|std::(?:fs|net|process|env)|tokio::fs|remove_file|remove_dir|DELETE FROM|\.migrate\(/, path);
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /FileUploadChunkJournal|JournaledFileUploadStaging|PersistentChunkState|services::file_upload_chunk_journal/, path);
  }
  assert.doesNotMatch(await read("engine/migrations/0018_collaboration_file_uploads.sql"), /file_upload_chunks|chunk_index|confirmed|nonce/);
});

test("write-ahead inventory preserves private capabilities and fixed unique coordinates", async () => {
  const journal = (await sources("engine/src/services/file_upload_chunk_journal")).map(([,source]) => codeOnly(source)).join("\n");
  assert.match(journal, /MAX_FILE_UPLOAD_CHUNK_ENTRIES: u32 = 16_384/);
  assert.match(journal, /pub\(crate\) async fn reserve_unknown/);
  assert.match(journal, /pub\(crate\) async fn confirm/);
  assert.doesNotMatch(journal, /pub async fn (?:reserve_unknown|confirm)/);
  assert.match(journal, /SHARE ROW EXCLUSIVE MODE/);
  assert.match(journal, /SET TRANSACTION ISOLATION LEVEL READ COMMITTED/);
  assert.match(journal, /SET CONSTRAINTS ALL IMMEDIATE/);
  const ddl = await read("engine/migrations/0019_collaboration_file_upload_chunks.sql");
  assert.match(ddl, /PRIMARY KEY \(upload_id, chunk_index\)/);
  assert.match(ddl, /nonce UUID NOT NULL UNIQUE/);
  assert.doesNotMatch(ddl, /manifest|ALTER TABLE|DROP TABLE/);
  const wrapper = codeOnly(await read("engine/src/services/file_upload_staging/journaled/mod.rs"));
  assert.match(wrapper, /self\.inner\.local\.admit\(\)/);
  assert.match(wrapper, /attempt\.local\.state = UploadChunkState::Unknown/);
  assert.match(wrapper, /reserve_unknown/);
  assert.match(wrapper, /\.journal\s*\.confirm/);
  assert.match(wrapper, /verify_local_proof/);
  assert.doesNotMatch(wrapper, /Serialize|Deserialize|pub fn restore|remove_file|remove_dir|sqlx|PgPool/);
});

test("disposable chunk journal SQL runner exactly selects all wired ignored cases", async () => {
  const ignored = [];
  const target = await read("engine/tests/file_upload_chunk_journal_tdd.rs");
  assert.match(target, /file_upload_chunk_journal\/mod\.rs/);
  for (const match of target.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
    ignored.push(`baseline::${match[1]}`);
  }
  const wiring = await read("engine/tests/file_upload_chunk_journal/mod.rs");
  for (const [path, source] of await sources("engine/tests/file_upload_chunk_journal")) {
    const module = path.split("/").at(-1).replace(/\.rs$/, "");
    for (const match of source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
      assert.match(wiring, new RegExp(`\\bmod ${module};`), `${module} must be wired`);
      ignored.push(`cases::${module}::${match[1]}`);
    }
  }
  const runner = await read("scripts/test-file-upload-chunk-journal.sh");
  const inventory = [...runner.matchAll(/\b(?:baseline|cases)::[\w:]+/g)].map(match => match[0]);
  assert.ok(ignored.length > 1);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /\b(?:rg|grep) -Fx/, /--ignored --exact --nocapture/, /--test file_upload_chunk_journal_tdd/]) {
    assert.match(runner, pattern);
  }
  assert.ok((await read(".github/workflows/ci.yml")).includes("run: bash scripts/test-file-upload-chunk-journal.sh"));
});

test("bilingual journal docs distinguish historical records from recovery and original-file proof", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const docs = await read(`docs/${lang}/large-file-chunk-journal.md`);
    for (const phrase of ["FileUploadChunkJournal", "JournaledFileUploadStaging", "UploadChunkRecord", "Unknown", "Confirmed", "READ COMMITTED", "16,384", "test-file-upload-chunk-journal.sh"]) {
      assert.ok(docs.includes(phrase), `${lang}: ${phrase}`);
    }
    await assertDocumentationReference(`docs/${lang}/README.md`, `docs/${lang}/large-file-chunk-journal.md`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileUploadChunkJournal"));
  }
});
