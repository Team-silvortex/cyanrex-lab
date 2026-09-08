#!/usr/bin/env node
// Paired before/after runs using the unchanged public mainline benchmark example.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import os from "node:os";
import { learningCases as cases, learningPairs } from "./bench-learning-store-cases.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = process.argv[2] && path.resolve(process.argv[2]);
const baseline = process.argv[3] && path.resolve(process.argv[3]);
if (!output || !baseline || process.argv.length !== 4) {
  throw Error("Usage: node scripts/bench-learning-store.mjs <new-output-directory> <frozen-baseline-mainline-binary>");
}
const env = { ...process.env, TOKIO_WORKER_THREADS: "16" };
delete env.DATABASE_URL;
delete env.CYANREX_BENCH_DATABASE_URL;
const run = promisify(execFile);
const command = async (program, args) => (await run(program, args, {
  cwd: root, env, timeout: 180000, maxBuffer: 8 * 1024 * 1024,
})).stdout.trim();
const hash = async file => createHash("sha256").update(await readFile(file)).digest("hex");
const metadata = {
  started_at: new Date().toISOString(), complete: false, source_commit: await command("git", ["rev-parse", "HEAD"]),
  source_status: await command("git", ["status", "--porcelain", "--", "engine"]), source_sha256: {},
  rustc: await command("rustc", ["--version"]), node: process.version, cpu: os.cpus()[0]?.model,
  kernel: os.release(), tokio_workers: 16, temporary_directory: os.tmpdir(), total_memory_bytes: os.totalmem(), repeats: 3, cases,
  baseline: { path: baseline, sha256: await hash(baseline), provenance: "Caller must establish baseline source identity separately." },
  scope: "Local-file LearningStore public mainline example; no database, HTTP, eBPF, compiler, or Agent. Thirty synthetic students. Source bytes include the fixed example source plus the padding parameter. Data is preloaded before latency measurement. Writes still rewrite the entire pretty JSON file without fsync. GNU time includes fixture creation/loading/deletion; not steady-state RSS. Pairs and cases rotate; no CPU pinning or page-cache eviction.",
  initial_loadavg: os.loadavg(),
};
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output); // Never overwrite prior measurements.
const files = (await command("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
  "engine/src", "engine/examples/mainline_bench.rs", "engine/Cargo.toml", "engine/Cargo.lock"]))
  .split("\0").filter(Boolean).sort();
for (const file of [...files, "scripts/bench-learning-store.mjs", "scripts/bench-learning-store-cases.mjs", "scripts/tests/learningBenchmark.test.mjs"]) {
  metadata.source_sha256[file] = await hash(path.join(root, file));
}
const saveMetadata = () => writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
await saveMetadata();
await command("cargo", ["build", "--manifest-path", "engine/Cargo.toml", "--release", "--locked", "--offline", "--example", "mainline_bench"]);
const current = path.join(root, "engine/target/release/examples/mainline_bench");
metadata.current_binary_sha256 = await hash(current);
await saveMetadata();
for (const binary of [baseline, current]) await command(binary, ["learning", "recent", "1000", "128", "10"]);
const resourceFile = path.join(output, "resource.tmp.json");
const format = '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M}';
const all = [];
const binaries = { before: baseline, after: current };
for (let round = 0; round < 3; round++) {
  for (const { item, phases } of learningPairs(round)) {
    for (const phase of phases) {
      const binary = binaries[phase];
      const loadavg = os.loadavg();
      const stdout = await command("/usr/bin/time", ["-f", format, "-o", resourceFile, binary, ...item.args]);
      const result = JSON.parse(stdout);
      if (result.mode !== "learning" || result.kind !== item.args[1] || result.initial_rows !== Number(item.args[2]) ||
          result.latency?.count !== Number(item.args[4]) || result.storage !== "isolated_local_file") {
        throw Error(`Unexpected benchmark output: ${item.name} ${phase}`);
      }
      const row = { case: item.name, phase, round: round + 1, args: item.args, loadavg, result,
        resource: JSON.parse(await readFile(resourceFile, "utf8")) };
      all.push(row);
      await appendFile(path.join(output, "runs.jsonl"), JSON.stringify(row) + "\n");
      console.log(`[learning ${round + 1}/3] ${item.name} ${phase}: p95=${(result.latency.p95_us / 1000).toFixed(3)}ms, ${row.resource.max_rss_kib} KiB`);
    }
  }
}
const summary = [];
for (const item of cases) for (const phase of ["before", "after"]) {
  const rows = all.filter(row => row.case === item.name && row.phase === phase);
  const metrics = {};
  for (const key of ["result.latency.p50_us", "result.latency.p95_us", "result.latency.mean_us",
    "result.file_bytes", "resource.wall_seconds", "resource.user_seconds", "resource.system_seconds", "resource.max_rss_kib"]) {
    const values = rows.map(row => key.split(".").reduce((value, part) => value?.[part], row));
    if (values.some(value => typeof value !== "number")) throw Error(`Missing result: ${key}`);
    values.sort((a, b) => a - b);
    metrics[key] = { median: values[1], min: values[0], max: values[2] };
  }
  summary.push({ case: item.name, phase, args: item.args, metrics });
}
await writeFile(path.join(output, "summary.json"), JSON.stringify(summary, null, 2) + "\n");
metadata.complete = true;
metadata.finished_at = new Date().toISOString();
metadata.final_loadavg = os.loadavg();
await saveMetadata();
await rm(resourceFile);
console.log(`[learning] ${all.length} measured runs complete: ${output}`);
