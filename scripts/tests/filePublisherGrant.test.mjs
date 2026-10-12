import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const ignoredNames = source => [...source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);

// Inventory/composition checks only. Disposable SQL establishes actual locking and authorization.
test("publisher ledger is explicit trusted metadata and is not mounted or consumed as permission", async () => {
  assert.match(await read("engine/src/services/mod.rs"), /pub mod file_publisher_grant_store;/);
  const source = codeOnly(await read("engine/src/services/file_publisher_grant_store/commands.rs"));
  for (const phrase of ["pub async fn issue", "pub async fn revoke", "pub async fn get", "issue_in_transaction", "revoke_in_transaction", "SET CONSTRAINTS ALL IMMEDIATE", "verify_mutation_in_transaction", "tx.commit"]) {
    assert.ok(source.includes(phrase), phrase);
  }
  const mutation = codeOnly(await read("engine/src/services/file_publisher_grant_store/mutation.rs"));
  for (const phrase of ["self.ledger", "schema.verify", "pending.ledger", "pending.selected", "confirmed_one", "insert_change"]) {
    assert.ok(mutation.includes(phrase), phrase);
  }
  assert.doesNotMatch(mutation, /Registry|Runtime|std::(?:fs|net|env)|tokio::fs|reqwest|\.publish\(|\.matches\(/);
  assert.doesNotMatch(source, /Registry|Runtime|std::(?:fs|net|env)|tokio::fs|reqwest|\.publish\(|\.matches\(/);
  const ddl = await read("engine/migrations/0021_collaboration_file_publisher_grants.sql");
  for (const phrase of ["collaboration_file_publisher_grant_schema", "collaboration_file_publisher_grants", "collaboration_file_publisher_grant_changes", "grant_payload BYTEA", "recorded_at BIGINT"]) {
    assert.ok(ddl.includes(phrase), phrase);
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /SessionFilePublisherGrant|FilePublisherGrantStore|get_session_file_publisher_grant/, path);
  }
});

test("Session derives only its own grant subject and owns the final authorization and commit", async () => {
  const source = codeOnly(await read("engine/src/services/auth_service/durable_source/file_publisher_commands.rs"));
  for (const pattern of [/READ COMMITTED/, /begin_private_work/, /FileAccountRef::new\(context\.owner, context\.account_id\(\)\)/, /get_in_transaction/, /verify_pending_in_transaction/, /context\.finish/, /tx\.commit/, /pending\.into_record/]) {
    assert.match(source, pattern);
  }
  assert.doesNotMatch(source, /READ WRITE|install_empty|\.issue\(|\.revoke\(|\.get\(|\.matches\(|\.publish\(|require_policy_manager|session_account_manager|read_session_artifact|FileUploadPublisher/);
  const start = source.indexOf("SET TRANSACTION ISOLATION LEVEL READ COMMITTED");
  assert.ok(start < source.indexOf(".begin_private_work"));
  assert.ok(source.indexOf(".verify_pending_in_transaction") < source.indexOf("context.finish"));
  assert.ok(source.indexOf("context.finish") < source.indexOf("tx.commit"));
  assert.ok(source.indexOf("tx.commit") < source.indexOf("pending.into_record"));
});

test("borrowed grant snapshots keep original pins and fence history appends before row locks", async () => {
  const source = codeOnly(await read("engine/src/services/file_publisher_grant_store/transaction.rs"));
  for (const pattern of [/schema: SchemaPin/, /get_in_transaction/, /verify_pending_in_transaction/, /pending\.schema\.verify/, /current != pending\.expected/, /IN SHARE MODE/, /FOR SHARE/, /into_record/]) {
    assert.match(source, pattern);
  }
  assert.ok(source.indexOf("IN SHARE MODE") < source.indexOf("FOR SHARE"));
  assert.doesNotMatch(source, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS|derive\(Clone|Serialize|Deserialize/);
  const schema = codeOnly(await read("engine/src/services/file_publisher_grant_store/schema.rs"));
  assert.match(schema, /SHARE ROW EXCLUSIVE MODE/);
  assert.doesNotMatch(schema, /READ WRITE/);
});

test("publisher runners select every wired ignored database case exactly", async () => {
  for (const [target, directory, wiringPath, runnerPath, count, prefix] of [
    ["file_publisher_grant_tdd", "file_publisher_grant", "engine/tests/file_publisher_grant/mod.rs", "scripts/test-file-publisher-grants.sh", 12, "cases::"],
    ["session_file_publisher_grant_tdd", "session_file_publisher_grant", "engine/tests/session_file_publisher_grant_tdd.rs", "scripts/test-session-file-publisher-grants.sh", 7, ""],
  ]) {
    const main = await read(`engine/tests/${target}.rs`);
    const wiring = await read(wiringPath);
    const ignored = ignoredNames(main).map(name => `baseline::${name}`);
    for (const entry of await readdir(new URL(`engine/tests/${directory}/`, root))) {
      if (!entry.endsWith(".rs")) continue;
      const module = entry.replace(/\.rs$/, "");
      const names = ignoredNames(await read(`engine/tests/${directory}/${entry}`));
      if (names.length) assert.match(wiring, new RegExp(`\\bmod ${module};`));
      ignored.push(...names.map(name => `${prefix}${module}::${name}`));
    }
    const runner = await read(runnerPath);
    const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
    assert.equal(ignored.length, count, target);
    assert.equal(new Set(inventory).size, inventory.length, target);
    assert.deepEqual(inventory.sort(), ignored.sort(), target);
    for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /\b(?:rg|grep) -Fx/, /--ignored --exact --nocapture/]) {
      assert.match(runner, pattern, target);
    }
    assert.ok(runner.includes(`--test ${target}`));
    assert.ok((await read(".github/workflows/ci.yml")).includes(`bash ${runnerPath}`));
  }
});

test("bilingual publisher docs preserve the metadata-only and unreleased boundaries", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-publisher-grants.md`);
    for (const phrase of ["FilePublisherGrantStore", "SessionFilePublisherGrantWorkspace", "ManageFileGrants", "grant_payload", "recorded_at", "READ COMMITTED", "READ ONLY", "FOR SHARE", "0.5.6", "test-file-publisher-grants.sh", "test-session-file-publisher-grants.sh"]) {
      assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    }
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide"]) {
      assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-publisher-grants.md"), `${lang}: ${file}`);
    }
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("filePublisherGrant"));
  }
});
