import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { auditProductionDependencies, executeNpmAudit } from "../check-npm-audit.mjs";

const report = (counts = {}, status = 0) => ({
  status,
  stdout: JSON.stringify({
    auditReportVersion: 2,
    vulnerabilities: {},
    metadata: { vulnerabilities: { info: 0, low: 0, moderate: 0, high: 0, critical: 0,
      total: Object.values(counts).reduce((sum, count) => sum + count, 0), ...counts } },
  }),
  stderr: "",
});
const failure = (code, message = "registry request failed") => ({
  status: 1, stdout: JSON.stringify({ error: { code, summary: message } }), stderr: "",
});

async function simulate(results) {
  let calls = 0;
  const sleeps = [];
  const logs = [];
  const result = await auditProductionDependencies("/project/frontend", {
    execute: async (directory) => {
      assert.equal(directory, "/project/frontend");
      return results[Math.min(calls++, results.length - 1)];
    },
    sleep: async (ms) => sleeps.push(ms),
    log: (message) => logs.push(message),
  });
  return { result, calls, sleeps, logs };
}

test("npm audit retains production scope, moderate threshold, and finite timeouts", async () => {
  const result = await executeNpmAudit("/project/frontend", async (command, args, options) => {
    assert.equal(command, "npm");
    assert.deepEqual(args, ["audit", "--omit=dev", "--audit-level=moderate", "--json",
      "--offline=false", "--fetch-retries=0", "--fetch-timeout=20000"]);
    assert.equal(options.cwd, "/project/frontend");
    assert.equal(options.timeout, 60_000);
    assert.equal(options.killSignal, "SIGKILL");
    return { stdout: report().stdout, stderr: "" };
  });
  assert.equal(result.status, 0);
});

test("valid clean and below-threshold audit reports pass without retrying", async () => {
  for (const counts of [{}, { low: 1 }]) {
    const { result, calls, sleeps } = await simulate([report(counts)]);
    assert.equal(result.status, 0);
    assert.equal(calls, 1);
    assert.deepEqual(sleeps, []);
  }
});

test("vulnerability reports fail immediately, including misleading transient error text", async () => {
  for (const severity of ["moderate", "high", "critical"]) {
    const vulnerable = { ...report({ [severity]: 1 }, 1), stderr: "ECONNRESET E503" };
    const { result, calls } = await simulate([vulnerable, report()]);
    assert.equal(result.status, 1);
    assert.equal(calls, 1);
  }
  assert.equal((await simulate([report({ high: 1 }, 0)])).result.status, 1);
});

test("network failures, rate limits, and server errors retry a fresh audit", async () => {
  for (const code of ["ECONNRESET", "ECONNREFUSED", "ETIMEDOUT", "EAI_AGAIN", "E429", "E503"]) {
    const { result, calls, sleeps, logs } = await simulate([failure(code), report()]);
    assert.equal(result.status, 0, code);
    assert.equal(calls, 2, code);
    assert.deepEqual(sleeps, [1_000]);
    assert.match(logs.join("\n"), /retry/i);
  }
});

test("npm 9 message-only socket failures are recognized", async () => {
  const network = { status: 1, stdout: JSON.stringify({
    message: "Client network socket disconnected before secure TLS connection was established",
  }), stderr: "npm ERR! audit endpoint returned an error" };
  assert.equal((await simulate([network, report()])).calls, 2);
});

test("retired Quick fallback retries Bulk through npm without accepting the failed report", async () => {
  const retired = {
    status: 1,
    stdout: JSON.stringify({ statusCode: 400, message: "Invalid package tree" }),
    stderr: "npm notice This endpoint is being retired. Use the bulk advisory endpoint instead.\n"
      + "npm WARN audit 400 Bad Request - POST https://registry.npmjs.org/-/npm/v1/security/audits/quick",
  };
  const { result, calls } = await simulate([retired, report()]);
  assert.equal(result.status, 0);
  assert.equal(calls, 2);
});

test("persistent transient failures stop after three attempts and stay failed", async () => {
  const { result, calls, sleeps, logs } = await simulate([failure("ECONNRESET")]);
  assert.notEqual(result.status, 0);
  assert.equal(calls, 3);
  assert.deepEqual(sleeps, [1_000, 2_000]);
  assert.match(logs.join("\n"), /3 attempts/i);
});

test("authentication, certificate, lockfile, and unknown errors never retry", async () => {
  for (const code of ["E401", "E403", "E400", "ENOLOCK", "EUSAGE", "CERT_HAS_EXPIRED",
    "SELF_SIGNED_CERT_IN_CHAIN", "SOMETHING_NEW"]) {
    const { result, calls } = await simulate([failure(code), report()]);
    assert.notEqual(result.status, 0, code);
    assert.equal(calls, 1, code);
  }
  assert.equal((await simulate([failure("E401", "Authentication failed after E503"), report()])).calls, 1);
  assert.equal((await simulate([failure("CERT_HAS_EXPIRED", "ECONNRESET"), report()])).calls, 1);
});

test("missing or malformed reports cannot pass even if npm exits zero", async () => {
  for (const stdout of ["", "not JSON", "{}", '{"auditReportVersion":2}',
    JSON.stringify({ auditReportVersion: 2, vulnerabilities: {}, metadata: {
      vulnerabilities: { info: 0, low: 0, moderate: -1, high: 0, critical: 0, total: 0 },
    } })]) {
    const { result, calls } = await simulate([{ status: 0, stdout, stderr: "" }, report()]);
    assert.notEqual(result.status, 0);
    assert.equal(calls, 1);
  }
});

test("process launch failures fail closed and timeouts remain bounded", async () => {
  for (const error of [Object.assign(new Error("npm missing"), { code: "ENOENT" }),
    Object.assign(new Error("timed out"), { killed: true, signal: "SIGKILL" })]) {
    const execution = await executeNpmAudit("/project/frontend", async () => { throw error; });
    const { result, calls } = await simulate([execution]);
    assert.notEqual(result.status, 0);
    assert.equal(calls, error.killed ? 3 : 1);
  }
});

test("real npm CLI retries Bulk, including a retired Quick fallback when used", async () => {
  const fixture = await mkdtemp(path.join(os.tmpdir(), "cyanrex-npm-audit-"));
  const requests = [];
  const bulk = "/-/npm/v1/security/advisories/bulk";
  const quick = "/-/npm/v1/security/audits/quick";
  const server = createServer((request, response) => {
    requests.push(request.url);
    request.resume();
    response.setHeader("content-type", "application/json");
    if (request.url === bulk) {
      response.statusCode = requests.filter((url) => url === bulk).length === 1 ? 503 : 200;
      response.end("{}");
    } else if (request.url === quick) {
      response.statusCode = 400;
      response.setHeader("npm-notice", "This endpoint is being retired. Use the bulk advisory endpoint instead.");
      response.end(JSON.stringify({ statusCode: 400, message: "Invalid package tree" }));
    } else {
      response.statusCode = 404;
      response.end("{}");
    }
  });
  try {
    const manifest = { name: "audit-fixture", version: "1.0.0", private: true,
      dependencies: { "audit-fixture-dependency": "1.0.0" } };
    await writeFile(path.join(fixture, "package.json"), JSON.stringify(manifest));
    await writeFile(path.join(fixture, "package-lock.json"), JSON.stringify({
      name: manifest.name, version: manifest.version, lockfileVersion: 3, requires: true,
      packages: { "": manifest, "node_modules/audit-fixture-dependency": { version: "1.0.0" } },
    }));
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", resolve);
    });
    const registry = `http://127.0.0.1:${server.address().port}`;
    const script = fileURLToPath(new URL("../check-npm-audit.mjs", import.meta.url));
    const result = await promisify(execFile)(process.execPath, [script, fixture], {
      timeout: 15_000,
      env: { ...process.env, npm_config_registry: registry, npm_config_audit_registry: registry,
        npm_config_update_notifier: "false", npm_config_noproxy: "127.0.0.1",
        npm_config_cache: path.join(fixture, "cache"), npm_config_userconfig: path.join(fixture, ".npmrc"),
        npm_config_globalconfig: path.join(fixture, "global.npmrc") },
    });
    assert.equal(JSON.parse(result.stdout).metadata.vulnerabilities.total, 0);
    assert.match(result.stderr, /retrying a fresh audit/);
    assert.deepEqual(requests, requests.includes(quick) ? [bulk, quick, bulk] : [bulk, bulk]);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
    await rm(fixture, { recursive: true, force: true });
  }
});

test("local and CI frontend/SDK audits share the same fail-closed wrapper", async () => {
  const gate = await readFile(new URL("../quality-gate.sh", import.meta.url), "utf8");
  const workflow = await readFile(new URL("../../.github/workflows/ci.yml", import.meta.url), "utf8");
  for (const project of ["frontend", "sdk-js"]) {
    assert.ok(gate.includes(`node "$PROJECT_ROOT/scripts/check-npm-audit.mjs" "$PROJECT_ROOT/${project}"`));
  }
  assert.equal(workflow.match(/node \.\.\/scripts\/check-npm-audit\.mjs \./g)?.length, 2);
  assert.doesNotMatch(gate + workflow, /\baudit --omit=dev/);
});
