import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_transfer/staging/";
const before = (source, left, right) => {
  const first = source.indexOf(left), second = source.indexOf(right);
  assert.ok(first >= 0 && second >= 0 && first < second, `ordered actual ${left} before ${right}`);
};

// Composition/inventory only. Actual native files/jobs/Source SQL establish behavior.
test("private saving freezes explicit host selection and opaque output without changing old errors", async () => {
  const s = code(await read(`${dir}save.rs`));
  for (const phrase of ["FileSaveTarget", "Arc<Target>", "pub fn open(parent: &Path, leaf: &str)", "leaf.is_empty()", "leaf.len() > 255", "character.is_control()", "SaveDirectory::open_private(parent)", "SavedFile", "FileSaveError", "manifest_digest(manifest)", "assembled.proof().matches", "work_save::run"]) assert.ok(s.includes(phrase), phrase);
  const directory = code(await read(`${dir}save_directory.rs`));
  assert.match(directory, /fn open_private\(path: &Path\)[\s\S]*?ChunkDirectory::open\(path\)\.map\(Self::Private\)/);
  assert.doesNotMatch(s, /manifest\.display_name|Serialize|Deserialize|pub fn (?:new|from|path|bytes|descriptor)|std::env|reqwest|sqlx|remove_|rename\(|\.truncate/);
  const acknowledgement = s.slice(s.indexOf("pub struct SavedFile"), s.indexOf("pub(super) async fn run"));
  assert.doesNotMatch(acknowledgement, /pub\s+file:|derive\([^)]*(?:Clone|Debug|Serialize|Deserialize)/);
  assert.equal((await read(`${dir}save.rs`)).match(/```compile_fail/g)?.length, 3);
  const mod = code(await read(`${dir}mod.rs`));
  assert.match(mod, /pub async fn save_assembled_file\(\s*&self,\s*manifest: &FileTransferManifest,\s*assembled: &AssembledFile,\s*target: &FileSaveTarget,?\s*\)/);
  assert.ok(mod.includes("pub use save::") && mod.includes("save::run(&self.inner"));
  const old = mod.slice(mod.indexOf("pub enum ChunkStagingError"), mod.indexOf("pub struct ChunkStaging"));
  assert.doesNotMatch(old, /Save|Target|SourceChanged/);
});

test("assembly proof pins original read only source and root and never shares seek cursors", async () => {
  const assembly = code(await read(`${dir}assemble.rs`));
  assert.match(assembly, /proof: Arc<AssembledProof>/);
  before(assembly, "require_output(directory", "AssembledProof::capture");
  const completion = assembly.slice(assembly.indexOf("fn finish_created"));
  before(completion, "let mut retained = open_at", "drop(writer)");
  before(completion, "drop(writer)", "verify_file(manifest");
  const proof = code(await read(`${dir}assembled_proof.rs`)).split("#[cfg(test)]")[0];
  for (const phrase of ["file: File", "snapshot: Snapshot", "binding: [u8; 32]", "root: Identity", "directory: File", "directory.directory.try_clone()", "self.root == root && self.binding == binding", "Snapshot::private_file", "assembled_name(self.binding)", "read_at(buffer, self.position)", "checked_add"]) assert.ok(proof.includes(phrase), phrase);
  assert.doesNotMatch(proof, /\.seek\(|\.rewind\(|Vec<u8>|pub fn (?:new|from|path|bytes|descriptor)|reqwest|sqlx|std::env/);
});

test("saving keeps real worker proof ownership and original exclusive copy and readback sequence", async () => {
  const allWork = code(await read(`${dir}work_save.rs`));
  const workStart = allWork.indexOf("pub(super) async fn run(");
  const workEnd = allWork.indexOf("\n#[cfg(test)]\npub(super) async fn run_with");
  assert.ok(workStart >= 0 && workEnd > workStart, "original production job boundary exists");
  const work = allWork.slice(workStart, workEnd);
  let previous = -1;
  for (const phrase of ["work::admit(inner)", "manifest.clone()", "Arc::clone(proof)", "Arc::clone(target)", "spawn_blocking(move ||", "let _permit = permit", "save_io::run"]) {
    const current = work.indexOf(phrase); assert.ok(current > previous, `ordered ${phrase}`); previous = current;
  }
  assert.doesNotMatch(work, /acquire_owned\(\)\.await|tokio::spawn|timeout|retry|Vec<u8>/);
  const io = code(await read(`${dir}save_io.rs`));
  for (const phrase of ["proof.verify", "entry_exists", "verify_stream", "O_EXCL", "stream::copy_verify", "original == proof.identity()", "from_mode(0o400)", "sync_all()", "verify_file", "require_output", "FileSaveError::Unconfirmed"]) assert.ok(io.includes(phrase), phrase);
  const creation = io.slice(io.indexOf("fn run_inner"), io.indexOf("fn finish_created"));
  before(creation, "verify_stream(manifest, &mut proof.reader())", "let writer = open_at");
  const finish = io.slice(io.indexOf("fn finish_created"));
  before(finish, "let mut retained = open_at", "drop(writer)");
  before(finish, "drop(writer)", "verify_file(manifest");
  assert.doesNotMatch(io, /Vec<u8>|read_to_end|read_to_string|remove_|rename\(|\.truncate|reqwest|sqlx|tokio::spawn/);
});

test("private save target wires actual baseline and native cases without opt in placeholders", async () => {
  const main = await read("engine/tests/file_save_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  let defaults = 0;
  for (const file of await readdir(new URL("engine/tests/file_save/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_save/${file}`);
    const count = (s.match(/#\[(?:tokio::)?test\]/g) || []).length;
    defaults += count;
    assert.doesNotMatch(s, /#\[ignore/);
    if (count) assert.match(main, new RegExp(`\\bmod ${file.replace(/\.rs$/, "")};`));
  }
  assert.equal(defaults, 12, "exact real native cases, not helper inventory");
  const baseline = await read("engine/tests/file_save/baseline.rs");
  for (const phrase of ["save_assembled_file", "observe", "source_root_after", "target_root_after", "CHOSEN_LEAF", "FILE_TRANSFER_CHUNK_BYTES + 17"]) assert.ok(baseline.includes(phrase), phrase);
  before(baseline, "checked(", "result.expect(");
  const source = await read("engine/tests/file_manifest_client/source.rs");
  for (const phrase of ["FileSaveTarget::open", "staging.save_assembled_file", "assert_saved_copy", "source_root_before", "local_before", "selected_parent"]) assert.ok(source.includes(phrase), phrase);
});

test("selected saving stays unmounted and bilingual notes distinguish host capability and UI policy", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /FileSaveTarget|save_assembled_file|SavedFile|FileSaveError/, path);
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-save.md`);
    for (const phrase of ["0.5.7", "0.3.0", "FileSaveTarget", "save_assembled_file", "AssembledFile", "SavedFile", "FileSaveError", "FileVersionRef", "O_EXCL", "O_NOFOLLOW", "0700", "0400", "64 KiB", "64 GiB", "Unconfirmed", "file_save_tdd", "fileSave", "106", "42"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-chunk-assembly", "large-file-download-client", "large-file-manifest-client"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-save.md`);
  }
});
