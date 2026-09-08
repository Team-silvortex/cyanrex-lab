#!/usr/bin/env node
// Calls a deliberately ignored, private service benchmark. Does not expose new Engine APIs.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile, appendFile, rm } from "node:fs/promises";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import os from "node:os";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = process.argv[2] && path.resolve(process.argv[2]);
if (!output || process.argv.length !== 3) throw Error("Usage: node scripts/bench-event-history-reads.mjs <new-output-directory>");
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output);
const env = { ...process.env, TOKIO_WORKER_THREADS: "16" };
delete env.DATABASE_URL;
delete env.CYANREX_BENCH_DATABASE_URL;
const run = promisify(execFile);
const command = async (program, args, extra = {}) => (await run(program, args, {
  cwd: root, env: { ...env, ...extra }, timeout: 180000, maxBuffer: 8 * 1024 * 1024,
})).stdout.trim();
const hash = async relative => createHash("sha256").update(await readFile(path.join(root, relative))).digest("hex");
const cases = [];
for (const records of [500, 5000, 20000, 50000]) {
  for (const kind of ["latest", "filtered", "miss", "full"]) cases.push({ name: `${kind}-${records}`, kind, records });
}
for (const records of [500, 20000]) cases.push({ name: `mixed-${records}`, kind: "mixed", records });
const metadata = {
  started_at: new Date().toISOString(), complete: false, source_commit: await command("git", ["rev-parse", "HEAD"]),
  source_status: await command("git", ["status", "--porcelain", "--", "engine/src"]), source_sha256: {},
  rustc: await command("rustc", ["--version"]), cpu: os.cpus()[0]?.model, kernel: os.release(),
  tokio_workers: 16, build: "release test executable (cfg(test)); not a production binary", repeats: 3,
  initial_loadavg: os.loadavg(), cases,
  scope: "Private EventBus filtered snapshots; no database, HTTP, subscriptions, or kernel eBPF. Payload field 128 bytes. Mixed reads sleep 10 ms after each latest-200 query; synthetic contention, not frontend polling.",
};
const sources = (await command("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "engine/src", "engine/Cargo.toml", "engine/Cargo.lock"]))
  .split("\0").filter(Boolean).sort();
for (const relative of [...sources, "scripts/bench-event-history-reads.mjs"]) metadata.source_sha256[relative] = await hash(relative);
await writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
const build = await command("cargo", ["test", "--manifest-path", "engine/Cargo.toml", "--release", "--locked", "--offline", "--lib", "--no-run", "--message-format=json"]);
const binary = build.split("\n").map(line => JSON.parse(line)).find(row =>
  row.reason === "compiler-artifact" && row.target.kind.includes("lib") && row.profile.test && row.executable)?.executable;
if (!binary) throw Error("Cargo did not report a library test executable");
metadata.binary_sha256 = createHash("sha256").update(await readFile(binary)).digest("hex");
const resourceFile = path.join(output, "resource.tmp.json");
const format = '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M}';
const all = [];
for (let round = 0; round < 3; round++) {
  const offset = (round * 7) % cases.length;
  for (const item of [...cases.slice(offset), ...cases.slice(0, offset)]) {
    const loadavg = os.loadavg();
    const stdout = await command("/usr/bin/time", ["-f", format, "-o", resourceFile, binary,
      "--ignored", "--exact", "services::event_bus::read_bench::measure", "--nocapture"],
    { CYANREX_READ_BENCH_KIND: item.kind, CYANREX_READ_BENCH_RECORDS: String(item.records) });
    const match = stdout.match(/^CYANREX_READ_BENCH=(\{.*\})$/m);
    if (!match) throw Error("Missing benchmark result");
    const row = { case: item.name, round: round + 1, loadavg, result: JSON.parse(match[1]),
      resource: JSON.parse(await readFile(resourceFile, "utf8")) };
    all.push(row);
    await appendFile(path.join(output, "runs.jsonl"), JSON.stringify(row) + "\n");
    console.log(`[read bench ${round + 1}/3] ${item.name}: ${row.resource.wall_seconds}s`);
  }
}
const summaries = cases.map(item => {
  const rows = all.filter(row => row.case === item.name);
  const metrics = {};
  const paths = ["result.throughput", "result.publish.p95_us", "result.snapshot.p95_us", "resource.max_rss_kib"];
  for (const key of paths) {
    const values = rows.map(row => key.split(".").reduce((value, part) => value?.[part], row));
    if (values.some(value => typeof value !== "number")) continue;
    values.sort((a, b) => a - b);
    metrics[key] = { median: values[1], min: values[0], max: values[2] };
  }
  return { ...item, metrics };
});
await writeFile(path.join(output, "summary.json"), JSON.stringify(summaries, null, 2) + "\n");
metadata.complete = true;
metadata.finished_at = new Date().toISOString();
metadata.final_loadavg = os.loadavg();
await writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
await rm(resourceFile);
console.log(`[read bench] ${all.length} measured runs complete: ${output}`);
