import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const dir = "engine/src/platform_http/";
const ordered = (s, words) => {
  let last = -1;
  for (const word of words) {
    const at = s.indexOf(word);
    assert.ok(at >= 0 && at > last, `present and ordered ${word}`);
    last = at;
  }
};

// Static composition/inventory only. Default Rust and separate Source SQL prove behavior.
test("distribution accepts one opaque source workspace and independent bounded capacities", async () => {
  const s = code(await read(`${dir}file_distribution/mod.rs`));
  assert.match(s, /pub fn new\(\s*source: DurableAuthSource,\s*workspace: SessionFileChunkReadWorkspace,\s*origin: &str,?\s*\)/);
  assert.match(s, /pub fn with_capacities\([\s\S]*?manifest_capacity: usize,\s*chunk_capacity: usize/);
  for (const phrase of ["FileDistributionHttpConfigError", "InvalidOrigin", "InvalidCapacity", "InvalidWorkspace", "origin, 4, 4", "manifests: FileDownloadManifestHttpState", "chunks: FileChunkReadHttpState"]) assert.ok(s.includes(phrase), phrase);
  ordered(s.slice(s.indexOf("pub fn with_capacities(")), ["security::validate_origin(origin)?", "1..=16", ".manifest_workspace_for(&source)", "FileDownloadManifestHttpState::with_capacity", "source.clone()", "FileChunkReadHttpState::with_capacity(source, workspace, origin, chunk_capacity)"]);
  assert.doesNotMatch(s, /Serialize|Deserialize|pub (?:manifests|chunks):|pub fn (?:source|workspace|router|manifest)|SessionBind|install_empty|\.begin\(|\.connect\(|std::(?:fs|net|env)|tokio::spawn|Semaphore::new/);
  const exports = await read(`${dir}mod.rs`);
  assert.match(exports, /#\[cfg\(unix\)\]\s*mod file_distribution;/);
  assert.ok(exports.includes("build_file_distribution_router"));
});

test("manifest configuration is derived from the original cached download pins without storage work", async () => {
  const s = code(await read("engine/src/services/auth_service/durable_source/file_chunk_read.rs"));
  const getter = s.slice(s.indexOf("pub(crate) fn manifest_workspace_for("), s.indexOf("pub(crate) fn file_scope("));
  assert.match(getter, /std::result::Result<SessionFileDownloadWorkspace, SessionFileDownloadError>/);
  ordered(getter, ["source.check_file_download_scope(&self.downloads)?", "Ok(self.downloads.clone())"]);
  assert.doesNotMatch(getter, /pub fn|sqlx|\.await|\.pool|\.begin|\.install|\.staging|std::fs|Semaphore/);
  const checks = code(await read("engine/src/services/auth_service/durable_source/file_download.rs"));
  const scope = checks.slice(checks.indexOf("pub(super) fn check_file_download_scope("));
  for (const phrase of ["scope.authority_id != self.authority_id", "scope != workspace.recipients.store.scope()", "SourceMismatch", "ScopeMismatch"]) assert.ok(scope.includes(phrase), phrase);
});

test("paired router uses original fallback free typed routes and one private fallback header layer", async () => {
  const s = code(await read(`${dir}file_distribution/mod.rs`));
  const builder = s.slice(s.indexOf("pub fn build_file_distribution_router("));
  for (const phrase of ["file_download_manifest::routes().with_state(state.manifests)", "file_chunk_read::routes().with_state(state.chunks)", ".fallback(not_found)", "security::response_headers", "HttpError::new(StatusCode::NOT_FOUND, \"not_found\")"]) assert.ok(builder.includes(phrase), phrase);
  assert.equal((builder.match(/\.fallback\(/g) || []).length, 1);
  assert.equal((builder.match(/\.layer\(/g) || []).length, 1);
  assert.doesNotMatch(builder, /build_file_download_manifest_router|build_file_chunk_read_router|\.route\(|\.oneshot|credentials|request_bytes|sqlx|\.await/);
  for (const module of ["file_download_manifest", "file_chunk_read"]) {
    const old = code(await read(`${dir}${module}/mod.rs`));
    assert.match(old, /pub\(super\) fn routes\(\) -> Router<\w+>/);
    assert.ok(old.includes("any(handlers::dispatch)"));
    assert.match(old, /routes\(\)\s*\.fallback\(handlers::dispatch\)\s*\.layer\(middleware::from_fn\(security::response_headers\)\)\s*\.with_state\(state\)/);
  }
});

test("distribution defaults and composed Source inventory execute real routing and byte lifetimes", async () => {
  const main = await read("engine/tests/file_distribution_http_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  let count = 0;
  for (const file of await readdir(new URL("engine/tests/file_distribution_http/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_distribution_http/${file}`);
    const tests = (s.match(/#\[(?:tokio::)?test\]/g) || []).length;
    count += tests;
    assert.doesNotMatch(s, /#\[ignore/);
    if (tests) assert.match(main, new RegExp(`\\bmod ${file.replace(/\.rs$/, "")};`));
  }
  assert.equal(count, 9);
  const baseline = await read("engine/tests/file_distribution_http/baseline.rs");
  ordered(baseline, ["checked(", "assert_eq!(responses.len(), 2)"]);
  for (const phrase of ["SERVICE_UNAVAILABLE", "service_unavailable", "unconfirmed", "root_before", "root_after"]) assert.ok(baseline.includes(phrase), phrase);
  const bridge = code(await read("engine/tests/file_delivery/distribution_bridge.rs"));
  for (const phrase of ["build_file_distribution_router", "FileDistributionHttpState::new", "f.workspace.clone()", "bridge.router.clone().oneshot(request)"]) assert.ok(bridge.includes(phrase), phrase);
  assert.doesNotMatch(bridge, /build_file_download_manifest_router|build_file_chunk_read_router|\.metadata\.clone|\.chunks\.clone/);
  const source = await read("engine/tests/file_delivery/source.rs");
  assert.ok(source.includes("DistributionBridge as Bridge"));
  assert.match(source, /#\[path = "distribution_lifetime\.rs"\]\s*mod distribution_lifetime;/);
  const lifetime = await read("engine/tests/file_delivery/distribution_lifetime.rs");
  for (const phrase of ["clone.slice(1..3)", "source_full(f).await", "retained_json", "drop(slice)", "recipient_delta", "f.sharing.snapshot().await", "metadata_path", "chunk_path"]) assert.ok(lifetime.includes(phrase), phrase);
  assert.equal((lifetime.match(/#\[ignore/g) || []).length, 1);
  assert.ok((await read("scripts/test-file-delivery.sh")).includes("source::distribution_lifetime::postgres_distribution_keeps_binary_owners_and_metadata_independent_across_clones_and_revocation"));
});

test("paired distribution remains unmounted with current bilingual consistency and isolation limits", async () => {
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /FileDistributionHttpState|FileDistributionHttpConfigError|build_file_distribution_router/, path);
  for (const lang of ["en", "zh-CN"]) {
    const s = await read(`docs/${lang}/large-file-distribution-router.md`);
    for (const phrase of ["0.5.7", "0.3.0", "2026-10-13", "FileDistributionHttpState", "FileDistributionHttpConfigError", "with_capacities", "build_file_distribution_router", "/platform/v1/file-manifests", "/platform/v1/file-chunks", "not_attempted", "Bytes", "file_distribution_http_tdd", "test-file-delivery.sh", "fileDistributionHttp", "106", "54", "8"]) assert.ok(s.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-manifest-http", "large-file-chunk-http"]) assert.ok((await read(`docs/${lang}/${file}.md`)).includes("large-file-distribution-router.md"), `${lang}: ${file}`);
  }
});
