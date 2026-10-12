import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const names = source => [...source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);

// Inventory and composition only; exact opt-in SQL establishes actual behavior and lock boundaries.
test("recipient ledger is independent trusted metadata with no live or download consumer", async () => {
  assert.match(await read("engine/src/services/mod.rs"), /pub mod file_recipient_grant_store;/);
  const source = code(await read("engine/src/services/file_recipient_grant_store/commands.rs"));
  for (const phrase of ["pub async fn issue", "pub async fn revoke", "pub async fn get", "issue_in_transaction", "revoke_trusted_in_transaction", "SET CONSTRAINTS ALL IMMEDIATE", "verify_mutation_in_transaction", "tx.commit"]) assert.ok(source.includes(phrase), phrase);
  const mutation = code(await read("engine/src/services/file_recipient_grant_store/mutation.rs"));
  for (const phrase of ["self.ledger", "schema.verify", "pending.ledger", "pending.selected", "fingerprint", "confirmed_one", "insert_change"]) assert.ok(mutation.includes(phrase), phrase);
  assert.doesNotMatch(mutation, /FilePublisherGrant|FileUpload|Registry|Runtime|std::(?:fs|net|env)|tokio::fs|reqwest|\.matches\(|\.begin\(|\.commit\(|SET LOCAL|SET CONSTRAINTS/);
  assert.doesNotMatch(source, /FilePublisherGrant|FileUpload|Registry|Runtime|std::(?:fs|net|env)|tokio::fs|reqwest|\.matches\(/);
  const ddl = await read("engine/migrations/0022_collaboration_file_recipient_grants.sql");
  for (const phrase of ["collaboration_file_recipient_grant_schema", "collaboration_file_recipient_grants", "collaboration_file_recipient_grant_changes", "publisher_account_id", "recipient_account_id", "issuer_account_id", "grant_payload BYTEA", "recorded_at BIGINT"]) assert.ok(ddl.includes(phrase), phrase);
  assert.doesNotMatch(ddl, /\bUNIQUE\b|file_id|version_id|byte_length|sha256/);
  const namespace = code(await read("engine/src/services/file_recipient_grant_store/namespace.rs"));
  assert.match(namespace, /search_path != store\.namespace/);
  assert.ok(namespace.includes('search_path != format!("\\\"{}\\\"", store.namespace)'));
  assert.match(namespace, /Self::capture\(store, tx\)\.await\? != \*self/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /SessionFileRecipientGrant|FileRecipientGrantStore|get_session_file_recipient_grant/, path);
});

test("recipient canonical history retains every exact file and account binding", async () => {
  const source = code(await read("engine/src/services/file_recipient_grant_store/records.rs"));
  for (const phrase of ["FileRecipientGrant::parse_json", "canonical(&grant)? != raw", "octet_length(grant_payload)", "history_grant.file() != selected_grant.file()", "history_grant.publisher() != selected_publisher", "history_grant.recipient() != selected_recipient", "actor != issuer", "recorded_at", "fingerprint"]) assert.ok(source.includes(phrase), phrase);
  const ledger = code(await read("engine/src/services/file_recipient_grant_store/ledger.rs"));
  for (const phrase of ["LIMIT $1", "self.max_grants", "coordinates != ledger.changes", "read_selected"]) assert.ok(ledger.includes(phrase), phrase);
  assert.doesNotMatch(source + ledger, /FilePublisherGrant|FileUpload|\.matches\(|std::fs|tokio::fs/);
});

test("own recipient read keeps borrowed original fences through final source authorization", async () => {
  const source = code(await read("engine/src/services/auth_service/durable_source/file_recipient_commands.rs"));
  for (const phrase of ["READ COMMITTED", "begin_private_work", "FileAccountRef::new(context.owner, context.account_id())", "get_in_transaction", "verify_pending_in_transaction", "context.finish", "tx.commit", "pending.into_record"]) assert.ok(source.includes(phrase), phrase);
  assert.ok(source.indexOf("READ COMMITTED") < source.indexOf(".begin_private_work"));
  assert.ok(source.indexOf(".verify_pending_in_transaction") < source.indexOf("context.finish"));
  assert.ok(source.indexOf("context.finish") < source.indexOf("tx.commit"));
  assert.ok(source.indexOf("tx.commit") < source.indexOf("pending.into_record"));
  assert.doesNotMatch(source, /PermissionDenied|READ WRITE|install_empty|\.issue\(|\.revoke\(|\.get\(|\.matches\(|FileUpload|FilePublisherGrant|require_policy_manager/);
  const borrowed = code(await read("engine/src/services/file_recipient_grant_store/transaction.rs"));
  for (const phrase of ["schema: SchemaPin", "pending.schema.verify", "current != pending.expected", "IN SHARE MODE", "FOR SHARE"]) assert.ok(borrowed.includes(phrase), phrase);
  assert.ok(borrowed.indexOf("IN SHARE MODE") < borrowed.indexOf("FOR SHARE"));
  assert.doesNotMatch(borrowed, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS|derive\(Clone|Serialize|Deserialize/);
});

test("recipient runners exactly select every wired ignored database case", async () => {
  for (const [target, directory, wiringPath, runnerPath, count, prefix] of [
    ["file_recipient_grant_tdd", "file_recipient_grant", "engine/tests/file_recipient_grant/mod.rs", "scripts/test-file-recipient-grants.sh", 12, "cases::"],
    ["session_file_recipient_grant_tdd", "session_file_recipient_grant", "engine/tests/session_file_recipient_grant_tdd.rs", "scripts/test-session-file-recipient-grants.sh", 7, ""],
  ]) {
    const ignored = names(await read(`engine/tests/${target}.rs`)).map(name => `baseline::${name}`);
    const wiring = await read(wiringPath);
    for (const file of await readdir(new URL(`engine/tests/${directory}/`, root))) {
      if (!file.endsWith(".rs")) continue;
      const module = file.replace(/\.rs$/, "");
      const cases = names(await read(`engine/tests/${directory}/${file}`));
      if (cases.length) assert.match(wiring, new RegExp(`\\bmod ${module};`));
      ignored.push(...cases.map(name => `${prefix}${module}::${name}`));
    }
    const runner = await read(runnerPath);
    const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
    assert.equal(ignored.length, count, target);
    assert.equal(new Set(inventory).size, inventory.length, target);
    assert.deepEqual(inventory.sort(), ignored.sort(), target);
    for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern, target);
    assert.ok(runner.includes(`--test ${target}`));
    assert.ok((await read(".github/workflows/ci.yml")).includes(`bash ${runnerPath}`));
  }
});

test("bilingual recipient docs retain independent metadata and unreleased scope", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-recipient-grants.md`);
    for (const phrase of ["FileRecipientGrantStore", "SessionFileRecipientGrantWorkspace", "SessionFileRecipientGrantError", "FileVersionRef", "grant_payload", "recorded_at", "READ COMMITTED", "READ ONLY", "FOR SHARE", "0.5.6", "0.3.0", "test-file-recipient-grants.sh", "test-session-file-recipient-grants.sh"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-access-contract", "large-file-publisher-grants", "large-file-session-publication"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-recipient-grants.md"), `${lang}: ${file}`);
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileRecipientGrant"));
  }
});
