import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const root = fileURLToPath(new URL("../../", import.meta.url));
const run = promisify(execFile);
const runner = "scripts/test-platform-browser-container.sh";
const entry = "scripts/platform-browser-container-entry.sh";

async function fixture(action) {
  const directory = await mkdtemp(path.join(os.tmpdir(), "cyanrex-browser-container-contract-"));
  try {
    const repo = path.join(directory, "repo");
    const tools = path.join(directory, "tools");
    for (const child of ["scripts", "engine/target/debug/deps"]) {
      await mkdir(path.join(repo, child), { recursive: true });
    }
    await copyFile(path.join(root, runner), path.join(repo, runner));
    await copyFile(path.join(root, entry), path.join(repo, entry));
    const session = path.join(repo, "engine/target/debug/deps/platform_browser_session_tdd-abcdef1234");
    const body = path.join(repo, "engine/target/debug/deps/platform_browser_task_body_tdd-abcdef1234");
    const node = path.join(tools, "node-v22.23.3-linux-x64");
    const chrome = path.join(tools, "chrome");
    const playwright = path.join(tools, "node_modules/playwright");
    for (const child of ["node-v22.23.3-linux-x64/bin", "chrome/locales", "node_modules/playwright", "node_modules/playwright-core", "bin"]) {
      await mkdir(path.join(tools, child), { recursive: true });
    }
    for (const file of [session, body, path.join(repo, "engine/target/debug/cyanrex-platform"),
      path.join(node, "bin/node"), path.join(chrome, "chrome")]) {
      await writeFile(file, "#!/bin/sh\nexit 0\n");
      await chmod(file, 0o755);
    }
    await writeFile(path.join(playwright, "index.mjs"), "export {};\n");
    await writeFile(path.join(chrome, "resources.pak"), "fixture\n");
    await writeFile(path.join(playwright, "package.json"), '{"name":"playwright"}\n');
    await writeFile(path.join(tools, "node_modules/playwright-core/package.json"), '{"name":"playwright-core"}\n');
    const log = path.join(directory, "docker-arguments");
    const docker = path.join(tools, "bin/docker");
    await writeFile(docker, `#!/bin/bash
printf '%s\\0' "$@" >> "$MOCK_DOCKER_LOG"
printf '\\n' >> "$MOCK_DOCKER_LOG"
if [[ "$1 $2" == "image inspect" ]]; then exit "\${MOCK_IMAGE_EXIT:-0}"; fi
if [[ "$1" == "run" ]]; then exit "\${MOCK_RUN_EXIT:-0}"; fi
exit 93
`);
    await chmod(docker, 0o755);
    const args = ["--image", "cyanrex-browser-contract:local", "--node-dir", node,
      "--chrome-dir", chrome, "--playwright-dir", playwright, "--session-test", session, "--body-test", body];
    const execute = (extra = [], environment = {}) => run("bash", [path.join(repo, runner), ...args, ...extra], {
      env: { ...process.env, PATH: `${path.join(tools, "bin")}:${process.env.PATH}`,
        MOCK_DOCKER_LOG: log, ...environment }, timeout: 10_000,
    });
    const calls = async () => (await readFile(log, "utf8")).trimEnd().split("\n")
      .map(line => line.split("\0").filter(Boolean));
    await action({ directory, repo, node, chrome, playwright, session, body, log, args, execute, calls });
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

test("browser test image is runtime-only and uses its existing real non-root account", async () => {
  const source = await readFile(path.join(root, "docker/platform-browser-tests.Dockerfile"), "utf8");
  assert.match(source, /^FROM ubuntu:24\.04$/m);
  assert.match(source, /^USER ubuntu:ubuntu$/m);
  assert.match(source, /^WORKDIR \/home\/ubuntu$/m);
  assert.match(source, /postgresql-16/);
  assert.match(source, /libnss3-tools/);
  assert.match(source, /Acquire::Retries=0/);
  assert.match(source, /Acquire::http::Timeout=30/);
  assert.match(source, /APT::Update::Error-Mode=any/);
  assert.match(source, /create_main_cluster = false/);
  assert.doesNotMatch(source, /(?:ENV|ARG)\s+HOME|user(?:add|del|mod)|cargo build|FROM (?:rust|mcr\.)/);
});

test("container preflight uses only exact read-only sources and bounded private runtime", async () => {
  await fixture(async f => {
    await f.execute();
    const [inspect, args] = await f.calls();
    assert.deepEqual(inspect, ["image", "inspect", "cyanrex-browser-contract:local"]);
    for (const pair of [["--network", "none"], ["--cap-drop", "ALL"],
      ["--security-opt", "no-new-privileges"], ["--pull", "never"], ["--user", "ubuntu:ubuntu"]]) {
      assert.equal(args[args.indexOf(pair[0]) + 1], pair[1]);
    }
    assert.ok(args.includes("--rm") && args.includes("--init") && args.includes("--read-only"));
    for (const flag of ["--memory", "--pids-limit", "--cpus", "--shm-size", "--tmpfs"]) {
      assert.ok(args.includes(flag));
    }
    const mounts = args.flatMap((value, index) => value === "--mount" ? [args[index + 1]] : []);
    assert.equal(mounts.length, 5);
    for (const mount of mounts) assert.match(mount, /^type=bind,src=.+,dst=.+,readonly$/);
    assert.ok(mounts.includes(`type=bind,src=${f.repo},dst=${f.repo},readonly`));
    assert.ok(mounts.some(value => value.includes("/node_modules/playwright-core,")));
    assert.equal(args.at(-4), "preflight");
    assert.doesNotMatch(args.join("\n"), /HOME=|DATABASE_URL=|docker\.sock|\.pki|\/sys\//);
    for (const flag of ["--privileged", "--pid", "--publish", "--volume", "--host"]) {
      assert.ok(!args.includes(flag));
    }
  });
});

test("real TLS execution is an explicit mode, never a default preflight claim", async () => {
  await fixture(async f => {
    await f.execute(["--run-tls"]);
    const args = (await f.calls())[1];
    assert.equal(args.at(-4), "tls");
  });
});

test("missing browser tools fail before contacting Docker", async () => {
  await fixture(async f => {
    await rm(path.join(f.chrome, "chrome"));
    await assert.rejects(f.execute(), /explicit executable/);
    await assert.rejects(readFile(f.log), { code: "ENOENT" });
  });
});

test("a missing explicit image is not pulled, built or replaced", async () => {
  await fixture(async f => {
    await assert.rejects(f.execute([], { MOCK_IMAGE_EXIT: "1" }));
    const calls = await f.calls();
    assert.equal(calls.length, 1);
    assert.equal(calls[0][0], "image");
  });
});

test("invalid or broad paths cannot become Docker mounts or executable selectors", async () => {
  await fixture(async f => {
    for (const extra of [["--node-dir", "/"], ["--chrome-dir", `${f.chrome},readonly=false`],
      ["--session-test", f.body], ["--body-test", "/bin/true"], ["--unknown"]]) {
      await assert.rejects(f.execute(extra));
    }
    await assert.rejects(readFile(f.log), { code: "ENOENT" });
  });
});

test("Docker execution failure remains a failure without retry or trust fallback", async () => {
  await fixture(async f => {
    await assert.rejects(f.execute([], { MOCK_RUN_EXIT: "47" }), { code: 47 });
    assert.equal((await f.calls()).length, 2);
  });
});

test("private entry retains exact TLS selectors, native identity and explicit SQL cleanup", async () => {
  const source = await readFile(path.join(root, entry), "utf8");
  assert.match(source, /getent passwd/);
  assert.match(source, /CapPrm:.*CapEff:.*CapInh:.*CapAmb:.*CapBnd:/);
  assert.match(source, /NoNewPrivs:/);
  assert.match(source, /\[\[.*HOME.*account_home/);
  assert.match(source, /\.pki\/nssdb/);
  assert.match(source, /tls::postgres_platform_browser_tls_private_ca_real_cookie_session_and_storage/);
  assert.match(source, /behavior::postgres_platform_browser_task_body_tls_confirmed_replace_conflict_and_revocation/);
  assert.match(source, /--ignored --exact/);
  assert.match(source, /pg_ctl.*-m fast -w stop/);
  assert.match(source, /trap cleanup EXIT/);
  assert.match(source, /backend_type = 'client backend' AND pid <> pg_backend_pid\(\)/);
  assert.match(source, /browser_entry_remaining.*== 0:1:0/);
  assert.match(source, /original TLS selectors passed/);
  assert.doesNotMatch(source, /(?:^|\s)(?:export\s+)?HOME=|ignoreHTTPSErrors|ignore-certificate|NODE_TLS_REJECT_UNAUTHORIZED|docker compose|--no-check-certificate|CYANREX_DB_FALLBACK/m);
});
