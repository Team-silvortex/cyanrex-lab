import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// These guards establish composition and inventory only, not byte/SQL/authority acceptance.
test("Session publication derives exact authority and shares final insert with source authorization", async () => {
  const source = code(await read("engine/src/services/auth_service/durable_source/file_publishing.rs"));
  for (const phrase of ["SessionFilePublishingWorkspace", "file_publishing_workspace", "publish_session_file_upload", "FileAccountRef::new(context.owner, context.account_id())", "FileAccountRef::new(run.owner(), run.account_id())", "refence_pending_in_transaction", "context.resume", ".finish_in_window", "READ COMMITTED", "SET CONSTRAINTS ALL IMMEDIATE"]) {
    assert.ok(source.includes(phrase), phrase);
  }
  assert.doesNotMatch(source, /Deserialize|Serialize|Default|role_refs|deployment_granted|require_policy_manager|READ WRITE|install_empty|\.publish\(/);
  assert.ok(source.indexOf("publisher.start") < source.indexOf("begin_private_work"));
  assert.ok(source.indexOf("context.finish_retained") < source.indexOf("publisher.observe"));
  const final = source.slice(source.indexOf("let pending = publication_sql"));
  assert.ok(final.indexOf("context.resume") < final.indexOf("refence_pending_in_transaction"));
  assert.ok(final.indexOf("refence_pending_in_transaction") < final.indexOf("publish_in_transaction"));
  assert.ok(final.indexOf("SET CONSTRAINTS ALL IMMEDIATE") < final.indexOf("verify_publication_in_transaction"));
  assert.ok(final.indexOf("finish_in_window") < final.indexOf("tx.commit"));
  assert.ok(final.indexOf("tx.commit") < final.indexOf("publisher.finish"));
  assert.doesNotMatch(final.slice(final.indexOf("finish_in_window"), final.indexOf("tx.commit")), /sqlx::query|select_target|verify_/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(code(await read(path)), /SessionFilePublishing|publish_session_file_upload|file_publishing_workspace/, path);
  }
});

test("borrowed publication and retained proofs cannot independently commit or adopt original pins", async () => {
  const mutation = code(await read("engine/src/services/file_upload_publication/mutation.rs"));
  for (const phrase of ["schema: PublicationSchemaPin", "expected: PublishedRow", "ledger: Ledger", "verify_mutation_schema", "&pending.schema", "pending.selected_namespace", "pin.verify_selected", "pin.verify", "stored != pending.expected", "pending.fingerprint", "self.ledger", "pending.ledger", "into_acknowledgement", "INSERT INTO collaboration_file_publications"]) {
    assert.ok(mutation.includes(phrase), phrase);
  }
  assert.doesNotMatch(mutation, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS|Serialize|Deserialize|derive\(Clone/);
  const trusted = code(await read("engine/src/services/file_upload_publication/commands.rs"));
  assert.match(trusted, /publish_pending_in_transaction\([\s\S]*?false\)/);
  for (const store of ["file_upload_store", "file_upload_chunk_journal", "file_upload_publication"]) {
    const namespace = code(await read(`engine/src/services/${store}/namespace.rs`));
    assert.match(namespace, /self\.name == current\.name\s*&& self\.oid == current\.oid/);
    assert.ok(namespace.includes("current.search_path == current.name"));
    assert.ok(namespace.includes('format!("\\\"{}\\\"", current.name)'));
    assert.match(namespace, /Self::capture\((store|journal), tx\)\.await\? != \*self/);
  }
  const phase = code(await read("engine/src/services/file_upload_staging/publication/phase.rs"));
  for (const phrase of ["OwnedSemaphorePermit", "BoundFileObservation", "FileUploadPublicationState::Unknown", "FileUploadPublicationState::Confirmed", "recheck_bound_file_retained", "verify_pin_in_transaction", "verify_confirmed_file_in_transaction", "into_acknowledgement", "recheck_bound_file"]) {
    assert.ok(phase.includes(phrase), phrase);
  }
  assert.doesNotMatch(phase, /sqlx::query|SET LOCAL|\.commit\(|Serialize|Deserialize|derive\(Clone/);
  const grants = code(await read("engine/src/services/file_publisher_grant_store/transaction.rs"));
  assert.match(grants, /refence_pending_in_transaction[\s\S]*pending\.schema\.verify[\s\S]*IN SHARE MODE/);
  const timed = code(await read("engine/src/services/auth_service/durable_source/private_work/timed.rs"));
  for (const phrase of ["self.source.verify", "self.relations.verify", "session_private_work_actor", "finish_retained", "clock_timestamp", "session_valid", "window_valid"]) assert.ok(timed.includes(phrase), phrase);
  assert.doesNotMatch(timed, /FileGrant|FileAccount|FileUpload|\.pool|\.begin\(|\.commit\(/);
});

test("bilingual Session file-publication documentation preserves timing and delivery boundaries", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-session-publication.md`);
    for (const phrase of ["SessionFilePublishingWorkspace", "SessionFilePublishingError", "FileUploadPublicationAttempt", "publish_session_file_upload", "READ COMMITTED", "READ ONLY", "SHARE", "SET CONSTRAINTS ALL IMMEDIATE", "Ready", "Unknown", "Confirmed", "64 KiB", "0.5.6", "0.3.0", "test-session-file-publication.sh"]) {
      assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    }
    for (const name of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-access-contract", "large-file-grant-management", "large-file-publisher-grants", "large-file-publication"]) {
      await assertDocumentationReference(`docs/${lang}/${name}.md`, `docs/${lang}/large-file-session-publication.md`);
    }
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileSessionPublication"));
  }
});

test("Session file-publication runner exactly selects every wired disposable PostgreSQL case", async () => {
  const names = source => [...source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/session_file_publication_tdd.rs");
  const ignored = names(main).map(name => `baseline::${name}`);
  for (const file of await readdir(new URL("engine/tests/session_file_publication/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/session_file_publication/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  const runner = await read("scripts/test-session-file-publication.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(ignored.length, 10);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test session_file_publication_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-session-file-publication.sh"));
});
