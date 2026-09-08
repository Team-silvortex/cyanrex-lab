#!/usr/bin/env node
// Runs isolated release-mode service benchmarks serially; never uses a live database/Engine.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const root = fileURLToPath(new URL("../", import.meta.url));
const run = promisify(execFile);
const output = process.argv[2] && path.resolve(process.argv[2]);
const repeats = Number(process.argv[3] || 3);
if (!output || !Number.isInteger(repeats) || repeats < 1 || repeats > 10) {
  throw new Error("Usage: node scripts/bench-mainline.mjs <new-output-directory> [repeats=3]");
}
const cases = [];
const add = (name, ...args) => cases.push({ name, args: args.map(String) });
for (const writers of [1, 4, 16, 32]) add(`events-default-w${writers}`, "events", 100000, 8, writers, 500, 128, 0, 0);
for (const [records, count] of [[5000, 30000], [20000, 20000], [50000, 10000]]) {
  add(`events-history-${records}`, "events", count, 8, 32, records, 128, 0, 0);
}
for (const users of [1, 32]) add(`events-users-${users}`, "events", 100000, users, 32, 500, 128, 0, 0);
add("events-payload-4k", "events", 100000, 8, 32, 500, 4096, 0, 0);
for (const subscribers of [8, 32]) add(`events-subscribers-${subscribers}`, "events", 60000, 8, 32, 500, 128, subscribers, 0);
add("events-readers-default", "events", 60000, 8, 32, 500, 128, 0, 8);
add("events-readers-20k", "events", 10000, 8, 32, 20000, 128, 0, 8);
for (const records of [500, 5000, 20000, 50000]) add(`snapshot-${records}`, "snapshots", records, 128, 100);
for (const operation of ["check", "complete"]) {
  add(`${operation}-cold`, "compiler", `${operation}-cold`, 50, 1);
  add(`${operation}-warm`, "compiler", `${operation}-warm`, 10000, 1);
  add(`${operation}-burst32`, "compiler", `${operation}-burst`, 20, 32);
}
for (const rows of [1000, 10000, 50000]) {
  for (const operation of ["recent", "overview", "record"]) {
    add(`learning-${operation}-${rows}`, "learning", operation, rows, 1024, operation === "recent" ? 100 : 10);
  }
}

await mkdir(path.dirname(output), { recursive: true });
await mkdir(output); // Do not replace a previous run or its results.
const env = { ...process.env, TOKIO_WORKER_THREADS: "16" };
delete env.DATABASE_URL;
delete env.CYANREX_BENCH_DATABASE_URL;
const command = async (program, args) => (await run(program, args, { cwd: root, env, timeout: 180000, maxBuffer: 4 * 1024 * 1024 })).stdout.trim();
const metadata = {
  started_at: new Date().toISOString(), source_commit: await command("git", ["rev-parse", "HEAD"]),
  rustc: await command("rustc", ["--version"]), node: process.version,
  kernel: os.release(), architecture: os.arch(), cpu: os.cpus()[0]?.model,
  logical_cpus: os.cpus().length, tokio_workers: 16, total_memory_bytes: os.totalmem(),
  initial_loadavg: os.loadavg(), repeats, build_profile: "release", database: "disabled",
  scope: "In-process EventBus + subscriber filtering/JSON; local compiler via RunnerManager; local-file LearningStore. No HTTP, PostgreSQL, eBPF load, or remote Agent.",
  measurement: "Event histories are prefilled. Publish latency excludes event construction; throughput includes it. GNU time covers the entire process including setup/teardown. Runs are serial and case order rotates per round.",
  source_worktree_status: await command("git", ["status", "--porcelain", "--", "engine/src", "engine/Cargo.toml", "engine/Cargo.lock"]),
  source_sha256: {}, benchmark_sha256: {}, cases, complete: false,
};
// Bind measurements to uncommitted implementations too; a base commit alone is ambiguous.
const sources = (await command("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "engine/src", "engine/Cargo.toml", "engine/Cargo.lock"]))
  .split("\0").filter(Boolean).sort();
for (const relative of sources) {
  metadata.source_sha256[relative] = createHash("sha256").update(await readFile(path.join(root, relative))).digest("hex");
}
for (const relative of ["engine/examples/mainline_bench.rs", "scripts/bench-mainline.mjs"]) {
  metadata.benchmark_sha256[relative] = createHash("sha256").update(await readFile(path.join(root, relative))).digest("hex");
}
await writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
await command("cargo", ["build", "--manifest-path", "engine/Cargo.toml", "--release", "--locked", "--offline", "--example", "mainline_bench"]);
const binary = path.join(root, "engine/target/release/examples/mainline_bench");
await command(binary, ["events", "3000", "8", "4", "500", "128", "0", "0"]);
const resourceFile = path.join(output, "resource.tmp.json");
const resourceFormat = '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M,"voluntary_switches":%w,"involuntary_switches":%c}';
const all = [];
for (let round = 0; round < repeats; round++) {
  const offset = (round * 11) % cases.length;
  for (const item of [...cases.slice(offset), ...cases.slice(0, offset)]) {
    const loadavg = os.loadavg();
    const stdout = await command("/usr/bin/time", ["-f", resourceFormat, "-o", resourceFile, binary, ...item.args]);
    const result = JSON.parse(stdout);
    const resource = JSON.parse(await readFile(resourceFile, "utf8"));
    const row = { case: item.name, round: round + 1, args: item.args, loadavg, resource, result };
    all.push(row);
    await appendFile(path.join(output, "runs.jsonl"), JSON.stringify(row) + "\n");
    console.log(`[bench ${round + 1}/${repeats}] ${item.name}: ${resource.wall_seconds}s`);
  }
}
function leaves(value, prefix = "", values = {}) {
  for (const [key, entry] of Object.entries(value)) {
    const name = prefix ? `${prefix}.${key}` : key;
    if (typeof entry === "number") values[name] = entry;
    else if (entry && typeof entry === "object" && !Array.isArray(entry)) leaves(entry, name, values);
  }
  return values;
}
const summaries = cases.map(item => {
  const rows = all.filter(row => row.case === item.name).map(row => leaves({ result: row.result, resource: row.resource }));
  const metrics = {};
  for (const key of Object.keys(rows[0])) {
    const values = rows.map(row => row[key]).sort((a, b) => a - b);
    const mid = Math.floor(values.length / 2);
    metrics[key] = { median: values.length % 2 ? values[mid] : (values[mid - 1] + values[mid]) / 2,
      min: values[0], max: values.at(-1) };
  }
  return { case: item.name, args: item.args, repeats, metrics };
});
await writeFile(path.join(output, "summary.json"), JSON.stringify(summaries, null, 2) + "\n");
metadata.complete = true;
metadata.finished_at = new Date().toISOString();
metadata.final_loadavg = os.loadavg();
await writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
await rm(resourceFile);
console.log(`[bench] ${all.length} measured runs complete: ${output}`);
