import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/services/file_transfer/staging/";

// Composition and inventory only. Actual native Rust file/job cases prove assembly behavior.
test("private assembly accepts only the fixed staging manifest and returns opaque metadata", async () => {
  const mod = code(await read(`${dir}mod.rs`));
  assert.match(mod, /pub async fn assemble_staged_file\(\s*&self,\s*manifest: &FileTransferManifest,?\s*\)/);
  assert.match(mod, /pub use assemble::AssembledFile;/);
  const s = code(await read(`${dir}assemble.rs`));
  assert.match(s, /pub struct AssembledFile\s*\{\s*file: FileVersionRef,?\s*\}/);
  assert.match(s, /pub fn file\(&self\) -> &FileVersionRef/);
  assert.doesNotMatch(s, /pub fn (?:new|from|path|bytes|descriptor)|pub\s+file:|derive\([^)]*(?:Clone|Serialize|Deserialize)[^)]*\)\]\s*pub struct AssembledFile/);
  assert.equal((await read(`${dir}assemble.rs`)).match(/```compile_fail/g)?.length, 3);
});

test("private assembly remains inside descriptor-relative staging and outside live authority composition", async () => {
  const files = await readdir(new URL(dir, root));
  const selected = files.filter(name => /^assemble(?:_|\.)/.test(name) && name.endsWith(".rs") && !name.includes("test"));
  assert.ok(selected.length >= 1);
  const s = (await Promise.all(selected.map(name => read(`${dir}${name}`)))).map(code).join("\n");
  assert.doesNotMatch(s, /sqlx|reqwest|SessionBind|AppState|FileChunkClient|std::env|tokio::fs|remove_file|remove_dir|rename\(|\.truncate\(true\)|read_to_end|read_to_string/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /AssembledFile|assemble_staged_file|file_chunk_assembly/, path);
  assert.match(await read("engine/src/services/file_transfer/mod.rs"), /#\[cfg\(unix\)\]\s*pub mod staging;/);
});

test("assembly admission and original-byte checks stay inside the actual bounded job", async () => {
  const s = code(await read(`${dir}work_assemble.rs`));
  let previous = -1;
  for (const phrase of ["work::admit(inner)?", "manifest.clone()", "Arc::clone(inner)", "spawn_blocking(move ||", "let _permit = permit", "assemble::run"]) {
    const current = s.indexOf(phrase);
    assert.ok(current > previous, `ordered ${phrase}`);
    previous = current;
  }
  assert.doesNotMatch(s, /acquire_owned\(\)\.await|tokio::spawn|OnceLock|Vec<u8>/);
  const fs = code(await read(`${dir}filesystem.rs`));
  for (const phrase of ["AT_SYMLINK_NOFOLLOW", "pub(super) fn private_file", "MAX_FILE_TRANSFER_BYTES", "FILE_TRANSFER_CHUNK_BYTES"]) assert.ok(fs.includes(phrase), phrase);
  const assemble = code(await read(`${dir}assemble.rs`));
  for (const phrase of ["entry_exists", "observe_legacy", "O_EXCL", "copy_legacy", "from_mode(0o400)", "sync_all()", "verify_file", "recheck_legacy", "require_output"]) assert.ok(assemble.includes(phrase), phrase);
  assert.doesNotMatch(assemble, /Vec<u8>|read_to_end|read_to_string|rename\(|remove_file/);
});

test("assembly native inventory has a real cross-chunk baseline without ignored placeholders", async () => {
  const main = await read("engine/tests/file_chunk_assembly_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  assert.match(main, /mod baseline;/);
  const s = await read("engine/tests/file_chunk_assembly/baseline.rs");
  for (const phrase of ["FILE_TRANSFER_CHUNK_BYTES + 17", "assemble_staged_file", "observe_output", "f.cleanup()", "source_after", "root_after", "before.len(), 2"]) assert.ok(s.includes(phrase), phrase);
  assert.ok(s.indexOf("f.cleanup()") < s.indexOf("result.expect("));
  for (const name of await readdir(new URL("engine/tests/file_chunk_assembly/", root))) {
    if (name.endsWith(".rs")) assert.doesNotMatch(await read(`engine/tests/file_chunk_assembly/${name}`), /#\[ignore/);
  }
});

test("assembly bilingual current boundaries are linked without changing release or live contracts", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const doc = await read(`docs/${lang}/large-file-chunk-assembly.md`);
    for (const phrase of ["0.5.6", "0.3.0", "assemble_staged_file", "AssembledFile", "FileVersionRef", "f1-", "64 KiB", "64 GiB", "Unconfirmed", "file_chunk_assembly_tdd", "fileChunkAssembly"]) assert.ok(doc.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "large-file-distribution", "large-file-staging", "large-file-chunk-client", "capability-maturity"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-chunk-assembly.md"), `${lang}: ${file}`);
  }
  assert.ok((await read("README.md")).includes("large-file-chunk-assembly.md"));
});
