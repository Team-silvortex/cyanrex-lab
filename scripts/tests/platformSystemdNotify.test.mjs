import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const strip = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => strip(source).replace(/\s+/g, "");
const directory = "engine/src/bin/cyanrex-platform/serve_loopback";

// These are wiring inventories, not receipt, supervisor activation or deployment evidence.
test("supervisor notification remains an explicit ordered serve suffix without changing diagnostic shapes", async () => {
  const main = await read("engine/src/bin/cyanrex-platform/main.rs");
  for (const value of ["take(19)", "7 | 9 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18", '"--notify-systemd"',
    '"check-process"', '"check-config"', '"check-storage"', '"serve-loopback"', '"serve-loopback-tls"'])
    assert.ok(main.includes(value), value);
  assert.doesNotMatch(strip(main), /NOTIFY_SOCKET|std::env::var|systemctl|Command::new|UnixDatagram/);
  const usage = main.match(/"(usage: cyanrex-platform[^"\n]+)"/)?.[1];
  assert.ok(usage && Buffer.byteLength(`${usage}\n`) <= 256);
});

test("notification adapter captures one nonblocking Unix peer and sends only fixed lifecycle messages", async () => {
  const notify = await read(`${directory}/notify.rs`);
  const code = compact(notify);
  for (const value of ['std::env::var_os("NOTIFY_SOCKET")', "AF_UNIX", "SOCK_DGRAM",
    "SOCK_NONBLOCK", "SOCK_CLOEXEC", "connect_addr(", 'b"READY=1"', 'b"STOPPING=1"'])
    assert.ok(code.includes(value), value);
  assert.equal([...code.matchAll(/var_os\("NOTIFY_SOCKET"\)/g)].length, 1);
  assert.equal([...code.matchAll(/\.connect_addr\(/g)].length, 1);
  assert.equal([...code.matchAll(/\.send\(bytes\)/g)].length, 1);
  for (const value of ['bytes.contains(&0)', 'bytes.strip_prefix(b"@")', "abstract_name.is_empty()",
    "SocketAddr::from_abstract_name(", 'bytes.starts_with(b"/")', "SocketAddr::from_pathname("])
    assert.ok(code.includes(value), value);
  const enabled = code.indexOf("if!enabled");
  const environment = code.indexOf('std::env::var_os("NOTIFY_SOCKET")');
  assert.ok(enabled >= 0 && environment > enabled, "disabled mode must return before reading the environment");
  assert.match(code.slice(enabled, environment), /returnOk\(None\)/);
  assert.doesNotMatch(code, /env::vars|set_var|remove_var|PGOPTIONS|DATABASE_URL|std::fs|canonicalize|symlink_metadata|Command::|systemctl|sudo|WATCHDOG|MAINPID|FDSTORE|STATUS=|sleep\(|timeout\(|\.await|\.bind\(|\.send_to\(|sendmsg|derive\([^)]*(?:Debug|Serialize)/);
});

test("READY follows original binding and final process check but precedes human stdout", async () => {
  const source = await read(`${directory}/mod.rs`);
  const sequence = ["Notifier::from_environment(notify_systemd)", "config::load(path)",
    "open_platform_storage_expected(", "TcpListener::bind(", "process.verify_current_thread()",
    ".ready()", 'println!("platform loopback host ready")', "http::serve_notified("];
  let offset = -1;
  for (const value of sequence) {
    const found = source.indexOf(value, offset + 1);
    assert.ok(found >= 0 && found > offset, value);
    offset = found;
  }
  assert.ok(source.indexOf("verify_before_runtime()") < source.indexOf("Notifier::from_environment("),
    "notification setup follows an explicit process recheck");
  const setup = source.indexOf("Notifier::from_environment(");
  const after = source.indexOf("process.verify_before_runtime()", setup);
  const propagate = source.indexOf("let notifier = prepared?", setup);
  const config = source.indexOf("config::load(path)", setup);
  assert.ok(setup < after && after < propagate && propagate < config,
    "setup failure is rechecked before result propagation or private input");
});

test("STOPPING follows listener close and a failed notification still reaches bounded drain", async () => {
  const http = compact(await read(`${directory}/http.rs`));
  const sequence = ["drop(listener)", ".stopping()", "shutdown.send(true)",
    "tokio::time::timeout(DRAIN_TIMEOUT,", "connections.join_next().await"];
  let offset = -1;
  for (const value of sequence) {
    const found = http.indexOf(value, offset + 1);
    assert.ok(found >= 0 && found > offset, value);
    offset = found;
  }
  const stopping = http.slice(http.indexOf("drop(listener)"), http.indexOf("shutdown.send(true)"));
  assert.match(stopping, /failed/);
  assert.doesNotMatch(stopping, /return|\.stopping\(\)\?/);
  assert.equal([...http.matchAll(/tokio::time::timeout\(/g)].length, 1);
});

test("notification documentation separates manual integration, kernel send and service acceptance", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-host-notifications.md`);
    for (const value of ["issues/3", "--notify-systemd", "NOTIFY_SOCKET", "READY=1", "STOPPING=1",
      "SO_PASSCRED", "Type=notify", "NotifyAccess=main", "Restart=no", "SupplementaryGroups=",
      "platform_notify_defaults_tdd", "test-platform-loopback-tls-host.sh"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /not.*manager acknowledged/ : /不.*管理器.*确认/);
    assert.match(guide, locale === "en" ? /installs no unit/ : /不安装.*unit|不安装.*单元|未安装.*单元/);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-host-notifications.md`));
    for (const file of ["platform-loopback-host.md", "platform-loopback-tls-host.md", "testing-guide.md"])
      await assertDocumentationReference(`docs/${locale}/${file}`, `docs/${locale}/platform-host-notifications.md`);
  }
  assert.equal((await read(".github/workflows/ci.yml")).trimEnd().split("\n").length, 600,
    "extend the existing explicit runner instead of adding a duplicate CI step");
});

test("default notification probes use owned real IPC while SQL selections stay explicit", async () => {
  const target = await read("engine/tests/platform_notify_defaults_tdd.rs");
  assert.doesNotMatch(target, /#\[ignore/);
  assert.ok(target.includes('cfg(target_os = "linux")'));
  const cases = await read("engine/tests/platform_notify_defaults/cli.rs");
  assert.equal([...cases.matchAll(/#\[test\]\s*fn platform_notify_/g)].length, 4);
  for (const value of ["--notify-systemd=true", "check-process", "check-config", "check-storage",
    "Watch::new", "receiver.control()", "safe_ids()", "legacy::no_dial", "legacy::no_host"])
    assert.ok(cases.includes(value), value);
  const receiver = compact(await read("engine/tests/platform_notify_defaults/receiver.rs"));
  for (const value of ["SO_PASSCRED", "SCM_CREDENTIALS", "MSG_CMSG_CLOEXEC", "MSG_DONTWAIT",
    "MSG_TRUNC|libc::MSG_CTRUNC", "credentials.pid==pidas", "credentials.uid==uid", "credentials.gid==gid"])
    assert.ok(receiver.includes(value), value);
  const runner = await read("scripts/test-platform-loopback-tls-host.sh");
  for (const name of ["notify::postgres_platform_loopback_host_systemd_ready_then_login_stopping_and_clean_exit",
    "notify::postgres_platform_loopback_tls_host_systemd_ready_then_https_login_stopping_and_clean_exit",
    "notify::postgres_platform_loopback_tls_host_notify_startup_failure_silent_and_shutdown_send_failure_still_drains"])
    assert.equal(runner.split(name).length - 1, 1, name);
});
