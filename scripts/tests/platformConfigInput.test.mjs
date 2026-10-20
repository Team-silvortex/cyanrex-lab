import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("check-config gates private input before any worker and checks the main thread again afterward", async () => {
  const code = codeOnly(await read("engine/src/bin/cyanrex-platform/main.rs"));
  const prepare = code.indexOf("let checked = prepare_platform_process");
  const firstGate = code.indexOf("checked.verify_before_runtime()", prepare);
  const input = code.indexOf("let result = config::check(path)");
  const finalGate = code.indexOf("checked.verify_before_runtime()", input);
  const worker = code.indexOf("spawn_scoped");
  assert.ok(prepare >= 0 && prepare < firstGate && firstGate < input && input < finalGate && finalGate < worker);
  assert.ok(code.slice(finalGate, worker).includes("return result"));
  for (const value of ["check-config", "--config", "is_absolute()", "take(17)", "4096",
    "platform configuration check passed; no host started", "platform configuration check failed; no host started"])
    assert.ok(code.includes(value), value);
  assert.doesNotMatch(code, /tokio|PgPool|ConnectOptions|TcpListener|build_state|AuthService|dotenv|env::var\(|set_var|remove_var/);
  const startup = codeOnly(await read("engine/src/platform_host/process_startup.rs"));
  const verifier = startup.split("pub fn verify_before_runtime")[1]?.split("pub fn verify_current_thread")[0] ?? "";
  assert.match(verifier, /status::read\(\)\?\.validate\(&self\.identity, self\.pid, status::tid\(\), true, true\)/);
});

test("configuration records accept maps only while preserving duplicate keys and original pure validators", async () => {
  const code = codeOnly(await read("engine/src/bin/cyanrex-platform/config.rs"));
  assert.equal([...code.matchAll(/deny_unknown_fields/g)].length, 5);
  for (const value of ["MapOnly<Input>", "MapOnly<Namespaces>", "MapOnly<Database>", "MapOnly<Limits>",
    "MapOnly<Transport>", "PlatformHostPreflightConfig::new", "PlatformNamespaces::new",
    "PlatformDatabaseTarget::new", "PlatformDatabaseEndpoint::unix", "PlatformDatabaseEndpoint::loopback",
    "SessionStockLimits::new", "16 * 1024", "format_version != 1", "password.len() > 4096", "password.contains('\\0')"])
    assert.ok(code.includes(value), value);
  assert.doesNotMatch(code, /derive\([^\)]*(?:Debug|Clone|Serialize)[^\)]*\)|serde_json::Value|from_value|ConnectOptions|Connector::new|std::env|canonicalize|inspect_platform/);
  const map = codeOnly(await read("engine/src/bin/cyanrex-platform/config/map_only.rs"));
  assert.match(map, /T::deserialize\(MapAccessDeserializer::new\(map\)\)/);
  assert.match(map, /deserializer\.deserialize_map/);
});

test("private input uses bounded no-follow descriptors and rechecks the original file and visible parent", async () => {
  const code = codeOnly(await read("engine/src/bin/cyanrex-platform/private_input.rs"));
  for (const value of ["O_RDONLY", "O_CLOEXEC", "O_NOFOLLOW", "O_NONBLOCK", "O_DIRECTORY", "libc::openat",
    "meta.uid() != uid()", "0o700", "before.nlink() != 1", "0o400 | 0o600", "16 * 1024",
    "take(MAX_BYTES + 1)", "bytes.len() as u64 != before.len()", "meta.mtime_nsec()", "meta.ctime_nsec()",
    "directory_identity(&visible_parent.metadata()?) != parent_identity", "snapshot(&visible.metadata()?) != original",
    "snapshot(&file.metadata()?) != original", "operation().map_err(|_| ())"])
    assert.ok(code.includes(value), value);
  assert.doesNotMatch(code, /O_CREAT|O_TRUNC|O_WRONLY|set_permissions|chmod|write_all|create_dir|remove_|std::env|println!|eprintln!/);
});

test("default CLI tests and bilingual docs preserve check-only boundaries and private input examples", async () => {
  const target = await read("engine/tests/platform_config_input_tdd.rs");
  assert.doesNotMatch(target, /#\[ignore|platform_process_startup_tdd\.rs/);
  const cases = (await Promise.all(["files", "json", "process"].map(name => read(`engine/tests/platform_config_input/${name}.rs`)))).join("\n");
  assert.equal([...cases.matchAll(/#\[test\]\s*fn platform_config_cli_/g)].length, 4);
  assert.ok(cases.includes("watch_positive"));
  assert.ok(cases.includes("positive"));
  for (const locale of ["en", "zh-CN"]) {
    const source = await read(`docs/${locale}/platform-startup-input.md`);
    for (const value of ["issues/3", "check-config", "verify_before_runtime", "16 KiB", "0400", "0600",
      "0700", "PGOPTIONS", "artifact_root", "format_version", "platform-database-connection.md"])
      assert.ok(source.includes(value), value);
    assert.match(source, locale === "en" ? /does not connect/ : /不连接数据库/);
    assert.match(source, locale === "en" ? /not secure zeroization/ : /不等于安全清零/);
    for (const path of ["README.md", "platform-process-startup.md"])
      assert.ok((await read(`docs/${locale}/${path}`)).includes("platform-startup-input.md"));
    assert.equal(source, await read(`frontend/public/course/${locale}/platform-startup-input.md`));
  }
});
