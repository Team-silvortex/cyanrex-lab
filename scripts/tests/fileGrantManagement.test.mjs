import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Composition checks only; explicit disposable SQL proves actual transaction behavior.
test("file-grant management derives exact actors without inheriting role or deployment authority", async () => {
  const source = code(await read("engine/src/services/auth_service/durable_source/file_grant_management.rs"));
  for (const phrase of ["SessionFileGrantManagementWorkspace", "file_grant_management_workspace", "issue_session_file_publisher_grant", "revoke_session_file_publisher_grant", "FileAccountRef::new(context.owner, context.account_id())", "actor != workspace.manager", "PermissionDenied", "READ COMMITTED", "synchronous_commit = on", "SET CONSTRAINTS ALL IMMEDIATE"]) {
    assert.ok(source.includes(phrase), phrase);
  }
  assert.doesNotMatch(source, /Deserialize|Serialize|Default|require_policy_manager|session_account_manager|deployment_granted|role_refs|READ WRITE|install_empty|\.issue\(|\.revoke\(|\.matches\(|\.publish\(/);
  const issue = source.slice(source.indexOf("pub async fn issue_session"), source.indexOf("pub async fn revoke_session"));
  const revoke = source.slice(source.indexOf("pub async fn revoke_session"), source.indexOf("fn check_file_manager_scope"));
  assert.equal((issue.match(/require_private_subject/g) || []).length, 2);
  assert.doesNotMatch(revoke, /require_private_subject/);
  for (const command of [issue, revoke]) {
    assert.ok(command.indexOf(".begin_private_work") < command.indexOf("actor != workspace.manager"));
    assert.ok(command.indexOf("actor != workspace.manager") < command.indexOf("_in_transaction"));
    assert.ok(command.indexOf("SET CONSTRAINTS ALL IMMEDIATE") < command.indexOf("verify_mutation_in_transaction"));
    assert.ok(command.indexOf("verify_mutation_in_transaction") < command.indexOf("context.finish"));
    assert.ok(command.indexOf("context.finish") < command.indexOf("tx.commit"));
    assert.ok(command.indexOf("tx.commit") < command.indexOf("pending.into_record"));
    assert.doesNotMatch(command.slice(command.indexOf("context.finish")), /sqlx::query|\.store|verify_mutation/);
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(code(await read(path)), /SessionFileGrantManagement|issue_session_file_publisher|revoke_session_file_publisher/, path);
  }
});

test("borrowed mutations and exact-subject guards preserve first pins without independent authority", async () => {
  const mutation = code(await read("engine/src/services/file_publisher_grant_store/mutation.rs"));
  for (const phrase of ["schema: SchemaPin", "selected: Selected", "ledger: Ledger", "pending.schema.verify", "current != pending.selected", "pending.fingerprint", "self.ledger", "pending.ledger", "into_record"]) {
    assert.ok(mutation.includes(phrase), phrase);
  }
  assert.doesNotMatch(mutation, /\.begin\(|\.acquire\(|\.commit\(|SET LOCAL|SET TRANSACTION|SET CONSTRAINTS|derive\(Clone|Serialize|Deserialize|Registry|PgPool/);
  const subject = code(await read("engine/src/services/auth_service/durable_source/private_work/subjects.rs"));
  for (const phrase of ["principal: PrincipalRef", "account_id: LegacyAccountId", "self.source.verify", "self.relations.verify", "self.targets[self.selected].verify", "session_policy_target", "command_target", "session_private_work_actor", "identity.account_id != account_id", "current != principal"]) {
    assert.ok(subject.includes(phrase), phrase);
  }
  assert.doesNotMatch(subject, /FileAccountRef|FileGrant|require_policy_manager|session_account_manager|\.pool|\.begin\(|\.commit\(/);
});

test("bilingual Session grant-management docs state config revocation and unconnected consumption", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-grant-management.md`);
    for (const phrase of ["SessionFileGrantManagementWorkspace", "ManageFileGrants", "FileAccountRef", "READ COMMITTED", "READ ONLY", "SET CONSTRAINTS ALL IMMEDIATE", "0.5.6", "0.3.0", "test-session-file-grant-management.sh"]) {
      assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    }
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-publisher-grants", "large-file-distribution", "large-file-access-contract"]) {
      await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-grant-management.md`);
    }
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileGrantManagement"));
  }
  assert.match(await read("docs/en/large-file-grant-management.md"), /does not revoke surviving[\s\S]*clones or in-flight/);
  assert.match(await read("docs/zh-CN/large-file-grant-management.md"), /不会撤销存活 clone 或在途命令/);
});

test("Session grant-management runner selects the entire wired ignored inventory exactly", async () => {
  const names = source => [...source.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (postgres_\w+)\(/g)].map(m => m[1]);
  const main = await read("engine/tests/session_file_grant_management_tdd.rs");
  const ignored = names(main).map(name => `baseline::${name}`);
  for (const file of await readdir(new URL("engine/tests/session_file_grant_management/", root))) {
    if (!file.endsWith(".rs")) continue;
    const module = file.replace(/\.rs$/, "");
    const cases = names(await read(`engine/tests/session_file_grant_management/${file}`));
    if (cases.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...cases.map(name => `${module}::${name}`));
  }
  const runner = await read("scripts/test-session-file-grant-management.sh");
  const inventory = [...runner.matchAll(/\b[\w:]+::postgres_\w+/g)].map(m => m[0]);
  assert.equal(ignored.length, 10);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/, /--test session_file_grant_management_tdd/]) {
    assert.match(runner, pattern);
  }
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-session-file-grant-management.sh"));
});
