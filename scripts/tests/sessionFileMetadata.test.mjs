import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Composition/inventory only. Real disposable SQL cases establish authorization and locking.
test("Session file metadata is explicit and does not connect publication or byte access", async () => {
  const source = codeOnly(await read("engine/src/services/auth_service/durable_source/file_commands/mod.rs"));
  for (const name of ["reserve_session_file_upload", "get_session_file_upload", "get_session_file_publication"]) {
    assert.ok(source.includes(name));
  }
  for (const pattern of [/begin_private_work/, /context\.owner/, /context\.account_id\(\)/, /READ COMMITTED/, /context\.finish/, /tx\.commit/, /pending\.into_record/]) {
    assert.match(source, pattern);
  }
  assert.doesNotMatch(source, /FileUploadPublisher|FileUploadPublicationAttempt|std::(?:fs|net|env)|tokio::fs|reqwest|\.publish\(|\.reserve\(|\.get\(|READ WRITE|install_empty|read_session_artifact/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /SessionFile(?:Metadata|Upload|Publication)|session_file_upload|session_file_publication/, path);
  }
});

test("borrowed file helpers retain original pins and leave commit ownership with source", async () => {
  for (const dir of ["file_upload_store", "file_upload_publication"]) {
    const source = codeOnly(await read(`engine/src/services/${dir}/transaction.rs`));
    for (const pattern of [/get_in_transaction/, /verify_pending_in_transaction/, /pending\.schema\.verify/, /FOR SHARE/, /share/, /into_record/]) {
      assert.match(source, pattern, dir);
    }
    assert.doesNotMatch(source, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS|derive\(Clone|Serialize|Deserialize/, dir);
  }
  const guard = codeOnly(await read("engine/src/services/auth_service/durable_source/private_work/mod.rs"));
  assert.match(guard, /relations: RelationPin/);
  assert.match(guard, /let relations = RelationPin::capture/);
  assert.match(guard, /self\.relations\.verify/);
});

test("Session file metadata runner exactly inventories wired ignored cases", async () => {
  const target = await read("engine/tests/session_file_metadata_tdd.rs");
  const wiring = await read("engine/tests/session_file_metadata/mod.rs");
  const ignored = [...target.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => `baseline::${m[1]}`);
  for (const entry of await readdir(new URL("engine/tests/session_file_metadata/", root))) {
    if (!entry.endsWith(".rs")) continue;
    const module = entry.replace(/\.rs$/, "");
    const source = await read(`engine/tests/session_file_metadata/${entry}`);
    for (const m of source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
      assert.match(wiring, new RegExp(`\\bmod ${module};`));
      ignored.push(`cases::${module}::${m[1]}`);
    }
  }
  const runner = await read("scripts/test-session-file-metadata.sh");
  const inventory = [...runner.matchAll(/\b(?:baseline|cases)::[\w:]+/g)].map(m => m[0]);
  assert.equal(ignored.length, 10);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /\b(?:rg|grep) -Fx/, /--ignored --exact --nocapture/, /--test session_file_metadata_tdd/]) {
    assert.match(runner, pattern);
  }
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-session-file-metadata.sh"));
});

test("bilingual Session metadata docs separate private intent from distribution permission", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-session-metadata.md`);
    for (const phrase of ["SessionFileUploadWorkspace", "SessionFilePublicationWorkspace", "READ COMMITTED", "READ ONLY", "FOR SHARE", "Published", "test-session-file-metadata.sh", "0.5.5"]) {
      assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    }
    assert.ok((await read(`docs/${lang}/README.md`)).includes("large-file-session-metadata.md"));
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("sessionFileMetadata"));
  }
});
