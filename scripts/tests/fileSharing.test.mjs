import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Composition and exact inventory only. Real disposable SQL establishes actual authority/locking.
test("sharing derives the current publisher and compares the complete trusted version", async () => {
  const s = code(await read("engine/src/services/auth_service/durable_source/file_sharing.rs"));
  for (const phrase of ["SessionFileSharingWorkspace", "SessionFileRecipientGrantIssue", "issue_session_file_recipient_grant", "FileAccountRef::new(context.owner, context.account_id())", "FileVersionRef::from_manifest(record.manifest())", "file != issue.expected_file", "permits_pending_in_transaction", "require_private_subject", "SET CONSTRAINTS ALL IMMEDIATE", "finish_in_window", "READ COMMITTED", "tx.commit"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /Serialize|Deserialize|Default|require_policy_manager|deployment_granted|role_refs|READ WRITE|install_empty|FileUploadPublisher|std::(?:fs|net|env)|tokio::fs|reqwest/);
  const issue = s.slice(s.indexOf("pub async fn issue_session_file_recipient_grant"), s.indexOf("pub async fn revoke_session_file_recipient_grant"));
  assert.ok(issue.indexOf("SET CONSTRAINTS ALL IMMEDIATE") < issue.lastIndexOf(".verify_pending_in_transaction"));
  assert.ok(issue.lastIndexOf(".verify_pending_in_transaction") < issue.indexOf(".finish_in_window"));
  assert.ok(issue.indexOf(".finish_in_window") < issue.indexOf("tx.commit"));
  assert.doesNotMatch(issue.slice(issue.indexOf(".finish_in_window"), issue.indexOf("tx.commit")), /sqlx::query|select_target|verify_/);
  assert.ok(issue.indexOf("tx.commit") < issue.indexOf("pending.into_record"));
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /SessionFileSharing|SessionFileRecipientGrantIssue|issue_session_file_recipient_grant|revoke_session_file_recipient_grant/, path);
});

test("revocation uses the initial writer fence and never depends on live publication permission", async () => {
  const s = code(await read("engine/src/services/auth_service/durable_source/file_sharing.rs"));
  const revoke = s.slice(s.indexOf("pub async fn revoke_session_file_recipient_grant"), s.indexOf("fn check_file_sharing_scope"));
  for (const phrase of ["revoke_owned_in_transaction", "verify_mutation_in_transaction", "context.finish", "tx.commit", "pending.into_record"]) assert.ok(revoke.includes(phrase), phrase);
  assert.doesNotMatch(revoke, /get_in_transaction|require_private_subject|permits_pending|finish_in_window|workspace\.publication|workspace\.publishers|\.get\(|\.revoke\(/);
  const mutation = code(await read("engine/src/services/file_recipient_grant_store/mutation.rs"));
  assert.match(mutation, /self\.check_schema\(tx, true\)/);
  assert.match(mutation, /owned && previous\.publisher\(\) != actor/);
  const implementation = mutation.slice(mutation.indexOf("async fn revoke_pending_in_transaction"));
  assert.ok(implementation.indexOf("previous.publisher() != actor") < implementation.indexOf("UPDATE collaboration_file_recipient_grants"));
  assert.doesNotMatch(implementation, /previous\.issuer|IN SHARE MODE|FOR SHARE/);
});

test("borrowed mutation and publication views retain original pins without independent commit", async () => {
  const mutation = code(await read("engine/src/services/file_recipient_grant_store/mutation.rs"));
  for (const phrase of ["schema: SchemaPin", "selected: Selected", "fingerprint: [u8; 32]", "ledger: Ledger", "pending.schema.verify", "current != pending.selected", "self.ledger(tx).await? != pending.ledger"]) assert.ok(mutation.includes(phrase), phrase);
  assert.doesNotMatch(mutation, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS|Serialize|Deserialize|derive\(Clone/);
  const view = code(await read("engine/src/services/file_upload_publication/transaction.rs"));
  for (const phrase of ["schema: PublicationSchemaPin", "expected: Option<(PublishedRow, [u8; 32])>", "fn record(&self)", "pending.schema.verify", "current != pending.expected"]) assert.ok(view.includes(phrase), phrase);
  assert.doesNotMatch(view, /PublicationAcknowledgement|\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET CONSTRAINTS/);
});

test("sharing runner selects every wired ignored database case exactly", async () => {
  const names = source => [...source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/session_file_sharing_tdd.rs");
  const ignored = names(main).map(name => `baseline::${name}`);
  for (const file of await readdir(new URL("engine/tests/session_file_sharing/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/session_file_sharing/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  const runner = await read("scripts/test-session-file-sharing.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(ignored.length, 10);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test session_file_sharing_tdd/]) assert.match(runner, pattern);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-session-file-sharing.sh"));
});

test("bilingual sharing docs keep publisher ownership separate from download or operator delegation", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-sharing.md`);
    for (const phrase of ["SessionFileSharingWorkspace", "SessionFileRecipientGrantIssue", "SessionFileSharingError", "issue_session_file_recipient_grant", "revoke_session_file_recipient_grant", "expected_file", "READ COMMITTED", "READ ONLY", "SHARE ROW EXCLUSIVE", "SET CONSTRAINTS ALL IMMEDIATE", "0.5.6", "0.3.0", "test-session-file-sharing.sh"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-access-contract", "large-file-recipient-grants", "large-file-session-publication"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-sharing.md`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileSharing"));
  }
});
