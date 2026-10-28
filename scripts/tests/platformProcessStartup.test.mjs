import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("process command is explicit and independent of Engine, storage, listeners and environment defaults", async () => {
  const code = codeOnly(await read("engine/src/bin/cyanrex-platform/main.rs"));
  for (const value of ["check-process", "--uid", "--gid", "--groups", "prepare_platform_process",
    "verify_current_thread", "spawn_scoped", "take(19)", "take(129)", "2048", "4096", "exit(64)", "exit(1)",
    "platform process check passed; no host started"]) assert.ok(code.includes(value), value);
  assert.doesNotMatch(code, /tokio|build_state|AppState|AuthService|TcpListener|PgPool|PgConnectOptions|dotenv|env::vars|env::var\(|set_var|remove_var|capset|setuid|setgid|setgroups/);
  assert.ok(code.indexOf("let checked = prepare_platform_process") < code.indexOf("spawn_scoped"));
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs"])
    assert.doesNotMatch(await read(path), /prepare_platform_process|PlatformProcessStartup/);
});

test("Linux startup gates the single main thread before and after the sole irreversible restriction", async () => {
  const code = codeOnly(await read("engine/src/platform_host/process_startup.rs"));
  for (const value of ["PlatformProcessIdentity", "u32::MAX", "groups.len() > 128", "pair[0] >= pair[1]",
    "PR_SET_NO_NEW_PRIVS", "libc::c_ulong", "true, false", "true, true", "false, true", "self.pid"])
    assert.ok(code.includes(value), value);
  const set = code.indexOf("PR_SET_NO_NEW_PRIVS");
  assert.ok(code.indexOf("status::read()?.validate") < set);
  assert.ok(code.lastIndexOf("status::read()?.validate") > set);
  assert.doesNotMatch(code, /capset|setuid|setgid|setgroups|PR_CAPBSET_DROP|PR_CAP_AMBIENT_CLEAR_ALL|Serialize|Deserialize|Default|std::env/);
  assert.doesNotMatch(code, /#\[derive\([^\]]*(?:Debug|Clone|Serialize)[^\]]*\)\]\s*pub struct PlatformProcessStartup/);
});

test("native status is bounded verified procfs and cross-checked without acquiring privileges", async () => {
  const code = codeOnly(await read("engine/src/platform_host/process_startup/status.rs"));
  for (const value of ["/proc/thread-self/status", "16 * 1024", "MAX_STATUS_BYTES + 1", "O_NOFOLLOW",
    "O_CLOEXEC", "fstatfs", "PROC_SUPER_MAGIC", "getresuid", "getresgid", "getgroups", "PR_GET_NO_NEW_PRIVS",
    "CapInh", "CapPrm", "CapEff", "CapAmb", "CapBnd", "NoNewPrivs", "self.threads != 1",
    "self.uid != [identity.uid; 4]", "self.gid != [identity.gid; 4]", "self.capabilities != [0; 4]"])
    assert.ok(code.includes(value), value);
  assert.doesNotMatch(code, /capset|setuid|setgid|setgroups|std::env|write_all|create_dir/);
  const tests = await read("engine/tests/platform_process_startup_tdd.rs");
  assert.equal([...tests.matchAll(/#\[test\]\s*fn platform_process_/g)].length, 7);
  assert.ok(tests.includes("capability_free_child"));
  assert.ok(tests.includes("SYS_capset"));
  assert.ok(tests.includes('CARGO_BIN_EXE_cyanrex-platform'));
  assert.doesNotMatch(tests, /#\[ignore/);
});

test("bilingual process docs separate irreversible startup checks from sandbox and host acceptance", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const source = await read(`docs/${locale}/platform-process-startup.md`);
    for (const value of ["issues/3", "check-process", "u32::MAX", "128", "/proc/thread-self/status",
      "no_new_privs=1", "verify_current_thread", "bounding", "platform-session-permissions.md"])
      assert.ok(source.includes(value), value);
    assert.match(source, locale === "en" ? /not a sandbox/ : /不是沙箱/);
    assert.match(source, locale === "en" ? /does not start a host/ : /不启动宿主/);
    await assertDocumentationReference(`docs/${locale}/README.md`, `docs/${locale}/platform-process-startup.md`);
    await assertDocumentationReference(`docs/${locale}/platform-host-preflight.md`, `docs/${locale}/platform-process-startup.md`);
  }
});
