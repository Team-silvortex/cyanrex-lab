// Inventories complement real ordinary-role SQL and explicit native plain/TLS host evidence.
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const compact = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/[^\n]*/gm, "").replace(/\s+/g, "");
const body = (source, declaration) => {
  const start = source.indexOf(declaration); assert.ok(start >= 0, declaration);
  const opening = source.indexOf("{", start);
  let end = opening + 1, depth = 1;
  for (; end < source.length && depth; end++) {
    if (source[end] === "{") depth++;
    else if (source[end] === "}") depth--;
  }
  assert.equal(depth, 0); return source.slice(start, end);
};

test("Task text host validates eight capacities before a single original split while earlier constructors omit the route", async () => {
  const source = compact(await read("engine/src/platform_http/workspace_router/mod.rs"));
  for (const name of ["with_replace_capacities", "with_text_publication_capacities"]) {
    const old = body(source, `pubfn${name}(`);
    assert.ok(old.includes("task_text:None"));
    assert.doesNotMatch(old, /task_text:Some\(|split_platform_task_text_publication_connectors/);
  }
  const added = body(source, "pubfnwith_task_text_publication_capacities(");
  const capacities = "login_capacity,lifecycle_capacity,index_capacity,content_capacity,create_capacity,replace_capacity,publish_capacity,task_publish_capacity";
  assert.match(added, new RegExp(`config::validate\\(storage.origin\\(\\),&\\[${capacities},?\\],?\\)\\?;`));
  assert.ok(added.indexOf("config::validate(") < added.indexOf("split_platform_task_text_publication_connectors(storage)"));
  assert.ok(added.includes("task_text:Some(TaskTextArtifactPublishHttpState::with_capacity(task_text,task_publish_capacity,)?),"));
  const split = body(compact(await read("engine/src/platform_host/session_pair.rs")), "pub(crate)fnsplit_platform_task_text_publication_connectors(");
  assert.equal([...split.matchAll(/Arc::new\(storage\)/g)].length, 1);
  assert.equal([...split.matchAll(/Arc::clone\(&storage\)/g)].length, 7);
  assert.deepEqual([...split.matchAll(/(Platform\w+Connector)::from_shared\(/g)].map(m => m[1]),
    ["PlatformLoginConnector", "PlatformSessionConnector", "PlatformTaskIndexConnector", "PlatformTaskContentConnector",
      "PlatformTaskCreateConnector", "PlatformTaskReplaceConnector", "PlatformTextArtifactConnector", "PlatformTaskTextArtifactConnector"]);
  assert.doesNotMatch(split + added, /\.connect\(|\.await|inspect_|verify_|\.clone\(|std::(?:env|fs)|PgPool/);
  const router = body(source, "pubfnbuild_platform_workspace_router(");
  assert.equal([...router.matchAll(/\.merge\(/g)].length, 8);
  assert.ok(router.includes("ifletSome(task_text)=state.task_text{router.merge(task_text_artifact_publish::routes().with_state(task_text))"));
  assert.equal([...router.matchAll(/\.fallback\(/g)].length, 1);
  assert.equal([...router.matchAll(/\.layer\(/g)].length, 1);
  for (const file of ["task_text_artifact_connection.rs", "text_artifact_connection.rs"]) {
    const connector = compact(await read(`engine/src/platform_host/${file}`));
    const shared = body(connector, "pub(super)fnfrom_shared(");
    assert.ok(shared.includes("storage,workers:Arc::new(Semaphore::new(4))"));
    assert.doesNotMatch(shared, /\.connect\(|inspect_|verify_|std::(?:env|fs)/);
  }
});

test("the bounded CLI explicitly selects Task text publication without changing UI public APIs or transport limits", async () => {
  const main = compact(await read("engine/src/bin/cyanrex-platform/main.rs"));
  assert.ok(main.includes(".take(19)"));
  assert.ok(main.includes('text.get(without_notify-1)==Some(&"--task-text-publication")'));
  assert.ok(main.includes("task_text_publication||session_ui||notify_systemd"));
  const host = compact(await read("engine/src/bin/cyanrex-platform/serve_loopback/mod.rs"));
  const choice = body(host, "fnrun_with_transport(");
  assert.ok(choice.includes("if task_text_publication".replaceAll(" ", "")));
  assert.ok(choice.includes("with_task_text_publication_capacities(storage,4,4,4,4,4,4,4,4,)"));
  assert.ok(choice.includes("}else{PlatformWorkspaceHttpState::with_text_publication_capacities(storage,4,4,4,4,4,4,4,)"));
  assert.ok(choice.indexOf("open_platform_storage_expected(") < choice.indexOf("if task_text_publication".replaceAll(" ", "")));
  assert.ok(choice.indexOf("with_task_text_publication_capacities(") < choice.indexOf("TcpListener::bind("));
  const defaults = await read("engine/tests/platform_task_text_publication_defaults_tdd.rs");
  assert.equal([...defaults.matchAll(/^#\[test\]/gm)].length, 3);
  const transport = compact(await read("engine/src/bin/cyanrex-platform/serve_loopback/http.rs"));
  assert.ok(transport.includes("CONNECTIONS:usize=16"));
  for (const path of ["engine/src/bin/cyanrex-platform/session_ui.rs", "engine/src/bin/cyanrex-platform/session_ui/task-edit.js",
    "engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /with_task_text_publication_capacities|TaskTextArtifactPublishHttpState|--task-text-publication/, path);
});

test("six Task text host SQL selectors including native plain and TLS are explicitly inventoried and selected by CI", async () => {
  const directory = "engine/tests/platform_task_text_publication_host", cases = [];
  const harness = await read("engine/tests/platform_task_text_publication_host_tdd.rs");
  for (const file of await readdir(new URL(`${directory}/`, root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`${directory}/${file}`);
    const declared = [...source.matchAll(/async fn (postgres_platform_task_text_publication_\w+)\(/g)];
    const ignored = [...source.matchAll(/#\[tokio::test\]\s*#\[ignore = "([^"]+)"\]\s*async fn (postgres_platform_task_text_publication_\w+)\(/g)];
    assert.equal(declared.length, ignored.length, file);
    for (const [, reason, name] of ignored) {
      assert.match(reason, /requires an explicit disposable CYANREX_TEST_DATABASE_URL/);
      assert.ok(harness.includes(`platform_task_text_publication_host/${file}`));
      cases.push(`${file.slice(0, -3)}::${name}`);
    }
  }
  assert.equal(cases.length, 6);
  const runner = await read("scripts/test-platform-task-text-publication-host.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_task_text_publication_\w+)\b/gm)].map(m => m[1]);
  assert.equal(new Set(selected).size, 6); assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/set -euo pipefail/, /CYANREX_TEST_DATABASE_URL:\?/, /--locked --test platform_task_text_publication_host_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const workflow = await read(".github/workflows/ci.yml");
  const step = workflow.split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.equal([...step.matchAll(/bash scripts\/test-platform-task-text-publication-host\.sh/g)].length, 1);
  assert.ok(workflow.trimEnd().split("\n").length <= 600);
});

test("Task text host documentation is bilingual mirrored and describes an explicit switch not browser saving", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const path = `docs/${locale}/platform-text-artifact-connection.md`, guide = await read(path);
    for (const text of ["--task-text-publication", "with_task_text_publication_capacities", "test-platform-task-text-publication-host.sh"])
      assert.ok(guide.includes(text), `${locale}: ${text}`);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-text-artifact-connection.md`));
    const security = await read(`docs/${locale}/security.md`);
    assert.ok(security.includes("--task-text-publication"));
    assert.equal(security, await read(`frontend/public/course/${locale}/security.md`));
  }
});
