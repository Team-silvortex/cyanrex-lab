import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function sources(directory) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sources(path));
    else if (entry.name.endsWith(".rs")) result.push([path, await read(path)]);
  }
  return result;
}

// Inventory and composition guards only: default Rust cases establish byte/contract behavior.
test("large-file core is independently exported without widening existing Artifact or text contracts", async () => {
  const model = codeOnly(await read("engine/src/models/collaboration/mod.rs"));
  assert.match(model, /mod file_transfer;/);
  assert.match(model, /pub use file_transfer::\*;/);
  const ids = codeOnly(await read("engine/src/models/collaboration/scalars.rs"));
  assert.match(ids, /\bFileId,/);
  assert.match(ids, /\bFileVersionId,/);
  assert.match(await read("engine/src/services/mod.rs"), /pub mod file_transfer;/);
  assert.match(await read("engine/src/services/artifact_store/mod.rs"), /MAX_CONTENT: usize = 1_048_576/);
  assert.match(await read("engine/src/models/collaboration/content.rs"), /MAX_TEXT_PAYLOAD_BYTES: usize = 256 \* 1024/);
  assert.doesNotMatch(codeOnly(await read("engine/src/models/collaboration/content.rs")), /FileTransferManifest|FileVersionId/);
});

test("large-file streaming preparation has no persistent publication authority or live composition", async () => {
  for (const [path, source] of await sources("engine/src/services/file_transfer")) {
    if (["engine/src/services/file_transfer/staging/queued_tests.rs", "engine/src/services/file_transfer/staging/whole_tests.rs", "engine/src/services/file_transfer/staging/read_tests.rs", "engine/src/services/file_transfer/staging/assembly_tests.rs", "engine/src/services/file_transfer/staging/assembly_test_support.rs", "engine/src/services/file_transfer/staging/assembly_flow_tests.rs", "engine/src/services/file_transfer/staging/assembled_proof_tests.rs", "engine/src/services/file_transfer/staging/save_test_support.rs", "engine/src/services/file_transfer/staging/save_work_tests.rs", "engine/src/services/file_transfer/staging/save_drift_tests.rs", "engine/src/services/file_transfer/staging/save_user_tests.rs", "engine/src/services/file_transfer/staging/save_directory_tests.rs"].includes(path)) continue;
    const production = codeOnly(source.replace(/#\[cfg\(test\)\]\s*mod queued_tests;/g, ""))
      .split("#[cfg(test)]")[0];
    assert.doesNotMatch(production, /sqlx|PgPool|AuthService|ArtifactStore|TaskStore|reqwest|std::(?:net|process|env)|install_empty|\.transaction\(/, path);
    if (!["engine/src/services/file_transfer/staging/work.rs", "engine/src/services/file_transfer/staging/work_bound.rs", "engine/src/services/file_transfer/staging/work_whole.rs", "engine/src/services/file_transfer/staging/work_read.rs", "engine/src/services/file_transfer/staging/work_assemble.rs", "engine/src/services/file_transfer/staging/work_save.rs"].includes(path)) {
      assert.doesNotMatch(production, /spawn_blocking/, path);
    }
    if (path.includes("/staging/")) {
      assert.doesNotMatch(production, /remove_file|remove_dir|read_to_end|serde_json::to_vec/, path);
      if (path === "engine/src/services/file_transfer/staging/read.rs") {
        // Only the move-only checked reader may buffer bytes, privately and under a retained
        // response lease. Stream/stage/receipt/whole-observation paths still cannot buffer them.
        assert.match(production, /bytes: Vec<u8>/);
        assert.match(production, /_lease: OwnedSemaphorePermit/);
        assert.match(production, /fn bytes\(&self\) -> &\[u8\]/);
        assert.ok(production.indexOf("Snapshot::private") < production.indexOf("bytes.try_reserve_exact(length)"));
        assert.ok(production.indexOf("bytes.try_reserve_exact(length)") < production.indexOf("copy_chunk_checked"));
        assert.doesNotMatch(production, /pub fn|into_vec|derive\([^)]*Clone|Serialize|Deserialize/);
      } else {
        assert.doesNotMatch(production, /Vec<u8>/, path);
      }
    }
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) {
    assert.doesNotMatch(codeOnly(await read(path)), /FileTransferManifest|copy_chunk_checked|inspect_stream|services::file_transfer/, path);
  }
  assert.doesNotMatch(await read("engine/Cargo.toml"), /autotests\s*=\s*false/);
});

test("private chunk staging remains a separate default-tested Unix preparation boundary", async () => {
  const service = await read("engine/src/services/file_transfer/mod.rs");
  assert.match(service, /#\[cfg\(unix\)\]\s*pub mod staging;/);
  const stagingService = await read("engine/src/services/file_transfer/staging/mod.rs");
  assert.match(stagingService, /#\[cfg\(test\)\]\s*mod queued_tests;/);
  assert.match(stagingService, /#\[cfg\(test\)\]\s*mod assembly_flow_tests;/);
  const harness = await read("engine/tests/file_chunk_staging_tdd.rs");
  assert.match(harness, /file_chunk_staging\/mod\.rs/);
  for (const [path, source] of await sources("engine/tests/file_chunk_staging")) {
    assert.doesNotMatch(codeOnly(source), /#\[ignore/, path);
  }
  const worker = codeOnly(await read("engine/src/services/file_transfer/staging/work.rs"));
  assert.match(worker, /try_acquire_owned/);
  assert.match(worker, /spawn_blocking/);
  assert.doesNotMatch(worker, /OnceLock|PasswordWorkGate|acquire_owned\(\)\.await/);
  for (const lang of ["en", "zh-CN"]) {
    const staging = await read(`docs/${lang}/large-file-staging.md`);
    assert.match(staging, /ChunkStaging/);
    assert.match(staging, /Unconfirmed/);
    assert.match(staging, /0400/);
    assert.ok((await read(`docs/${lang}/README.md`)).includes("large-file-staging.md"));
  }
});

test("large-file default test target includes manifest stream and Unix descriptor coverage", async () => {
  const harness = await read("engine/tests/file_transfer_tdd.rs");
  for (const module of ["manifest", "streams", "files"]) {
    assert.ok(harness.includes(`#[path = "file_transfer/${module}.rs"]`));
    const source = await read(`engine/tests/file_transfer/${module}.rs`);
    assert.match(source, /#\[test\]/);
    assert.doesNotMatch(source, /#\[ignore/);
  }
  const workflow = await read(".github/workflows/ci.yml");
  assert.ok(workflow.includes("run: cargo test --manifest-path engine/Cargo.toml --locked"), "CI must run all default Rust targets");
  assert.match(await read("scripts/quality-gate.sh"), /cargo test --manifest-path .* --locked/);
});

test("large-file bilingual documentation separates implemented checks from unconnected distribution", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const proposal = await read(`docs/${lang}/large-file-distribution.md`);
    const core = await read(`docs/${lang}/large-file-core.md`);
    assert.match(proposal, /https:\/\/github\.com\/Team-silvortex\/cyanrex-lab\/issues\/2/);
    assert.match(core, /copy_chunk_checked/);
    assert.match(core, /64 KiB/);
    assert.match(core, /4 MiB/);
    assert.match(core, /fsync/);
    assert.ok((await read(`docs/${lang}/README.md`)).includes("large-file-core.md"));
  }
});
