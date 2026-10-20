import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

const execute = promisify(execFile);
const source = new URL("../debug-system.sh", import.meta.url);
const realCurl = ["/usr/bin/curl", "/bin/curl"].find(existsSync);

function fixture(t, configuration) {
  const root = mkdtempSync(path.join(tmpdir(), "cyanrex-debug-system-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const name of ["scripts", "docker", "bin"]) mkdirSync(path.join(root, name));
  copyFileSync(source, path.join(root, "scripts/debug-system.sh"));
  copyFileSync(new URL("../runtime-config.sh", import.meta.url), path.join(root, "scripts/runtime-config.sh"));
  writeFileSync(path.join(root, "docker/docker-compose.yml"), "services: {}\n");
  if (configuration !== undefined) writeFileSync(path.join(root, "docker/.env"), configuration, { mode: 0o600 });
  for (const name of ["dirname", "date", "head", "grep", "tail", "cut", "tr", "ls", "awk"]) {
    const binary = ["/usr/bin", "/bin"].map(directory => path.join(directory, name)).find(existsSync);
    assert.ok(binary, `${name} is a required shell utility`);
    symlinkSync(binary, path.join(root, "bin", name));
  }
  const tool = (name, body) => writeFileSync(path.join(root, "bin", name), `#!/bin/bash\nset -eu\n${body}\n`, { mode: 0o700 });
  tool("docker", `case "\${1:-}:\${2:-}" in
    --version:) printf 'synthetic Docker\\n' ;;
    compose:version) exit 0 ;;
    compose:*) exit "\${TEST_COMPOSE_STATUS:-0}" ;;
    *) exit 99 ;;
  esac`);
  tool("curl", `printf 'CALL\\0' >> "\$TEST_CURL_ARGS"
    printf '%s\\0' "\$@" >> "\$TEST_CURL_ARGS"
    printf 'END\\0' >> "\$TEST_CURL_ARGS"
    if [ -n "\${TEST_REAL_CURL:-}" ]; then exec "\$TEST_REAL_CURL" "\$@"; fi
    printf '{"status":"ok"}\\n'
    exit "\${TEST_CURL_STATUS:-0}"`);
  tool("uname", "printf 'synthetic test kernel\\n'");
  tool("ss", "exit 1");
  const log = path.join(root, "curl-args");
  return {
    root,
    tool,
    args: () => existsSync(log) ? readFileSync(log, "utf8").split("\0").filter(Boolean) : [],
    run: async (overrides = {}) => {
      let result;
      try {
        result = await execute("/bin/bash", [path.join(root, "scripts/debug-system.sh")], {
          cwd: root, env: { PATH: path.join(root, "bin"), LANG: "C", TEST_CURL_ARGS: log, ...overrides },
          timeout: 20_000, maxBuffer: 128 * 1024,
        });
      } catch (error) { result = { ...error, stdout: error.stdout ?? "", stderr: error.stderr ?? "" }; }
      return { ...result, output: result.stdout + result.stderr };
    },
  };
}

function completed(result) {
  assert.equal(result.code, undefined, result.output);
  assert.match(result.stdout, /Diagnostics completed\./);
}

test("diagnostics finish with missing or partial optional configuration and never evaluate private shell text", async t => {
  for (const configuration of [undefined, "", "POSTGRES_PASSWORD=synthetic-never-print\nCYANREX_BIND_ADDRESS=127.0.0.1\nprintf trap > unexpected-evaluation\n"]) {
    const f = fixture(t, configuration), result = await f.run();
    completed(result);
    assert.doesNotMatch(result.output, /synthetic-never-print|printf trap/);
    assert.equal(existsSync(path.join(f.root, "unexpected-evaluation")), false);
    assert.match(result.output, /engine port 127\.0\.0\.1:8080/);
    if (configuration !== undefined) assert.match(result.output, /CYANREX_INSTANCE_ID=undefined/);
  }
});

test("optional configuration lookup is exact and retains the final literal value without shell expansion", async t => {
  const f = fixture(t, "CYANREX_INSTANCE_ID=old\nCYANREX_INSTANCE_ID=new\nCYANREX_COMPOSE_PROJECT=project=literal\nCYANREX_ENGINE_PORT=18080\nOTHER_CYANREX_FRONTEND_PORT=1\nCYANREX_BIND_ADDRESS=127.0.0.1\n");
  const result = await f.run(); completed(result);
  assert.match(result.output, /instance 'new' \(project 'cyanrex-new'\)/);
  assert.match(result.output, /CYANREX_COMPOSE_PROJECT=project=literal/);
  assert.match(result.output, /frontend port 127\.0\.0\.1:3000/);
  assert.ok(f.args().includes("http://127.0.0.1:18080/health"));
});

test("a failed diagnostic command warns and continues with one bounded configuration-independent health request", async t => {
  const f = fixture(t, ""), result = await f.run({ TEST_COMPOSE_STATUS: "7", TEST_CURL_STATUS: "28" });
  completed(result);
  assert.match(result.stderr, /command failed/);
  const args = f.args();
  assert.equal(args.filter(value => value === "CALL").length, 1);
  assert.equal(args[1], "-q", "ignore per-user curl configuration before parsing other options");
  for (const [option, value] of [["--connect-timeout", "3"], ["--max-time", "10"], ["--noproxy", "*"]]) {
    assert.equal(args[args.indexOf(option) + 1], value, option);
  }
  assert.equal(args.at(-2), "http://127.0.0.1:8080/health");
});

test("invalid Engine ports cannot change the health target or trigger a network request", async t => {
  for (const port of ["8080@other.invalid", "8080/path", "80 80", "0", "65536", "999999999999999999999999999999999"]) {
    const f = fixture(t, `CYANREX_ENGINE_PORT=${port}\n`), result = await f.run();
    completed(result);
    assert.deepEqual(f.args(), [], port);
    assert.match(result.stderr, /invalid.*(?:port|health)|health.*invalid/i);
  }
});

test("valid runtime overrides remain selected and a missing optional curl does not stop diagnostics", async t => {
  for (const port of ["1", "65535", "0008080"]) {
    const f = fixture(t, "CYANREX_ENGINE_PORT=18080\n"), result = await f.run({ CYANREX_ENGINE_PORT: port });
    completed(result);
    assert.ok(f.args().includes(`http://127.0.0.1:${port}/health`));
  }
  const f = fixture(t, "");
  rmSync(path.join(f.root, "bin/curl"));
  const result = await f.run(); completed(result);
  assert.match(result.stderr, /curl not installed/);
  assert.deepEqual(f.args(), []);
});

test("health checking times out against an actual loopback server that never sends headers", { skip: !realCurl, timeout: 24_000 }, async t => {
  const requests = [];
  let receivedAt;
  const server = createServer(request => {
    receivedAt = performance.now();
    requests.push({ method: request.method, url: request.url });
  });
  // The fixture also bounds the old unconfigured curl, so Red leaves no orphaned probe.
  server.setTimeout(16_000, socket => socket.destroy());
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(async () => {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  });
  const f = fixture(t);
  const result = await f.run({ TEST_REAL_CURL: realCurl, CYANREX_ENGINE_PORT: String(server.address().port),
    ALL_PROXY: "http://127.0.0.1:1", http_proxy: "http://127.0.0.1:1" });
  completed(result);
  assert.deepEqual(requests, [{ method: "GET", url: "/health" }]);
  assert.ok(performance.now() - receivedAt < 13_000, "the real HTTP wait ends within its ten-second budget plus scheduling margin");
  assert.match(result.stderr, /command failed/);
});
