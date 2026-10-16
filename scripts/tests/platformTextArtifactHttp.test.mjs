import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
async function sources(directory, extension) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sources(path, extension));
    else if (extension.test(entry.name)) result.push([path, await read(path)]);
  }
  return result;
}

// Source-level boundary checks complement, but never replace, actual HTTP/Session regressions.
test("text Artifact HTTP remains standalone without live, browser, SDK or issuer wiring", async () => {
  const files = await Promise.all(["engine/src/application.rs", "engine/src/state.rs", "engine/src/main.rs",
    "engine/openapi/openapi.json"].map(async path => [path, await read(path)]));
  for (const [directory, extension] of [["engine/src/routes", /\.rs$/], ["sdk-js/src", /\.ts$/],
    ["frontend/src/features/tasks", /\.tsx?$/]]) files.push(...await sources(directory, extension));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source),
    /\b(?:TextArtifactHttpState|build_text_artifact_router)\b|\/platform\/v1\/text-artifacts/, path);
  const taskRouter = codeOnly(await read("engine/src/platform_http/mod.rs"));
  assert.doesNotMatch(taskRouter.slice(taskRouter.indexOf("pub fn build_task_content_router(")),
    /build_text_artifact_router|TextArtifactHttpState|\.merge\(/,
    "Task router composition is unchanged; text capacity is not a hidden shared-global quota");
});

test("text Artifact HTTP reuses bounded transport and the two current-Session commands only", async () => {
  const all = (await sources("engine/src/platform_http/text_artifact", /\.rs$/))
    .map(([, source]) => codeOnly(source)).join("\n");
  for (const name of ["create_session_artifact", "read_session_artifact"])
    assert.equal([...all.matchAll(new RegExp(`\\.${name}\\(`, "g"))].length, 1, name);
  assert.doesNotMatch(all, /\b(?:validate_session|revise_session_artifact|create_session_content_task|register_intent|advance|install_empty_namespace|spawn|spawn_blocking)\s*\(|sqlx::query|\.transaction\(|\.commit\(|std::env|tokio::fs/);
  for (const pattern of [/admission::acquire\(/, /admission::response\(/, /security::credentials\(/,
    /security::response_headers/, /security::bounded_request_bytes\(/,
    /contract::parse\(/, /validate_text_bytes\(/,
    /"cyanrex\.task-text"/, /"text\/plain"/, /2\s*\*\s*1024\s*\*\s*1024/]) assert.match(all, pattern);
  assert.match(all, /Arc\s*<\s*Semaphore\s*>/);
  assert.match(all, /Self::with_capacity\([\s\S]*?,\s*4\s*\)/);
  assert.match(all, /config::validate\(origin, &\[max_in_flight\]\)\?;/);
  assert.match(all, /admission: Arc::new\(Semaphore::new\(max_in_flight\)\)/);
  assert.ok(all.indexOf("config::validate(origin, &[max_in_flight])?") < all.indexOf("Semaphore::new(max_in_flight)"));
  const configuration = codeOnly(await read("engine/src/platform_http/config.rs"));
  assert.match(configuration, /security::validate_origin\(origin\)\?;[\s\S]*capacities\s*\.iter\(\)\s*\.any\([\s\S]*!\(1\s*\.\.=\s*16\)\.contains\(/);
  const transport = codeOnly(await read("engine/src/platform_http/security.rs"));
  assert.match(transport, /MAX_REQUEST_BYTES:\s*usize\s*=\s*64\s*\*\s*1024/);
  assert.match(transport, /bounded_request_bytes\(request, empty, MAX_REQUEST_BYTES\)/);
  assert.match(transport, /Duration::from_secs\(2\)/);
  const textValidation = codeOnly(await read("engine/src/services/task_content.rs"));
  assert.match(textValidation, /fn validate_text_bytes\(/);
  assert.match(textValidation, /MAX_TEXT_PAYLOAD_BYTES/);
  const model = await read("engine/src/models/collaboration/content.rs");
  assert.match(model, /MAX_TEXT_PAYLOAD_BYTES:\s*usize\s*=\s*256\s*\*\s*1024/);
});

test("text Artifact HTTP PostgreSQL inventory is exact and deliberately selected by CI", async () => {
  const cases = [];
  for (const module of ["behavior", "faults", "concurrency"]) {
    const source = await read(`engine/tests/text_artifact_http/${module}.rs`);
    for (const [, name] of source.matchAll(/async fn (postgres_text_artifact_http_\w+)\(/g))
      cases.push(`${module}::${name}`);
  }
  assert.equal(cases.length, 10, "new SQL cases require explicit runner inventory changes");
  const runner = await read("scripts/test-platform-text-artifact-http.sh");
  const selected = [...runner.matchAll(/^  ((?:\w+::)*postgres_text_artifact_http_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test text_artifact_http_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\$\{?DATABASE_URL\b/);
  const target = await read("engine/tests/text_artifact_http_tdd.rs");
  for (const module of ["behavior", "faults", "concurrency"])
    assert.match(target, new RegExp(`mod ${module};`));
  const step = (await read(".github/workflows/ci.yml"))
    .split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL:\s*"postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /&& bash scripts\/test-platform-text-artifact-http\.sh/);
});
