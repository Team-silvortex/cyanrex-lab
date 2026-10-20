import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import test from "node:test";

const project = fileURLToPath(new URL("../../", import.meta.url));
const secrets = ["fixture-postgres-secret-canary", "fixture-admin-secret-canary", "FIXTURETOTPSECRETCANARYABCDEFGHIJK"];
const tool = `#!${process.execPath}
const fs = require("node:fs"), path = require("node:path");
const name = path.basename(process.argv[1]), args = process.argv.slice(2);
const runtime = Object.fromEntries(["INSTANCE_ID", "ENGINE_PORT", "FRONTEND_PORT", "POSTGRES_PORT", "BIND_ADDRESS", "COMPOSE_PROJECT", "POSTGRES_VOLUME_NAME"].map(key => [key, process.env["CYANREX_" + key]]));
fs.appendFileSync(process.env.START_TEST_CALLS, JSON.stringify({ name, args, runtime }) + "\\n");
if (name === "docker" || name === "docker-compose") {
  if (name === "docker" && args[0] === "compose" && args[1] === "version" && process.env.START_TEST_LEGACY === "1") process.exit(1);
  const composeArgs = name === "docker-compose" ? args : args[0] === "compose" ? args.slice(1) : [];
  const operation = composeArgs.find(value => ["version", "up", "down", "ps", "logs"].includes(value)) ?? args[0];
  const held = operation === "logs" && process.env.START_TEST_HOLD_LOGS === "1" ? "logs"
    : operation === "up" && process.env.START_TEST_HOLD_UP === "1" ? "up" : null;
  if (held) {
    const timer = setInterval(() => {
      if (fs.existsSync(path.join(process.env.START_TEST_ROOT, "release-" + held))) { clearInterval(timer); process.exit(0); }
    }, 10);
  }
} else if (name === "cargo") {
  fs.writeSync(2, "Fixture stops before any local Engine/frontend process can start.\\n");
  process.exit(73);
} else if (name === "npm" || name === "sudo" || name === "openssl") {
  fs.writeSync(2, "Forbidden fixture tool invocation: " + name + "\\n"); process.exit(91);
} else if (name === "uname") console.log(args[0] === "-s" ? "Linux" : "fixture-kernel");
else console.log("fixture-tool");
`;

async function fixture(t) {
  const root = await mkdtemp(path.join(tmpdir(), "cyanrex-start-operations-"));
  const children = [], callsFile = path.join(root, "calls.jsonl");
  await Promise.all(["scripts", "docker", "bin"].map(name => mkdir(path.join(root, name))));
  await copyFile(path.join(project, "start.sh"), path.join(root, "start.sh"));
  await copyFile(path.join(project, "scripts/start-lock.sh"), path.join(root, "scripts/start-lock.sh"));
  await copyFile(path.join(project, "scripts/debug-system.sh"), path.join(root, "scripts/debug-system.sh"));
  await copyFile(path.join(project, "scripts/runtime-config.sh"), path.join(root, "scripts/runtime-config.sh"));
  await writeFile(path.join(root, "docker/docker-compose.yml"), "services: {}\n");
  await writeFile(path.join(root, "docker/.env"), [
    `POSTGRES_PASSWORD=${secrets[0]}`, `CYANREX_ADMIN_PASSWORD=${secrets[1]}`,
    `CYANREX_ADMIN_TOTP_SECRET=${secrets[2]}`, "CYANREX_BIND_ADDRESS=127.0.0.1", "",
  ].join("\n"), { mode: 0o600 });
  for (const name of ["docker", "docker-compose", "cargo", "npm", "sudo", "openssl", "uname", "clang", "bpftool"]) {
    await writeFile(path.join(root, "bin", name), tool, { mode: 0o755 });
  }
  const environment = { ...process.env };
  for (const key of Object.keys(environment)) {
    if (key.startsWith("CYANREX_") || key.startsWith("START_TEST_")
      || key.startsWith("COMPOSE_") || key.startsWith("POSTGRES_") || key === "NEXT_PUBLIC_ENGINE_URL"
      || ["BASH_ENV", "ENV", "NODE_OPTIONS", "SHELLOPTS", "BASHOPTS", "SKIP_START_LOCK", "SKIP_CONFLICT_CHECK"].includes(key)) delete environment[key];
  }
  Object.assign(environment, { PATH: `${path.join(root, "bin")}${path.delimiter}${process.env.PATH}`,
    LC_ALL: "C", START_TEST_ROOT: root, START_TEST_CALLS: callsFile });
  const launch = (args, extra = {}) => {
    const child = spawn("bash", [path.join(root, "start.sh"), ...args], {
      cwd: root, env: { ...environment, ...extra }, detached: true, stdio: ["ignore", "pipe", "pipe"],
    });
    const running = { child, stdout: "", stderr: "", exited: false };
    child.stdout.on("data", chunk => { running.stdout += chunk; });
    child.stderr.on("data", chunk => { running.stderr += chunk; });
    running.done = new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", (code, signal) => { running.exited = true; resolve({ code, signal, stdout: running.stdout, stderr: running.stderr }); });
    });
    children.push(running);
    return running;
  };
  const completed = async running => {
    let timer;
    try {
      return await Promise.race([running.done, new Promise((_resolve, reject) => {
        timer = setTimeout(() => reject(new Error("Synthetic launcher did not finish within five seconds")), 5000);
      })]);
    } finally { clearTimeout(timer); }
  };
  const calls = async () => {
    try { return (await readFile(callsFile, "utf8")).trim().split("\n").filter(Boolean).map(line => JSON.parse(line)); }
    catch (error) { if (error.code === "ENOENT") return []; throw error; }
  };
  const waitFor = async predicate => {
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) { if (await predicate(await calls())) return; await delay(10); }
    assert.fail("Synthetic Docker operation was not observed");
  };
  t.after(async () => {
    await Promise.all(["logs", "up"].map(name => writeFile(path.join(root, `release-${name}`), "release\n")));
    for (const running of children) {
      if (running.exited) continue;
      for (const signal of ["SIGTERM", "SIGKILL"]) {
        try { process.kill(-running.child.pid, signal); } catch (error) { if (error.code !== "ESRCH") throw error; }
        await Promise.race([running.done, delay(200)]);
        if (running.exited) break;
      }
      await running.done;
    }
    await rm(root, { recursive: true, force: true });
  });
  return { root, launch, completed, calls, waitFor,
    configure: text => writeFile(path.join(root, "docker/.env"), text, { mode: 0o600 }),
    release: name => writeFile(path.join(root, `release-${name}`), "release\n") };
}

const operation = (call, name) => call.name === "docker" && call.args[0] === "compose" && call.args.includes(name);
function assertSuccess(result) { assert.equal(result.code, 0, result.stdout + result.stderr); }

test("logs without a service passes no empty service argument", async t => {
  const f = await fixture(t);
  assertSuccess(await f.completed(f.launch(["logs"])));
  const call = (await f.calls()).find(call => operation(call, "logs"));
  assert.ok(call);
  assert.deepEqual(call.args.slice(call.args.indexOf("logs")), ["logs", "-f"]);
});

test("logs retains one explicitly selected service without adding arguments", async t => {
  const f = await fixture(t);
  assertSuccess(await f.completed(f.launch(["logs", "engine", "--instance-id", "room-a"])));
  const call = (await f.calls()).find(call => operation(call, "logs"));
  assert.deepEqual(call.args.slice(call.args.indexOf("logs")), ["logs", "-f", "engine"]);
  assert.equal(call.args[call.args.indexOf("-p") + 1], "cyanrex-room-a");
});

test("status reads the selected instance without creating a startup lock", async t => {
  const f = await fixture(t);
  assertSuccess(await f.completed(f.launch(["status", "--instance-id", "room-a"])));
  const call = (await f.calls()).find(call => operation(call, "ps"));
  assert.equal(call.args[call.args.indexOf("-p") + 1], "cyanrex-room-a");
  await assert.rejects(readFile(path.join(f.root, ".run/start-locks/cyanrex-room-a.lock")), { code: "ENOENT" });
});

test("a following log stream does not prevent status or stop for the same instance", async t => {
  const f = await fixture(t);
  const logs = f.launch(["logs", "engine", "--instance-id", "room-a"], { START_TEST_HOLD_LOGS: "1" });
  await f.waitFor(calls => calls.some(call => operation(call, "logs")));
  try {
    assert.equal(logs.exited, false);
    assertSuccess(await f.completed(f.launch(["status", "--instance-id", "room-a"])));
    assertSuccess(await f.completed(f.launch(["stop", "--instance-id", "room-a"])));
    assert.equal(logs.exited, false);
    const calls = await f.calls();
    assert.equal(calls.filter(call => operation(call, "ps")).length, 1);
    assert.equal(calls.filter(call => operation(call, "down")).length, 1);
  } finally { await f.release("logs"); await f.completed(logs); }
});

test("a pending start still excludes another start or stop for the same instance", async t => {
  const f = await fixture(t), args = ["start", "--mode", "docker", "--instance-id", "room-a", "--skip-conflict-check"];
  const first = f.launch(args, { START_TEST_HOLD_UP: "1" });
  await f.waitFor(calls => calls.some(call => operation(call, "up")));
  try {
    const second = await f.completed(f.launch(args));
    assert.notEqual(second.code, 0);
    assert.match(second.stdout + second.stderr, /already in progress/);
    const stop = await f.completed(f.launch(["stop", "--instance-id", "room-a"]));
    assert.notEqual(stop.code, 0);
    assert.match(stop.stdout + stop.stderr, /already in progress/);
    assert.equal((await f.calls()).filter(call => operation(call, "up")).length, 1);
    assert.equal((await f.calls()).some(call => operation(call, "down")), false);
  } finally { await f.release("up"); assertSuccess(await f.completed(first)); }
});

test("Docker debug remains accepted without tracing synthetic credentials", async t => {
  const f = await fixture(t);
  for (const [args, env] of [[["--debug"], {}], [[], { CYANREX_DEBUG: "1" }]]) {
    const result = await f.completed(f.launch(["start", "--mode", "docker", "--skip-conflict-check", ...args], env));
    assertSuccess(result);
    for (const secret of secrets) assert.equal((result.stdout + result.stderr).includes(secret), false, "debug output disclosed a synthetic credential");
    assert.match(result.stdout + result.stderr, /debug/i);
  }
  assert.equal((await f.calls()).some(call => ["openssl", "sudo", "cargo", "npm"].includes(call.name)), false);
});

test("native debug cannot disclose credentials before a synthetic build refusal", async t => {
  const f = await fixture(t);
  const result = await f.completed(f.launch(["start", "--mode", "native", "--skip-conflict-check", "--debug"]));
  assert.equal(result.code, 73, result.stdout + result.stderr);
  assert.match(result.stderr, /Fixture stops before any local Engine/);
  for (const secret of secrets) assert.equal((result.stdout + result.stderr).includes(secret), false, "native debug output disclosed a synthetic credential");
  const calls = await f.calls();
  assert.equal(calls.filter(call => call.name === "cargo").length, 1);
  assert.equal(calls.some(call => ["sudo", "npm", "openssl"].includes(call.name)), false);
});

const configuration = (extra = []) => [
  `POSTGRES_PASSWORD=${secrets[0]}`, `CYANREX_ADMIN_PASSWORD=${secrets[1]}`,
  `CYANREX_ADMIN_TOTP_SECRET=${secrets[2]}`, "CYANREX_INSTANCE_ID=file-room",
  "CYANREX_ENGINE_PORT=18080", "CYANREX_FRONTEND_PORT=13000", "CYANREX_POSTGRES_PORT=15433",
  "CYANREX_BIND_ADDRESS=127.0.0.2",
  "CYANREX_POSTGRES_VOLUME_NAME=cyanrex-postgres-data-${CYANREX_INSTANCE_ID:-default}", ...extra, "",
].join("\n");

test("start, stop, status, logs and launcher diagnostics select the same literal file instance", async t => {
  const f = await fixture(t);
  await f.configure(configuration());
  for (const args of [["start", "--skip-conflict-check"], ["status"], ["logs"], ["stop"], ["diagnose"]]) {
    assertSuccess(await f.completed(f.launch(args)));
  }
  const calls = (await f.calls()).filter(call => call.name === "docker" && call.args.includes("-p"));
  assert.ok(calls.length >= 6);
  for (const call of calls) {
    assert.equal(call.args[call.args.indexOf("-p") + 1], "cyanrex-file-room");
    assert.equal(call.runtime.ENGINE_PORT, "18080");
    assert.equal(call.runtime.BIND_ADDRESS, "127.0.0.2");
  }
});

test("CLI selectors beat inherited environment and file values before volume expansion", async t => {
  const f = await fixture(t);
  await f.configure(configuration());
  assertSuccess(await f.completed(f.launch(["start", "--skip-conflict-check", "--instance-id", "cli-room",
    "--engine-port", "28080", "--frontend-port", "23000", "--postgres-port", "25432", "--bind-address", "127.0.0.3"],
  { CYANREX_INSTANCE_ID: "env-room", CYANREX_ENGINE_PORT: "38080", CYANREX_COMPOSE_PROJECT: "unrelated" })));
  const call = (await f.calls()).find(call => operation(call, "up"));
  assert.deepEqual(call.runtime, { INSTANCE_ID: "cli-room", ENGINE_PORT: "28080", FRONTEND_PORT: "23000",
    POSTGRES_PORT: "25432", BIND_ADDRESS: "127.0.0.3", COMPOSE_PROJECT: "cyanrex-cli-room", POSTGRES_VOLUME_NAME: "cyanrex-postgres-data-cli-room" });
  await readFile(path.join(f.root, ".run/start-locks/cyanrex-cli-room.lock"));
  await assert.rejects(readFile(path.join(f.root, ".run/start-locks/cyanrex-file-room.lock")), { code: "ENOENT" });
});

test("inherited selectors beat file values and select the matching default database volume", async t => {
  const f = await fixture(t);
  await f.configure(configuration());
  assertSuccess(await f.completed(f.launch(["start", "--skip-conflict-check"],
    { CYANREX_INSTANCE_ID: "env-room", CYANREX_ENGINE_PORT: "28080" })));
  const call = (await f.calls()).find(call => operation(call, "up"));
  assert.equal(call.runtime.INSTANCE_ID, "env-room");
  assert.equal(call.runtime.ENGINE_PORT, "28080");
  assert.equal(call.runtime.FRONTEND_PORT, "13000");
  assert.equal(call.runtime.POSTGRES_VOLUME_NAME, "cyanrex-postgres-data-env-room");
});

test("the final file-selected instance is locked before start and excluded from concurrent stop", async t => {
  const f = await fixture(t);
  await f.configure(configuration());
  const first = f.launch(["start", "--skip-conflict-check"], { START_TEST_HOLD_UP: "1" });
  await f.waitFor(calls => calls.some(call => operation(call, "up")));
  try {
    await readFile(path.join(f.root, ".run/start-locks/cyanrex-file-room.lock"));
    const result = await f.completed(f.launch(["stop"]));
    assert.notEqual(result.code, 0);
    assert.match(result.stdout + result.stderr, /already in progress/);
    assert.equal((await f.calls()).some(call => operation(call, "down")), false);
    assertSuccess(await f.completed(f.launch(["stop", "--instance-id", "other-room"])));
  } finally { await f.release("up"); assertSuccess(await f.completed(first)); }
});

test("read-only operations never execute shell statements or generate credentials from the file", async t => {
  const f = await fixture(t);
  await f.configure(configuration(["printf trap > unexpected-evaluation"]));
  for (const args of [["status"], ["logs"], ["stop"], ["diagnose"]]) assertSuccess(await f.completed(f.launch(args)));
  await assert.rejects(readFile(path.join(f.root, "unexpected-evaluation")), { code: "ENOENT" });
  assert.equal((await f.calls()).some(call => ["openssl", "sudo"].includes(call.name)
    || (["cargo", "npm"].includes(call.name) && call.args[0] !== "--version")), false);
});

test("quoted, exported and CRLF selectors are literal and agree with CLI diagnostic overrides", async t => {
  const f = await fixture(t);
  await f.configure(configuration([" export CYANREX_INSTANCE_ID='quoted-room' # selected", 'CYANREX_ENGINE_PORT="28080"']).replaceAll("\n", "\r\n"));
  assertSuccess(await f.completed(f.launch(["status"])));
  let call = (await f.calls()).find(call => operation(call, "ps"));
  assert.equal(call.args[call.args.indexOf("-p") + 1], "cyanrex-quoted-room");
  assert.equal(call.runtime.ENGINE_PORT, "28080");
  assertSuccess(await f.completed(f.launch(["diagnose", "--instance-id", "cli-room", "--engine-port", "38080"])));
  call = (await f.calls()).filter(call => operation(call, "ps")).at(-1);
  assert.equal(call.args[call.args.indexOf("-p") + 1], "cyanrex-cli-room");
  assert.equal(call.runtime.ENGINE_PORT, "38080");
});

test("invalid selected identities and ports fail before locks, Docker or shell evaluation", async t => {
  for (const line of ["CYANREX_INSTANCE_ID=room/a", "CYANREX_INSTANCE_ID=$(touch unexpected-evaluation)",
    "CYANREX_INSTANCE_ID=room\u0000a", "CYANREX_ENGINE_PORT=9999999999999999999999999", "CYANREX_ENGINE_PORT=0"]) {
    const f = await fixture(t);
    await f.configure(configuration([line]));
    const result = await f.completed(f.launch(["start", "--skip-conflict-check"]));
    assert.notEqual(result.code, 0);
    assert.deepEqual(await f.calls(), []);
    await assert.rejects(readFile(path.join(f.root, "unexpected-evaluation")), { code: "ENOENT" });
    await assert.rejects(readFile(path.join(f.root, ".run/start-locks/cyanrex-file-room.lock")), { code: "ENOENT" });
    assert.doesNotMatch(result.stdout + result.stderr, /integer expression expected/);
  }
});

test("an indirect target change in trusted startup settings is rejected instead of changing the locked instance", async t => {
  const f = await fixture(t);
  await f.configure(configuration(['target_key=CYANREX_INSTANCE_ID', 'export "$target_key=drift-room"']));
  const result = await f.completed(f.launch(["start", "--skip-conflict-check"]));
  assert.notEqual(result.code, 0);
  assert.match(result.stderr, /changed the locked target/);
  assert.equal((await f.calls()).some(call => operation(call, "up")), false);
  await readFile(path.join(f.root, ".run/start-locks/cyanrex-file-room.lock"));
  await assert.rejects(readFile(path.join(f.root, ".run/start-locks/cyanrex-drift-room.lock")), { code: "ENOENT" });
});

test("observations without a configuration file use defaults without creating credentials or locks", async t => {
  const f = await fixture(t);
  await rm(path.join(f.root, "docker/.env"));
  for (const args of [["status"], ["logs"], ["diagnose"]]) {
    const result = await f.completed(f.launch(args));
    assertSuccess(result);
  }
  await assert.rejects(readFile(path.join(f.root, "docker/.env")), { code: "ENOENT" });
  await assert.rejects(readFile(path.join(f.root, ".run/start-locks/cyanrex-default.lock")), { code: "ENOENT" });
  for (const call of (await f.calls()).filter(call => call.name === "docker" && call.args.includes("-p"))) {
    assert.equal(call.args[call.args.indexOf("-p") + 1], "cyanrex-default");
    assert.equal(call.args[call.args.indexOf("--env-file") + 1], "/dev/null");
  }
});

test("an explicitly empty bind address is refused before selecting a different diagnostic fallback", async t => {
  const f = await fixture(t);
  await f.configure(configuration());
  for (const action of ["start", "diagnose"]) {
    const result = await f.completed(f.launch([action, "--bind-address", ""]));
    assert.notEqual(result.code, 0);
    assert.match(result.stderr, /bind-address must be a literal address/);
  }
  assert.deepEqual(await f.calls(), []);
  await assert.rejects(readFile(path.join(f.root, ".run/start-locks/cyanrex-file-room.lock")), { code: "ENOENT" });
});

test("plugin and legacy Compose receive the exact absolute configuration file for every action", async t => {
  for (const legacy of ["0", "1"]) {
    const f = await fixture(t);
    await f.configure(configuration());
    for (const args of [["start", "--skip-conflict-check"], ["status"], ["logs"], ["stop"], ["diagnose"]]) {
      assertSuccess(await f.completed(f.launch(args, { START_TEST_LEGACY: legacy, COMPOSE_ENV_FILES: "/unrelated/config.env" })));
    }
    const calls = (await f.calls()).filter(call => call.args.includes("-p"));
    assert.ok(calls.length >= 6);
    for (const call of calls) {
      assert.equal(call.name, legacy === "1" ? "docker-compose" : "docker");
      const index = call.args.indexOf("--env-file");
      assert.ok(index >= 0 && index < call.args.indexOf("-p"));
      assert.equal(call.args[index + 1], path.join(f.root, "docker/.env"));
    }
  }
});

test("every launcher action accepts the same explicit coordinate overrides", async t => {
  const f = await fixture(t);
  await f.configure(configuration());
  const coordinates = ["--instance-id", "cli-room", "--engine-port", "28080", "--frontend-port", "23000",
    "--postgres-port", "25432", "--bind-address", "127.0.0.3"];
  for (const action of ["start", "status", "logs", "stop", "diagnose"]) {
    assertSuccess(await f.completed(f.launch([action, ...coordinates])));
  }
  for (const call of (await f.calls()).filter(call => call.args.includes("-p"))) {
    assert.equal(call.runtime.INSTANCE_ID, "cli-room");
    assert.equal(call.runtime.ENGINE_PORT, "28080");
    assert.equal(call.runtime.FRONTEND_PORT, "23000");
    assert.equal(call.runtime.POSTGRES_PORT, "25432");
    assert.equal(call.runtime.BIND_ADDRESS, "127.0.0.3");
  }
});
