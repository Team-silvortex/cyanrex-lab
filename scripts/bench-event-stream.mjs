#!/usr/bin/env node
// Serial pressure checks against the real WebSocket handler on ephemeral loopback listeners.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, appendFile, rm } from "node:fs/promises";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import os from "node:os";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = process.argv[2] && path.resolve(process.argv[2]);
if (!output || process.argv.length !== 3) throw Error("Usage: node scripts/bench-event-stream.mjs <new-output-directory>");
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output);
const env = { ...process.env };
delete env.DATABASE_URL;
delete env.CYANREX_BENCH_DATABASE_URL;
const run = promisify(execFile);
const command = async (program, args, extra = {}) => (await run(program, args, {
  cwd: root, env: { ...env, ...extra }, timeout: 180000, maxBuffer: 8 * 1024 * 1024,
})).stdout.trim();
const metadata = {
  started_at: new Date().toISOString(), complete: false, source_commit: await command("git", ["rev-parse", "HEAD"]),
  source_status: await command("git", ["status", "--porcelain", "--", "engine"]), source_sha256: {},
  rustc: await command("rustc", ["--version"]), cpu: os.cpus()[0]?.model, kernel: os.release(),
  tokio_workers: 16, build: "release test executable (cfg(test))", repeats: 3,
  scope: "Real WebSocket handler with raw global broadcast and loopback TCP. No EventBus histories, DB, auth, browser, kernel, or Agent. Paced case is a delivery check, not maximum throughput.",
};
const sources = (await command("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "engine/src", "engine/Cargo.toml", "engine/Cargo.lock"]))
  .split("\0").filter(Boolean).sort();
for (const relative of [...sources, "scripts/bench-event-stream.mjs"]) {
  metadata.source_sha256[relative] = createHash("sha256").update(await readFile(path.join(root, relative))).digest("hex");
}
await writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
const build = await command("cargo", ["test", "--manifest-path", "engine/Cargo.toml", "--release", "--locked", "--offline", "--lib", "--no-run", "--message-format=json"]);
const binary = build.split("\n").map(line => JSON.parse(line)).find(row =>
  row.reason === "compiler-artifact" && row.target.kind.includes("lib") && row.profile.test && row.executable)?.executable;
if (!binary) throw Error("Cargo did not report a library test executable");
metadata.binary_sha256 = createHash("sha256").update(await readFile(binary)).digest("hex");
const resourceFile = path.join(output, "resource.tmp.json");
const all = [];
for (let round = 0; round < 3; round++) {
  const cases = ["paced", "burst", "stalled"];
  for (const kind of [...cases.slice(round), ...cases.slice(0, round)]) {
    const stdout = await command("/usr/bin/time", ["-f",
      '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M}', "-o", resourceFile,
      binary, "--ignored", "--exact", "routes::events::stream::pressure_tests::measure", "--nocapture"],
    { CYANREX_WS_BENCH_KIND: kind });
    const match = stdout.match(/^CYANREX_WS_BENCH=(\{.*\})$/m);
    if (!match) throw Error("Missing pressure-check result");
    const row = { case: kind, round: round + 1, result: JSON.parse(match[1]),
      resource: JSON.parse(await readFile(resourceFile, "utf8")), loadavg: os.loadavg() };
    all.push(row);
    await appendFile(path.join(output, "runs.jsonl"), JSON.stringify(row) + "\n");
    console.log(`[ws pressure ${round + 1}/3] ${kind}: ${JSON.stringify(row.result)}`);
  }
}
await writeFile(path.join(output, "summary.json"), JSON.stringify(all, null, 2) + "\n");
metadata.complete = true;
metadata.finished_at = new Date().toISOString();
await writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
await rm(resourceFile);
console.log(`[ws pressure] ${all.length} checks passed: ${output}`);
