import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Composition and inventory only; opt-in SQL establishes actual current authorization/fences.
test("manifest observation derives the recipient and consumes an exact independent grant", async () => {
  const s = code(await read("engine/src/services/auth_service/durable_source/file_download.rs"));
  for (const phrase of ["SessionFileDownloadWorkspace", "SessionFileDownloadRequest", "get_session_file_download_manifest", "FileAccountRef::new(context.owner, context.account_id())", "grant.state() != FileGrantState::Active", "grant.file() != &request.expected_file", "let publisher = grant.publisher()", "get_exact_in_transaction", "publisher.principal()", "publisher.account_id()", "FileVersionRef::from_manifest(record.manifest())", "file != request.expected_file", "READ COMMITTED", "finish_in_window", "tx.commit"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /Serialize|Deserialize|Default|READ WRITE|require_policy_manager|deployment_granted|role_refs|FileUploadPublisher|file_publisher_grant_store|require_private_subject|std::(?:fs|net|env)|tokio::fs|reqwest|install_empty/);
  const command = s.slice(s.indexOf("pub async fn get_session_file_download_manifest"), s.indexOf("fn check_file_download_scope"));
  assert.ok(command.lastIndexOf(".verify_pending_in_transaction") < command.indexOf(".finish_in_window"));
  assert.ok(command.indexOf(".finish_in_window") < command.indexOf("tx.commit"));
  assert.doesNotMatch(command.slice(command.indexOf(".finish_in_window"), command.indexOf("tx.commit")), /sqlx::query|select_target|verify_/);
  assert.ok(command.indexOf("tx.commit") < command.indexOf("Ok(manifest)"));
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /SessionFileDownload|get_session_file_download_manifest/, path);
});

test("recipient record views retain selected history and the original fenced fingerprint", async () => {
  const s = code(await read("engine/src/services/file_recipient_grant_store/transaction.rs"));
  for (const phrase of ["fn record(&self)", "schema: SchemaPin", "expected: Option<(Selected, [u8; 32])>", "IN SHARE MODE", "FOR SHARE", "pending.schema.verify", "current != pending.expected"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS|Serialize|Deserialize|derive\(Clone/);
});

test("publication reads filter exact file coordinates before decoding and retain the same predicate", async () => {
  const s = code(await read("engine/src/services/file_upload_publication/transaction.rs"));
  for (const phrase of ["get_exact_in_transaction", "exact: Option<(FileId, FileVersionId)>", "file.workspace() != self.scope", "pending.exact", "pending.schema.verify", "current != pending.expected"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET CONSTRAINTS|PublicationAcknowledgement/);
  const rows = code(await read("engine/src/services/file_upload_publication/records.rs"));
  for (const phrase of ["read_record_exact_with_lock(tx, upload_id, owner, None, share)", "($5::uuid IS NULL OR file_id=$5)", "($6::uuid IS NULL OR version_id=$6)", ".bind(exact.map(|value| value.0.as_uuid()))", ".bind(exact.map(|value| value.1.as_uuid()))", "canonical_manifest(&manifest)? != raw"]) assert.ok(rows.includes(phrase), phrase);
});

test("download manifest runner executes each wired ignored database case exactly", async () => {
  const names = source => [...source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/session_file_download_tdd.rs");
  const ignored = names(main);
  for (const file of await readdir(new URL("engine/tests/session_file_download/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/session_file_download/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  const runner = await read("scripts/test-session-file-download.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(ignored.length, 10);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test session_file_download_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-session-file-download.sh"));
});

test("bilingual manifest docs distinguish current metadata from future byte authority", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-download-manifest.md`);
    for (const phrase of ["SessionFileDownloadWorkspace", "SessionFileDownloadRequest", "SessionFileDownloadError", "get_session_file_download_manifest", "expected_file", "READ COMMITTED", "READ ONLY", "FOR SHARE", "0.5.6", "0.3.0", "test-session-file-download.sh"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-access-contract", "large-file-recipient-grants", "large-file-sharing"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-download-manifest.md"), `${lang}: ${file}`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileDownloadManifest"));
  }
});
