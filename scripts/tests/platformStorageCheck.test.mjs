import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("storage diagnostics use one connection and one budget through inspection and close without a retained baseline", async () => {
  const code = codeOnly(await read("engine/src/platform_host/storage_check.rs"));
  for (const value of ["pub async fn check_platform_database_storage", "-> Result<()>", "options(config, endpoint, password)",
    "tokio::time::timeout(Duration::from_secs(10)", "inspect_platform_database(config, connection)",
    "inspect_platform_storage_markers(config, connection, &catalog)", "inspected.and(closed)"])
    assert.ok(code.includes(value), value);
  assert.equal([...code.matchAll(/dial\(&options\)/g)].length, 1);
  assert.match(code, /let inspected = expected\.inspect\(config, &mut connection\)\.await;\s*let closed = connection\s*\.close\(\)/);
  const inspect = code.split("async fn inspect(")[1]?.split("\n}\n")[0] ?? "";
  assert.match(inspect, /Self::Current\s*=>\s*inspect_platform_database\(config, connection\)\.await\?/);
  assert.doesNotMatch(inspect, /dial\(|\.close\(|timeout\(/);
  assert.doesNotMatch(code, /PlatformDatabaseConnector|PgPool|PoolOptions|from_str|std::env|set_var|remove_var|println!|eprintln!|install_|GRANT |REVOKE |CREATE |Serialize|tracing::/);
  assert.ok((await read("engine/src/platform_host/mod.rs")).includes("pub use storage_check::check_platform_database_storage"));
});

test("the CLI gates retained private input, observes the root before runtime, and rechecks it after teardown on either outcome", async () => {
  const main = codeOnly(await read("engine/src/bin/cyanrex-platform/main.rs"));
  const load = main.indexOf("let result = config::load(path)");
  assert.ok(load >= 0 && main.indexOf("checked.verify_before_runtime()", load) > load);
  assert.ok(main.includes("storage_check::check(result?, &checked)"));
  const code = codeOnly(await read("engine/src/bin/cyanrex-platform/storage_check.rs"));
  const observe = code.indexOf("inspect_platform_artifact_root(");
  const build = code.indexOf("Builder::new_current_thread()");
  const run = code.indexOf(".block_on(async");
  const drop = code.indexOf("drop(runtime)");
  const verify = code.indexOf("verify_platform_artifact_root(");
  assert.ok(observe >= 0 && observe < build && build < run && run < drop && drop < verify);
  assert.match(code.slice(run, drop), /match\s+&input\.storage_expectation/);
  assert.ok(code.lastIndexOf("process.verify_before_runtime()") > verify);
  assert.ok(code.includes("inspected.and(root).and(process)"));
  assert.doesNotMatch(code + main, /new_multi_thread|TcpListener|PgPool|build_state|AuthService|tracing_subscriber|dotenv|env::var\(|set_var|remove_var|install_empty/);
  const config = codeOnly(await read("engine/src/bin/cyanrex-platform/config.rs"));
  assert.ok(config.includes("pub(super) struct ValidatedInput"));
  assert.doesNotMatch(config, /derive\([^\)]*(?:Debug|Clone|Serialize)[^\)]*\)|ConnectOptions|Connector::new|inspect_platform/);
});

test("all five SQL CLI cases and four native faults have exact opt-in or default execution paths", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_storage_check", root))) {
    if (!file.endsWith(".rs")) continue;
    const source = await read(`engine/tests/platform_storage_check/${file}`);
    for (const [, name] of source.matchAll(/async fn (postgres_platform_storage_check_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 5);
  const runner = await read("scripts/test-platform-storage-check.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_storage_check_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /--test platform_storage_check_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = (await read(".github/workflows/ci.yml")).split("- name: Run real PostgreSQL task artifact and review storage integration")[1]?.split("\n      - name:")[0] ?? "";
  assert.match(ci, /&& bash scripts\/test-platform-storage-check\.sh/);
  let defaults = "";
  for (const file of ["peer", "rejections"])
    defaults += await read(`engine/tests/platform_storage_check_defaults/${file}.rs`);
  assert.equal([...defaults.matchAll(/#\[test\]\s*fn platform_storage_cli_/g)].length, 4);
  assert.doesNotMatch(defaults, /#\[ignore/);
});

test("bilingual storage guides distinguish current observations, original runtime baselines and filesystem deadlines", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const source = await read(`docs/${locale}/platform-storage-check.md`);
    for (const value of ["issues/3", "check-storage", "check-config", "check_platform_database_storage", "PGOPTIONS",
      "platform-storage-markers.md", "platform-startup-input.md", "platform-database-connection.md",
      "platform storage observations completed; no host started"])
      assert.ok(source.includes(value), value);
    const prose = source.replace(/\s+/g, " ");
    assert.match(prose, locale === "en" ? /all observations are discarded/ : /观察值最终全部丢弃/);
    assert.match(prose, locale === "en" ? /no hard time bound/ : /没有硬耗时上限/);
    assert.match(prose, locale === "en" ? /valid same-name substitution/ : /有效同名替换/);
    for (const path of ["README.md", "platform-startup-input.md"])
      await assertDocumentationReference(`docs/${locale}/${path}`, `docs/${locale}/platform-storage-check.md`);
    assert.equal(source, await read(`frontend/public/course/${locale}/platform-storage-check.md`));
  }
});
