import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const directory = "engine/src/platform_http/";
const read = path => readFile(new URL(directory + path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const ordered = (source, phrases) => {
  let previous = -1;
  for (const phrase of phrases) {
    const current = source.indexOf(phrase, previous + 1);
    assert.ok(current > previous && current >= 0, `present and ordered: ${phrase}`);
    previous = current;
  }
};
const states = [
  ["mod.rs", "TaskContentHttpState", "max_in_flight"],
  ["catalog_task/mod.rs", "CatalogTaskContentHttpState", "max_in_flight"],
  ["task_index/mod.rs", "TaskContentIndexHttpState", "max_in_flight"],
  ["text_artifact/mod.rs", "TextArtifactHttpState", "max_in_flight"],
  ["session_lifecycle/mod.rs", "SessionLifecycleHttpState", "max_in_flight"],
  ["file_download_manifest/mod.rs", "FileDownloadManifestHttpState", "capacity"],
  ["file_chunk_read/mod.rs", "FileChunkReadHttpState", "capacity"],
];

// Architecture guards only: constructor behavior and response/Source lifetime tests remain Rust tests.
// The initial Red records duplicate responsibility, not a demonstrated behavioral defect.
test("HTTP configuration is private pure validation with Origin preceding every capacity", async () => {
  const module = code(await read("mod.rs"));
  assert.match(module, /(?:^|\n)mod config;/);
  assert.doesNotMatch(module, /pub(?:\([^)]*\))?\s+(?:mod|use)\s+config\b/);
  const helper = code(await read("config.rs"));
  assert.match(helper, /pub\(super\) fn validate\(\s*origin: &str,\s*capacities: &\[usize\],?\s*\) -> Result<\(\), TaskContentHttpConfigError>/);
  ordered(helper, ["security::validate_origin(origin)?", "capacities", ".iter()", ".any(", "!(1..=16).contains(", "TaskContentHttpConfigError::InvalidCapacity", "Ok(())"]);
  assert.doesNotMatch(helper, /Semaphore|\bArc\b|\bVec\b|\bString\b|to_owned|to_string|\.clone\(|\.await|async\s|\b(?:static|thread_local|OnceLock|LazyLock)\b|std::(?:fs|net|env)|tokio::|sqlx|\.connect\(|\.begin\(|\.manifest_workspace_for\(/,
    "the shared policy must not own admission, allocate configuration or perform I/O");
});

test("each HTTP state delegates validation but keeps its own clone-shared admission budget", async () => {
  for (const [path, name, capacity] of states) {
    const source = code(await read(path));
    assert.match(source, new RegExp(`#\\[derive\\(Clone\\)\\]\\s*pub struct ${name}\\b`), path);
    assert.match(source, /origin: String,\s*admission: Arc<Semaphore>/, path);
    assert.match(source, new RegExp(`config::validate\\(origin, &\\[${capacity}\\]\\)\\?;`), path);
    ordered(source, [`config::validate(origin, &[${capacity}])?`, "origin: origin.to_owned()", `admission: Arc::new(Semaphore::new(${capacity}))`]);
    assert.equal((source.match(/Arc::new\(Semaphore::new\(/g) || []).length, 1, path);
    assert.doesNotMatch(source, /security::validate_origin\(|1\s*\.\.=\s*16|\b(?:static|thread_local|OnceLock|LazyLock)\b/, path);
  }
});

test("all compatibility constructors still use four local HTTP slots without helper allocation", async () => {
  for (const [path] of states) {
    const source = code(await read(path));
    assert.match(source, /Self::with_capacity\([^;]*origin,\s*4\s*\)/, path);
    assert.doesNotMatch(source, /config::(?:new|budget|admission|semaphore|with_capacity)\(/, path);
  }
  const helper = code(await read("config.rs"));
  assert.equal((helper.match(/\bfn\s+/g) || []).length, 1);
});

test("distribution checks both capacities before workspace derivation and retains independent states", async () => {
  const source = code(await read("file_distribution/mod.rs"));
  ordered(source.slice(source.indexOf("pub fn with_capacities(")), [
    "config::validate(origin, &[manifest_capacity, chunk_capacity])?",
    ".manifest_workspace_for(&source)",
    "FileDownloadManifestHttpState::with_capacity(",
    "source.clone()",
    "manifest_capacity",
    "FileChunkReadHttpState::with_capacity(source, workspace, origin, chunk_capacity)",
    "Ok(Self { manifests, chunks })",
  ]);
  assert.match(source, /Self::with_capacities\(source, workspace, origin, 4, 4\)/);
  assert.match(source, /manifests: FileDownloadManifestHttpState,\s*chunks: FileChunkReadHttpState/);
  for (const error of ["InvalidOrigin", "InvalidCapacity"])
    assert.ok(source.includes(`TaskContentHttpConfigError::${error} => Self::${error}`), error);
  assert.doesNotMatch(source, /security::validate_origin\(|1\s*\.\.=\s*16|Semaphore|\bArc\b|\.admission|\.connect\(|\.begin\(|\.await|\b(?:static|thread_local|OnceLock|LazyLock)\b/);
});

test("Session Source and retained platform backends share validation not a global budget or fallback", async () => {
  const source = code(await read("session_lifecycle/mod.rs"));
  for (const phrase of [
    "Self::configured(Backend::Source(source), origin, max_in_flight)",
    "Self::configured(Backend::Platform(connector), &origin, max_in_flight)",
    "fn configured(",
    "config::validate(origin, &[max_in_flight])?",
    "admission: Arc::new(Semaphore::new(max_in_flight))",
  ]) assert.ok(source.includes(phrase), phrase);
  const configured = source.slice(source.indexOf("fn configured("), source.indexOf("pub fn build_session_lifecycle_router("));
  assert.doesNotMatch(configured, /Backend::Source|Backend::Platform|\.connect\(|\.pool|\.begin\(|\.await|\.unwrap|\.or_else|\.unwrap_or/);
});
