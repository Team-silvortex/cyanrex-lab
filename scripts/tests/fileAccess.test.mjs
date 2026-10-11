import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Composition guards only; default Rust tests establish actual decoding and exact matching.
test("file grant contracts stay in the shared model without teaching or storage dependencies", async () => {
  const prefix = "engine/src/models/collaboration/";
  const entries = await readdir(new URL(prefix, root));
  const files = entries.filter(name => /^file_access.*\.rs$/.test(name));
  if (entries.includes("file_access")) {
    for (const name of await readdir(new URL(`${prefix}file_access/`, root))) {
      if (name.endsWith(".rs")) files.push(`file_access/${name}`);
    }
  }
  assert.ok(files.length > 0);
  const source = codeOnly((await Promise.all(files.map(name => read(`${prefix}${name}`)))).join("\n"));
  for (const name of ["FileAccountRef", "FileVersionRef", "FileGrantValidity", "FileGrantState", "FilePublisherGrant", "FileRecipientGrant", "MAX_FILE_GRANT_JSON_BYTES", "MAX_FILE_GRANT_UNIX_SECONDS"]) {
    assert.ok(source.includes(name), name);
  }
  assert.doesNotMatch(source, /LegacyAccessPolicy|InstanceDeploymentGrant|has_role|teaching\.teacher|workspace\.owner|sqlx|PgPool|Transaction|DurableAuthSource|std::(?:fs|net|env)|tokio|reqwest/);
  assert.match(await read(`${prefix}mod.rs`), /pub use file_access::\*/);
  assert.match(await read(`${prefix}scalars.rs`), /\bFileGrantId\b/);
});

test("file grant values do not become live or Session permissions implicitly", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json", "engine/src/services/auth_service/durable_source/file_commands/mod.rs", "engine/src/services/collaboration_identity_store/access.rs"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /FilePublisherGrant|FileRecipientGrant|FileGrantValidity/, path);
  }
  const target = await read("engine/tests/file_access_tdd.rs");
  assert.doesNotMatch(target, /#\[ignore|CYANREX_TEST_DATABASE_URL/);
});

test("bilingual file grant documentation separates selectors from authenticated authority", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-access-contract.md`);
    for (const phrase of ["0.5.5", "FileAccountRef", "FileVersionRef", "FilePublisherGrant", "FileRecipientGrant", "LegacyAccountId", "4096", "253402300799", "[not_before, expires_at)", "matches", "file_access_tdd"]) {
      assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    }
    for (const name of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution"]) {
      assert.ok((await read(`docs/${lang}/${name}.md`)).includes("large-file-access-contract.md"), `${lang}: ${name}`);
    }
    assert.ok((await read(`docs/${lang}/testing-guide.md`)).includes("fileAccess"));
  }
});
