#!/usr/bin/env node
// Serial current-path A/B, optionally followed by an unchanged no-subscriber binary control.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, appendFile, rm } from "node:fs/promises";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import os from "node:os";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = process.argv[2] && path.resolve(process.argv[2]);
const baseline = process.argv[3] && path.resolve(process.argv[3]);
if (!output || ![3, 4].includes(process.argv.length)) {
  throw Error("Usage: node scripts/bench-event-fanout.mjs <new-output-directory> [baseline-mainline-binary]");
}
const env = { ...process.env, TOKIO_WORKER_THREADS: "16" };
delete env.DATABASE_URL;
delete env.CYANREX_BENCH_DATABASE_URL;
const run = promisify(execFile);
const command = async (program, args, extra = {}) => (await run(program, args, {
  cwd: root, env: { ...env, ...extra }, timeout: 180000, maxBuffer: 8 * 1024 * 1024,
})).stdout.trim();
const hash = async file => createHash("sha256").update(await readFile(file)).digest("hex");
const cases = [];
for (const users of [1, 8]) {
  for (const payload of [128, 4096]) {
    for (const mode of ["global", "owner"]) cases.push({ name: `${users}u-${payload}b-${mode}`, users, payload, mode });
  }
}
const metadata = {
  started_at: new Date().toISOString(), complete: false, source_commit: await command("git", ["rev-parse", "HEAD"]),
  source_status: await command("git", ["status", "--porcelain", "--", "engine", "scripts/bench-event-fanout.mjs"]),
  source_sha256: {}, rustc: await command("rustc", ["--version"]), cpu: os.cpus()[0]?.model,
  kernel: os.release(), tokio_workers: 16, repeats: 3, initial_loadavg: os.loadavg(), cases,
  scope: "Current EventBus global-vs-owner runtime paths, not old-vs-new binaries. 240k events, 32 writers/subscribers, prefilled 500-record histories. Capacity 262144 prevents lag. Global receivers encode matching events separately; owner receivers use the shared lazy JSON cache, as in the real handler. Both construct every matching Utf8Bytes frame payload and must produce equal bytes. No DB, HTTP, socket, browser, kernel or Agent. Complete throughput includes consumer drain. Capacity is deliberately not production sizing.",
  baseline: baseline ? { path: baseline, sha256: await hash(baseline),
    scope: "Caller-supplied historical mainline binary; establish its source provenance separately. Only used for no-subscriber control." } : null,
};
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output); // Refuse to overwrite any previous evidence.
const sources = (await command("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
  "engine/src", "engine/examples/mainline_bench.rs", "engine/Cargo.toml", "engine/Cargo.lock"]))
  .split("\0").filter(Boolean).sort();
for (const relative of [...sources, "scripts/bench-event-fanout.mjs"]) {
  metadata.source_sha256[relative] = await hash(path.join(root, relative));
}
const saveMetadata = () => writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
await saveMetadata();
const build = await command("cargo", ["test", "--manifest-path", "engine/Cargo.toml", "--release", "--locked", "--offline", "--lib", "--no-run", "--message-format=json"]);
const binary = build.split("\n").map(line => JSON.parse(line)).find(row =>
  row.reason === "compiler-artifact" && row.target.kind.includes("lib") && row.profile.test && row.executable)?.executable;
if (!binary) throw Error("Cargo did not report a library test executable");
metadata.binary_sha256 = await hash(binary);
let current;
if (baseline) {
  await command("cargo", ["build", "--manifest-path", "engine/Cargo.toml", "--release", "--locked", "--offline", "--example", "mainline_bench"]);
  current = path.join(root, "engine/target/release/examples/mainline_bench");
  metadata.current_mainline_binary_sha256 = await hash(current);
  metadata.control_args = ["events", "2000000", "8", "32", "500", "128", "0", "0"];
}
await saveMetadata();
const resourceFile = path.join(output, "resource.tmp.json");
const format = '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M}';
const all = [];
async function record(name, round, program, args, extra, marker) {
  const loadavg = os.loadavg();
  const stdout = await command("/usr/bin/time", ["-f", format, "-o", resourceFile, program, ...args], extra);
  const line = marker ? stdout.split("\n").find(line => line.startsWith(marker)) : stdout;
  if (!line) throw Error(`Missing benchmark result for ${name}`);
  const row = { case: name, round: round + 1, loadavg, result: JSON.parse(line.slice(marker.length)),
    resource: JSON.parse(await readFile(resourceFile, "utf8")) };
  if (row.result.expected_matching_copies !== undefined &&
      (row.result.matched_copies !== row.result.expected_matching_copies || row.result.lagged_copies !== 0)) {
    throw Error(`Invalid delivery accounting for ${name}`);
  }
  all.push(row);
  await appendFile(path.join(output, "runs.jsonl"), JSON.stringify(row) + "\n");
  console.log(`[fanout ${round + 1}/3] ${name}: ${Math.round(row.result.throughput)} events/s, ${row.resource.max_rss_kib} KiB`);
}
for (let round = 0; round < 3; round++) {
  const offset = (round * 3) % cases.length;
  for (const item of [...cases.slice(offset), ...cases.slice(0, offset)]) {
    await record(item.name, round, binary,
      ["--ignored", "--exact", "services::event_bus::subscriptions::bench::measure", "--nocapture"],
      { CYANREX_FANOUT_MODE: item.mode, CYANREX_FANOUT_USERS: String(item.users), CYANREX_FANOUT_PAYLOAD: String(item.payload) },
      "CYANREX_FANOUT_BENCH=");
  }
  if (baseline) {
    const controls = [["control-before", baseline], ["control-after", current]];
    for (const [name, program] of round % 2 ? controls.reverse() : controls) {
      await record(name, round, program, metadata.control_args, {}, "");
    }
  }
}
for (const users of [1, 8]) {
  for (const payload of [128, 4096]) {
    const rows = all.filter(row => row.case.startsWith(`${users}u-${payload}b-`));
    if (new Set(rows.map(row => row.result.serialized_bytes)).size !== 1) {
      throw Error(`Serialization work differs between delivery paths: ${users}u-${payload}b`);
    }
  }
}
const summary = [...new Set(all.map(row => row.case))].map(name => {
  const metrics = {};
  for (const key of ["result.throughput", "result.publish.p95_us", "result.complete_seconds",
    "resource.user_seconds", "resource.system_seconds", "resource.max_rss_kib"]) {
    const values = all.filter(row => row.case === name).map(row => key.split(".").reduce((v, k) => v?.[k], row));
    if (values.some(v => typeof v !== "number")) continue;
    values.sort((a, b) => a - b);
    metrics[key] = { median: values[1], min: values[0], max: values[2] };
  }
  return { case: name, metrics };
});
await writeFile(path.join(output, "summary.json"), JSON.stringify(summary, null, 2) + "\n");
metadata.complete = true;
metadata.finished_at = new Date().toISOString();
metadata.final_loadavg = os.loadavg();
await saveMetadata();
await rm(resourceFile);
console.log(`[fanout] ${all.length} measured runs complete: ${output}`);
