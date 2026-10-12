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

// Composition/inventory guards only; Rust cases establish actual byte and database behavior.
test("upload-bound staging keeps filesystem and SQL responsibilities separate without live authority", async () => {
  assert.match(codeOnly(await read("engine/src/services/mod.rs")), /#\[cfg\(unix\)\]\s*pub mod file_upload_staging;/);
  for (const [path, source] of await sources("engine/src/services/file_upload_staging")) {
    let code = codeOnly(source);
    if (path === "engine/src/services/file_upload_staging/publication/phase.rs") {
      // Later publication composition borrows a transaction type, not a connection or SQL owner.
      assert.match(code, /sqlx_compat::Postgres,/);
      assert.match(code, /use sqlx_core::transaction::Transaction;/);
      assert.doesNotMatch(code, /sqlx::query|\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS/, path);
      code = code.replace("sqlx_compat::Postgres,", "").replace("use sqlx_core::transaction::Transaction;", "");
    }
    assert.doesNotMatch(code, /sqlx|PgPool|AuthService|reqwest|spawn_blocking|std::(?:fs|net|process|env)|tokio::fs|install_empty|remove_file|remove_dir|Serialize|Deserialize/, path);
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /FileUploadStaging|UploadChunkAttempt|UploadChunkReceipt|services::file_upload_staging/, path);
  }
});

test("upload-bound source and file proof inventory retains private original-object checks", async () => {
  const adapter = codeOnly(await read("engine/src/services/file_upload_staging/mod.rs"));
  assert.match(adapter, /Arc::ptr_eq/);
  assert.match(adapter, /attempt\.state = UploadChunkState::Unknown/);
  assert.match(adapter, /verify_pin/);
  assert.match(adapter, /verify_bound_receipt/);
  const pins = codeOnly(await read("engine/src/services/file_upload_store/pins.rs"));
  assert.match(pins, /SET TRANSACTION ISOLATION LEVEL READ COMMITTED/);
  assert.match(pins, /pin\.schema\.verify/);
  assert.match(pins, /stored != pin\.reservation/);
  assert.doesNotMatch(pins, /INSERT INTO|UPDATE |DELETE FROM|spawn_blocking/);
  const proof = codeOnly(await read("engine/src/services/file_transfer/staging/bound.rs"));
  assert.match(proof, /pub\(crate\) struct BoundChunkReceipt/);
  assert.match(proof, /retained: File/);
  assert.doesNotMatch(proof, /derive|pub fn|pub retained|try_clone|Serialize|Deserialize/);
  assert.match(codeOnly(await read("engine/src/services/file_transfer/staging/work_bound.rs")), /work::admit\(inner\)/);
});

test("disposable upload-bound SQL runner exactly inventories every wired ignored case", async () => {
  const ignored = [];
  const target = await read("engine/tests/file_upload_staging_tdd.rs");
  assert.match(target, /file_upload_staging\/mod\.rs/);
  for (const match of target.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
    ignored.push(`baseline::${match[1]}`);
  }
  const wiring = await read("engine/tests/file_upload_staging/mod.rs");
  for (const [path, source] of await sources("engine/tests/file_upload_staging")) {
    const module = path.split("/").at(-1).replace(/\.rs$/, "");
    for (const match of source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)) {
      assert.match(wiring, new RegExp(`\\bmod ${module};`), `${module} must be wired`);
      ignored.push(`cases::${module}::${match[1]}`);
    }
  }
  const runner = await read("scripts/test-file-upload-staging.sh");
  const inventory = [...runner.matchAll(/\b(?:baseline|cases)::[\w:]+/g)].map(match => match[0]);
  assert.ok(ignored.length > 1);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /\b(?:rg|grep) -Fx/, /--ignored --exact --nocapture/, /--test file_upload_staging_tdd/]) {
    assert.match(runner, pattern);
  }
  assert.ok((await read(".github/workflows/ci.yml")).includes("run: bash scripts/test-file-upload-staging.sh"));
});

test("bilingual upload-bound docs distinguish local receipts from durable progress", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const docs = await read(`docs/${lang}/large-file-upload-staging.md`);
    for (const phrase of ["FileUploadStaging", "Ready", "Unknown", "Confirmed", "READ COMMITTED", "READ ONLY", "O_EXCL", "test-file-upload-staging.sh"]) {
      assert.ok(docs.includes(phrase), `${lang}: ${phrase}`);
    }
    assert.ok((await read(`docs/${lang}/README.md`)).includes("large-file-upload-staging.md"));
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileUploadStaging"));
  }
});
