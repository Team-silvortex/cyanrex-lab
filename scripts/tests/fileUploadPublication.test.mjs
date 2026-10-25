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

// Composition and inventory only; real Rust cases establish byte, SQL and wait behavior.
test("publication uses a separate create-only trusted metadata store without live authority", async () => {
  assert.match(codeOnly(await read("engine/src/services/mod.rs")), /#\[cfg\(unix\)\]\s*pub mod file_upload_publication;/);
  for (const [path, source] of await sources("engine/src/services/file_upload_publication")) {
    assert.doesNotMatch(codeOnly(source), /UPDATE\s+collaboration_|DELETE FROM|ALTER TABLE|AuthService|std::(?:fs|net|process|env)|tokio::fs|reqwest|spawn_blocking/, path);
  }
  const ddl = codeOnly(await read("engine/migrations/0020_collaboration_file_publications.sql"));
  for (const pattern of [/upload_id UUID PRIMARY KEY/, /UNIQUE \(file_id, version_id\)/, /nonce UUID NOT NULL UNIQUE/, /manifest BYTEA/, /journal_key BYTEA/, /max_publications BETWEEN 1 AND 256/]) {
    assert.match(ddl, pattern);
  }
  assert.doesNotMatch(ddl, /confirmed|unknown|UPDATE |DELETE |ALTER TABLE|DROP TABLE/i);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /FileUploadPublisher|FileUploadPublication|file_upload_publication|0020_collaboration_file_publications/, path);
  }
});

test("publication privately composes original confirmed journal and same byte proof after acknowledgement", async () => {
  const surface = codeOnly(await read("engine/src/services/file_upload_staging/publication/mod.rs"));
  const publisher = surface + codeOnly(await read("engine/src/services/file_upload_staging/publication/phase.rs"));
  for (const pattern of [/Arc::ptr_eq/, /capture_confirmed_file/, /capture_pin/, /FileUploadPublicationState::Unknown/, /verify_confirmed_file/, /observe_bound_file/, /recheck_bound_file_retained/, /verify_ack/, /recheck_bound_file/]) {
    assert.match(publisher, pattern);
  }
  assert.doesNotMatch(publisher, /UploadFileObservation|Serialize|Deserialize|pub fn restore|PgPool|install_empty/);
  assert.doesNotMatch(surface, /sqlx/);
  const journal = codeOnly(await read("engine/src/services/file_upload_chunk_journal/publication_pin.rs"));
  assert.match(journal, /WHERE upload_id=\$1 ORDER BY chunk_index LIMIT \$2/);
  assert.match(journal, /SET TRANSACTION ISOLATION LEVEL READ COMMITTED/);
  assert.match(journal, /pin\.schema\.verify/);
  assert.match(journal, /row\.fingerprint\(\)/);
  assert.doesNotMatch(journal, /INSERT INTO|UPDATE |DELETE FROM|std::fs|observe_bound_file|UploadFileObservation/);
  const fs = codeOnly(await read("engine/src/services/file_transfer/staging/whole.rs")).split("#[cfg(test)]")[0];
  assert.match(fs, /pub\(super\) fn recheck_retained/);
  assert.match(fs, /Ok\(observation\)/);
  const workers = codeOnly(await read("engine/src/services/file_transfer/staging/work_whole.rs")).split("#[cfg(test)]")[0];
  assert.match(workers, /pub\(super\) async fn recheck_retained/);
  assert.match(workers, /let _permit = permit/);
});

test("publication runner exactly inventories every wired disposable SQL case", async () => {
  const ignored = [];
  const target = await read("engine/tests/file_upload_publication_tdd.rs");
  assert.match(target, /file_upload_publication\/mod\.rs/);
  for (const match of target.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
    ignored.push(`baseline::${match[1]}`);
  }
  const wiring = await read("engine/tests/file_upload_publication/mod.rs");
  for (const [path, source] of await sources("engine/tests/file_upload_publication")) {
    const module = path.split("/").at(-1).replace(/\.rs$/, "");
    for (const match of source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
      assert.match(wiring, new RegExp(`\\bmod ${module};`), `${module} must be wired`);
      ignored.push(`cases::${module}::${match[1]}`);
    }
  }
  const runner = await read("scripts/test-file-upload-publication.sh");
  const inventory = [...runner.matchAll(/\b(?:baseline|cases)::[\w:]+/g)].map(match => match[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.equal(ignored.length, 13);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /\b(?:rg|grep) -Fx/, /--ignored --exact --nocapture/, /--test file_upload_publication_tdd/]) {
    assert.match(runner, pattern);
  }
  assert.ok((await read(".github/workflows/ci.yml")).includes("run: bash scripts/test-file-upload-publication.sh"));
});

test("bilingual publication docs distinguish persistent metadata from restored authority", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const docs = await read(`docs/${lang}/large-file-publication.md`);
    for (const phrase of ["FileUploadPublisher", "FileUploadPublicationStore", "FileUploadPublicationAttempt", "FileUploadPublicationRecord", "Ready", "Unknown", "Confirmed", "NotReady", "READ COMMITTED", "READ ONLY", "64 KiB", "16,384", "1–256", "test-file-upload-publication.sh"]) {
      assert.ok(docs.includes(phrase), `${lang}: ${phrase}`);
    }
    await assertDocumentationReference(`docs/${lang}/README.md`, `docs/${lang}/large-file-publication.md`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileUploadPublication"));
  }
});
