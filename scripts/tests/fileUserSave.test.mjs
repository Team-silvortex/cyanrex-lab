import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_transfer/staging/";
const ordered = (s, names) => {
  let last = -1;
  for (const name of names) {
    const at = s.indexOf(name);
    assert.ok(at >= 0 && at > last, `present and ordered ${name}`);
    last = at;
  }
};

// Composition/inventory only. Real native files/jobs/network/SQL are separate acceptance.
test("user save target is explicit and leaves the original private target policy separate", async () => {
  const s = code(await read(`${dir}save.rs`));
  assert.match(s, /pub fn open_user_directory\(parent: &Path, leaf: &str\)/);
  const old = s.slice(s.indexOf("pub fn open("), s.indexOf("pub fn open_user_directory("));
  const user = s.slice(s.indexOf("pub fn open_user_directory("), s.indexOf("pub struct SavedFile"));
  ordered(old, ["validate_leaf(leaf)", "SaveDirectory::open_private(parent)"]);
  ordered(user, ["validate_leaf(leaf)", "SaveDirectory::open_user(parent)"]);
  assert.doesNotMatch(old, /open_user|or_else|set_permissions|create_dir/);
  assert.doesNotMatch(s, /manifest\.display_name|Serialize|Deserialize|pub fn (?:path|bytes|descriptor)|std::env|reqwest|sqlx/);
  assert.equal((await read(`${dir}save.rs`)).match(/```compile_fail/g)?.length, 3);
});

test("protected traversal is shared but strict private 0700 terminal is not widened", async () => {
  const s = code(await read(`${dir}filesystem.rs`));
  const begin = s.indexOf("pub(super) fn open_root("), end = s.indexOf("pub(super) fn open_directory_path(");
  assert.ok(begin >= 0 && end > begin);
  const terminal = s.slice(begin, end);
  ordered(terminal, ["open_directory_path(path)", "directory.metadata()", "meta.mode() & 0o7777 != 0o700"]);
  assert.ok(terminal.includes("meta.uid() != uid()"));
  assert.doesNotMatch(terminal, /0o750|0o755|open_user/);
  const walk = s.slice(end, s.indexOf("pub(super) struct Identity"));
  for (const phrase of ["path.is_absolute()", "len() > 4096", "Component::Normal", "[0, uid()]", "0o022", "meta.uid() == 0", "0o1000", "parts == 0", "open_at", "O_DIRECTORY"]) assert.ok(walk.includes(phrase), phrase);
  assert.ok(s.includes("O_NOFOLLOW") && s.includes("O_CLOEXEC"));
  assert.ok(code(await read(`${dir}directory.rs`)).includes("let directory = open_root(path)"));
});

test("user directory pins original fd and exact UID GID mode without freezing legitimate entry timestamps", async () => {
  const s = code(await read(`${dir}save_directory.rs`)).split("\n#[cfg(test)]")[0];
  for (const phrase of ["Private(ChunkDirectory)", "ChunkDirectory::open(path).map(Self::Private)", "open_directory_path(path)", "SecuritySnapshot::of", "uid: u32", "gid: u32", "mode: u32", "metadata.gid()", "mode() & 0o7777", "0o700 | 0o750 | 0o755", "before != self.original", "visible != self.original", "after != self.original"]) assert.ok(s.includes(phrase), phrase);
  const start = s.indexOf("impl UserSaveDirectory"), end = s.indexOf("struct SecuritySnapshot");
  assert.ok(start >= 0 && end > start);
  ordered(s.slice(start, end), ["let before", "open_directory_path(&self.path)", "let visible", "let after", "before != self.original"]);
  assert.doesNotMatch(s, /mtime|ctime|metadata\.len\(|metadata\.nlink\(|set_permissions|create_dir|remove_|rename\(|Serialize|Deserialize/);
  const io = code(await read(`${dir}save_io.rs`));
  assert.ok(io.includes("target.directory.fd()"));
  for (const phrase of ["O_EXCL", "0o600", "from_mode(0o400)", "verify_stream", "stream::copy_verify", "verify_file", "FileSaveError::Unconfirmed"]) assert.ok(io.includes(phrase), phrase);
});

test("user directory defaults are real and preserve explicit authorized Source inventory", async () => {
  const main = await read("engine/tests/file_user_save_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  let count = 0;
  for (const file of await readdir(new URL("engine/tests/file_user_save/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_user_save/${file}`);
    const tests = (s.match(/#\[(?:tokio::)?test\]/g) || []).length;
    count += tests;
    assert.doesNotMatch(s, /#\[ignore/);
    if (tests) assert.match(main, new RegExp(`\\bmod ${file.replace(/\.rs$/, "")};`));
  }
  assert.equal(count, 10);
  const baseline = await read("engine/tests/file_user_save/baseline.rs");
  ordered(baseline, ["checked(", "constructor.expect("]);
  for (const phrase of ["open_user_directory", "source_root_after", "selected_before", "selected_entries", "verify_chunk", "observe", "FILE_TRANSFER_CHUNK_BYTES + 17"]) assert.ok(baseline.includes(phrase), phrase);
  const source = await read("engine/tests/file_delivery/source.rs");
  for (const phrase of ["open_user_directory", "0o700, 0o755, 0o750", "value.gid()", "assert_private_whole", "assert_saved_copy", "fetch_granted_and_save", "f.sharing.snapshot()", "recipient_delta"]) assert.ok(source.includes(phrase), phrase);
  assert.equal((source.match(/#\[ignore/g) || []).length, 2);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-delivery.sh"));
});

test("user directory choice remains trusted local host only and bilingual privacy boundaries are current", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /open_user_directory|UserSaveDirectory|SaveDirectory/, path);
  for (const lang of ["en", "zh-CN"]) {
    const s = await read(`docs/${lang}/large-file-user-save.md`);
    for (const phrase of ["0.5.7", "0.3.0", "2026-10-13", "open_user_directory", "0700", "0750", "0755", "UID", "GID", "mtime", "ctime", "0400", "O_EXCL", "Unconfirmed", "file_user_save_tdd", "fileUserSave", "106", "46"]) assert.ok(s.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-download-client", "large-file-manifest-client", "large-file-save", "large-file-delivery-client"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-user-save.md"), `${lang}: ${file}`);
  }
});
