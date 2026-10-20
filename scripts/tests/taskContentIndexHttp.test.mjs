import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const directory = "engine/src/platform_http/task_index";
function names(source, name) {
  const match = new RegExp(`struct\\s+${name}\\s*\\{([^}]+)\\}`).exec(source);
  assert.ok(match, `missing ${name}`);
  return [...match[1].matchAll(/(?:pub(?:\([^)]*\))?\s+)?(\w+)\s*:/g)].map(([, field]) => field);
}
async function sources(directory, extension) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sources(path, extension));
    else if (extension.test(entry.name)) result.push([path, await read(path)]);
  }
  return result;
}

// Structural wiring and inventory guards only; Rust and disposable SQL tests prove behavior.
test("metadata HTTP binds strict raw positional queries to one trusted current-Session command", async () => {
  const state = codeOnly(await read(`${directory}/mod.rs`));
  assert.deepEqual(names(state, "TaskContentIndexHttpState"), ["backend", "origin", "admission"]);
  for (const pattern of [/SessionTaskContentIndexWorkspace/, /config::validate\(origin, &\[max_in_flight\]\)\?;/, /origin, 4\)/, /Arc::new\(Semaphore::new\(max_in_flight\)\)/, /response_headers/]) assert.match(state, pattern);
  const configuration = codeOnly(await read("engine/src/platform_http/config.rs"));
  assert.match(configuration, /security::validate_origin\(origin\)\?;[\s\S]*capacities\s*\.iter\(\)\s*\.any\([\s\S]*!\(1\.\.=16\)\.contains\(/);
  assert.ok(state.indexOf("config::validate(origin, &[max_in_flight])?") < state.indexOf("Semaphore::new(max_in_flight)"));
  const handler = codeOnly(await read(`${directory}/handlers.rs`));
  assert.equal([...handler.matchAll(/state\.backend\.list\s*\(/g)].length, 1);
  assert.equal([...state.matchAll(/\.list_session_content_tasks\s*\(/g)].length, 2, "one command per exclusively selected backend");
  assert.match(state, /Source\s*\{\s*source: DurableAuthSource,\s*workspace: SessionTaskContentIndexWorkspace/);
  assert.match(state, /Platform\(Arc<PlatformTaskIndexConnector>\)/);
  assert.doesNotMatch(state, /\.or_else\(|connect_lazy|\.validate_session\(/);
  for (const pattern of [/Method::GET/, /method_not_allowed\("GET"\)/, /request\.uri\(\)\.query\(\)/, /security::credentials/, /admission::acquire/, /admission::response/, /security::request_bytes\(request, true\)/, /state\.backend\.scope\(\)/]) assert.match(handler, pattern);
  assert.ok(handler.indexOf("request_bytes(request, true)") < handler.indexOf("state.backend.list("));
  const contract = codeOnly(await read(`${directory}/contract.rs`));
  assert.deepEqual(names(contract, "PageRequest"), ["after", "limit"]);
  for (const pattern of [/query\.len\(\) > 128/, /query\.split\('&'\)/, /split_once\('='\)/, /"after" if page\.after\.is_none\(\)/, /"limit" if page\.limit\.is_none\(\)/, /starts_with\('0'\)/, /is_ascii_digit/, /1\.\.=32/, /_ => return Err/]) assert.match(contract, pattern);
  assert.doesNotMatch(contract, /Deserialize|serde_urlencoded|form_urlencoded|percent_decode/);
});

test("metadata HTTP projects only explicit private metadata without creating a second persistence path", async () => {
  const contract = codeOnly(await read(`${directory}/contract.rs`));
  assert.deepEqual(names(contract, "IndexResponse"), ["schema_version", "entries", "next_after"]);
  assert.deepEqual(names(contract, "IndexEntry"), ["reference", "revision", "title", "status", "definition_ref", "updated_at", "payload_items"]);
  assert.equal([...contract.matchAll(/#\[derive\(Serialize\)\]/g)].length, 2);
  assert.doesNotMatch(contract, /skip_serializing|Deserialize/);
  const store = codeOnly(await read("engine/src/services/task_store/content_index.rs"));
  assert.doesNotMatch(store, /Serialize|Deserialize/);
  for (const [path, source] of await sources(directory, /\.rs$/)) {
    if (path.endsWith("/tests.rs")) continue;
    assert.doesNotMatch(codeOnly(source), /sqlx|PgPool|ArtifactStore|TaskProvider|TaskAssessment|AuthService\b|std::(?:fs|env|net|process)|install_empty|\.assess\(|\.transaction\(/, path);
  }
});

test("standalone metadata HTTP is absent from live routing OpenAPI SDK and browser composition", async () => {
  const files = await Promise.all(["engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json"].map(async path => [path, await read(path)]));
  for (const [folder, extension] of [["engine/src/routes", /\.rs$/], ["sdk-js/src", /\.[cm]?[jt]s$/], ["frontend/src", /\.[cm]?[jt]sx?$/], ["frontend/pages", /\.[cm]?[jt]sx?$/]]) files.push(...await sources(folder, extension));
  for (const [path, source] of files) assert.doesNotMatch(codeOnly(source), /TaskContentIndexHttpState|build_task_content_index_router|\/platform\/v1\/task-index|list_session_content_tasks/, path);
  assert.match(await read("engine/src/platform_http/mod.rs"), /pub use task_index::\{build_task_content_index_router, TaskContentIndexHttpState\};/);
  assert.match(await read("engine/src/platform_http/handlers.rs"), /query\(\)\.is_some\(\)/);
  assert.match(await read("engine/src/platform_http/catalog_task/handlers.rs"), /endpoint_at\(&request, "\/platform\/v1\/catalog-tasks"\)/);
});

test("all eight metadata HTTP SQL cases are mounted and exactly run in the existing disposable CI step", async () => {
  const harness = await read("engine/tests/task_content_index_http_tdd.rs");
  const expected = [];
  for (const [path, source] of await sources("engine/tests/task_content_index_http", /\.rs$/)) {
    const module = path.split("/").at(-1).replace(/\.rs$/, "");
    const cases = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"\]\s*async fn (postgres_index_http_\w+)\(/g)].map(([, name]) => `${module}::${name}`);
    if (cases.length) assert.ok(harness.includes(`#[path = "task_content_index_http/${module}.rs"]\nmod ${module};`), path);
    expected.push(...cases);
    assert.equal([...source.matchAll(/async fn postgres_index_http_\w+\(/g)].length, cases.length, `untracked SQL case in ${path}`);
    assert.ok(source.split("\n").length <= 601, path);
  }
  assert.equal(expected.length, 8);
  const runner = await read("scripts/test-task-content-index-http.sh");
  const listed = [...runner.matchAll(/^  (\w+::postgres_index_http_\w+)\b/gm)].map(([, name]) => name);
  assert.deepEqual(listed.sort(), expected.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--test task_content_index_http_tdd/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(step, /CYANREX_TEST_DATABASE_URL: "postgres:.*@127\.0\.0\.1:/);
  assert.match(step, /test-platform-task-content-http\.sh && bash scripts\/test-task-content-index-http\.sh && bash scripts\/test-platform-catalog-task-content-http\.sh/);
});
