import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function rustSources(directory) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await rustSources(path));
    else if (entry.name.endsWith(".rs")) result.push([path, await read(path)]);
  }
  return result;
}

// Inventory/composition checks only, never evidence that a database transaction actually passed.
test("upload intent stays standalone and does not widen existing content contracts", async () => {
  assert.match(codeOnly(await read("engine/src/models/collaboration/scalars.rs")), /\bFileUploadId,/);
  assert.match(codeOnly(await read("engine/src/services/mod.rs")), /pub mod file_upload_store;/);
  assert.match(await read("engine/src/services/artifact_store/mod.rs"), /MAX_CONTENT: usize = 1_048_576/);
  assert.match(await read("engine/src/models/collaboration/content.rs"), /MAX_TEXT_PAYLOAD_BYTES: usize = 256 \* 1024/);
  for (const [path, source] of await rustSources("engine/src/services/file_upload_store")) {
    assert.doesNotMatch(codeOnly(source), /AuthService|ArtifactStore|TaskStore|ChunkStaging|reqwest|std::(?:fs|net|process|env)|tokio::fs|spawn_blocking|remove_file|DELETE FROM|\.migrate\(/, path);
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /FileUploadStore|FileUploadReservation|FileUploadId|services::file_upload_store/, path);
  }
});

test("upload ledger inventory retains bounded decoding and separate transaction responsibility", async () => {
  const service = await read("engine/src/services/file_upload_store/mod.rs");
  assert.match(service, /MAX_FILE_UPLOAD_INTENTS: u32 = 256/);
  const schema = codeOnly(await read("engine/src/services/file_upload_store/schema.rs"));
  assert.match(schema, /prepared_resource_sql::begin/);
  assert.match(schema, /prepared_resource_sql::ordinary_tables/);
  assert.match(schema, /SET TRANSACTION ISOLATION LEVEL READ COMMITTED/);
  assert.match(schema, /SHARE ROW EXCLUSIVE MODE/);
  assert.match(schema, /ACCESS SHARE MODE/);
  for (const column of ["namespace_oid", "schema_oid", "uploads_oid", "total_bytes", "owner_bytes", "total_uploads", "owner_uploads"]) {
    assert.ok(schema.includes(column), column);
    assert.ok((await read("engine/migrations/0018_collaboration_file_uploads.sql")).includes(column), column);
  }
  const ledger = codeOnly(await read("engine/src/services/file_upload_store/ledger.rs"));
  assert.match(ledger, /SELECT upload_id[^;]*LIMIT 257/);
  assert.match(ledger, /read_record/);
  const records = codeOnly(await read("engine/src/services/file_upload_store/records.rs"));
  assert.match(records, /CASE WHEN octet_length\(manifest\) <= \$4 THEN manifest ELSE NULL END/);
  assert.match(records, /FileTransferManifest::parse_json/);
  assert.match(records, /canonical_manifest\(&manifest\)\? != raw/);
  const commands = codeOnly(await read("engine/src/services/file_upload_store/commands.rs"));
  assert.match(commands, /SET CONSTRAINTS ALL IMMEDIATE/);
  assert.match(commands, /verify_pending_in_transaction\(&mut tx, &pending\)/);
  const borrowed = codeOnly(await read("engine/src/services/file_upload_store/transaction.rs"));
  assert.match(borrowed, /self\.ledger\(tx, pending\.owner\)\.await\? != \*expected/);
  assert.match(borrowed, /pending\.schema\.verify/);
  assert.match(commands, /SET LOCAL synchronous_commit = 'on'/);
});

test("disposable upload SQL runner inventory exactly matches wired ignored cases", async () => {
  const ignored = [];
  const target = await read("engine/tests/file_upload_intent_tdd.rs");
  assert.match(target, /file_upload_intent\/mod\.rs/);
  for (const match of target.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
    ignored.push(`baseline::${match[1]}`);
  }
  const wiring = await read("engine/tests/file_upload_intent/mod.rs");
  for (const [path, source] of await rustSources("engine/tests/file_upload_intent")) {
    const module = path.split("/").at(-1).replace(/\.rs$/, "");
    for (const match of source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
      assert.match(wiring, new RegExp(`\\bmod ${module};`), `${module} must be wired`);
      ignored.push(`cases::${module}::${match[1]}`);
    }
  }
  const runner = await read("scripts/test-file-upload-intent.sh");
  const inventory = [...runner.matchAll(/\b(?:baseline|cases)::[\w:]+/g)].map(match => match[0]);
  assert.ok(ignored.length > 1, "must include more than the TDD baseline");
  assert.equal(new Set(inventory).size, inventory.length, "duplicate runner names");
  assert.deepEqual(inventory.sort(), ignored.sort());
  assert.match(runner, /CYANREX_TEST_DATABASE_URL:\?/);
  assert.match(runner, /--ignored --list/);
  assert.match(runner, /\b(?:rg|grep) -Fx/);
  assert.match(runner, /--ignored --exact --nocapture/);
  assert.match(runner, /--test file_upload_intent_tdd/);
  const workflow = await read(".github/workflows/ci.yml");
  assert.ok(workflow.includes("run: bash scripts/test-file-upload-intent.sh"));
  assert.ok(workflow.includes("run: cargo test --manifest-path engine/Cargo.toml --locked"));
});

test("bilingual upload docs distinguish logical reservations from authenticated chunk progress", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const docs = await read(`docs/${lang}/large-file-upload-intent.md`);
    for (const phrase of ["FileUploadStore", "LegacyAccountId", "16 TiB", "256", "READ COMMITTED", "test-file-upload-intent.sh"]) {
      assert.ok(docs.includes(phrase), `${lang}: ${phrase}`);
    }
    assert.ok((await read(`docs/${lang}/README.md`)).includes("large-file-upload-intent.md"));
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileUploadIntent"));
  }
});
