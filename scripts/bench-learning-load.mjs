#!/usr/bin/env node
// Cold-process reads of identical files; fixture generation is outside each measured Rust process.
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { appendFile, mkdir, mkdtemp, open, readFile, realpath, rmdir, stat, unlink, writeFile } from "node:fs/promises";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import os from "node:os";
import { learningLoadPairs } from "./bench-learning-store-cases.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = process.argv[2] && path.resolve(process.argv[2]);
const baseline = process.argv[3] && path.resolve(process.argv[3]);
if (!output || !baseline || process.argv.length !== 4) {
  throw Error("Usage: node scripts/bench-learning-load.mjs <new-output-directory> <frozen-baseline-learning_load_bench>");
}
const current = path.join(root, "engine/target/release/examples/learning_load_bench");
if (await realpath(baseline) === await realpath(current).catch(error => {
  if (error.code !== "ENOENT") throw error;
  return current;
})) throw Error("Copy the baseline outside the build output before running this comparison.");
const env = { ...process.env };
delete env.DATABASE_URL;
delete env.CYANREX_BENCH_DATABASE_URL;
const run = promisify(execFile);
const command = async (program, args) => (await run(program, args, {
  cwd: root, env, timeout: 180000, maxBuffer: 8 * 1024 * 1024,
})).stdout.trim();
async function hash(file) {
  const digest = createHash("sha256");
  for await (const chunk of createReadStream(file)) digest.update(chunk);
  return digest.digest("hex");
}
const cases = learningLoadPairs(0).map(({ item }) => ({
  name: item.name, rows: Number(item.args[2]), padding_bytes: Number(item.args[3]),
  source_bytes: Number(item.args[3]) + 88,
}));
const metadata = {
  started_at: new Date().toISOString(), complete: false, source_commit: await command("git", ["rev-parse", "HEAD"]),
  source_status: await command("git", ["status", "--porcelain", "--", "engine"]), source_sha256: {},
  rustc: await command("rustc", ["--version"]), node: process.version, cpu: os.cpus()[0]?.model,
  kernel: os.release(), runtime: "current_thread", async_workers: 1, total_memory_bytes: os.totalmem(),
  temporary_directory: os.tmpdir(), repeats: 5, warmup_processes: 2, cases, fixtures: [],
  baseline: { path: baseline, sha256: await hash(baseline), provenance: "Caller must establish baseline source and identical harness identity separately." },
  scope: "Read-only LearningStore first recent-20 query in a fresh process, plus one warm query and a 2 ms async timer. Thirty synthetic students. Identical fixtures are generated outside measured Rust processes; freshly written files remain in the OS page cache. This is not a cold-disk benchmark. GNU time includes runtime, loading, queries, overview validation and teardown, but not fixture creation. No database, HTTP, eBPF, compiler, Agent, fsync, CPU pinning or cache eviction. Each result is one process observation, not a request percentile. Case order rotates and before/after order swaps each round.",
  initial_loadavg: os.loadavg(), fixtures_unchanged: false, temporary_fixtures_cleaned: false,
};
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output); // Existing reports are immutable.
const files = (await command("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
  "engine/src", "engine/examples/learning_load_bench.rs", "engine/Cargo.toml", "engine/Cargo.lock"]))
  .split("\0").filter(Boolean).sort();
for (const file of [...files, "scripts/bench-learning-load.mjs", "scripts/bench-learning-store-cases.mjs", "scripts/tests/learningBenchmark.test.mjs"]) {
  metadata.source_sha256[file] = await hash(path.join(root, file));
}
const saveMetadata = () => writeFile(path.join(output, "metadata.json"), JSON.stringify(metadata, null, 2) + "\n");
await saveMetadata();
await command("cargo", ["build", "--manifest-path", "engine/Cargo.toml", "--release", "--locked", "--offline", "--example", "learning_load_bench"]);
metadata.current_binary_sha256 = await hash(current);
await saveMetadata();

const temporary = await mkdtemp(path.join(os.tmpdir(), "cyanrex-learning-load-"));
const ownedFiles = [];
async function writeFixture(item) {
  const file = path.join(temporary, `${item.name}.json`);
  const handle = await open(file, "wx");
  ownedFiles.push(file);
  const source = "x".repeat(item.source_bytes);
  let buffer = "[\n";
  try {
    for (let index = 0; index < item.rows; index++) {
      const row = {
        id: `bench-attempt-${index}`, username: `bench-user-${index % 30}`, lab_id: "01-first-program",
        template_id: "xdp-pass", source, source_sha256: "synthetic-fixture", run_success: false,
        stage: "compile", attach_expected: false, attach_verified: false, completed: false,
        feedback: [], teacher_feedback: null, created_at: new Date(Date.UTC(2026, 8, 8) + index).toISOString(),
      };
      buffer += JSON.stringify(row, null, 2).replace(/^/gm, "  ") + (index + 1 === item.rows ? "\n]" : ",\n");
      if (buffer.length >= 64 * 1024) {
        await handle.writeFile(buffer);
        buffer = "";
      }
    }
    if (buffer) await handle.writeFile(buffer);
  } finally {
    await handle.close();
  }
  return { ...item, path: file, file_bytes: (await stat(file)).size, sha256: await hash(file) };
}
const resourceFile = path.join(output, "resource.tmp.json");
try {
  for (const item of cases) metadata.fixtures.push(await writeFixture(item));
  await saveMetadata();
  const binaries = { before: baseline, after: current };
  const small = metadata.fixtures.find(item => item.rows === 1000);
  for (const binary of [baseline, current]) await command(binary, [small.path, String(small.rows), String(small.source_bytes)]);
  const all = [];
  const format = '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M}';
  for (let round = 0; round < metadata.repeats; round++) {
    for (const { item, phases } of learningLoadPairs(round)) {
      const fixture = metadata.fixtures.find(value => value.name === item.name);
      const args = [fixture.path, String(fixture.rows), String(fixture.source_bytes)];
      for (const phase of phases) {
        const loadavg = os.loadavg();
        const result = JSON.parse(await command("/usr/bin/time", ["-f", format, "-o", resourceFile, binaries[phase], ...args]));
        if (result.mode !== "learning_cold_load" || result.runtime !== "current_thread" || result.read_only !== true ||
            result.initial_rows !== fixture.rows || result.source_bytes !== fixture.source_bytes ||
            result.file_bytes !== fixture.file_bytes || result.timer_period_ms !== 2 || !(result.timer_ticks > 0)) {
          throw Error(`Unexpected cold-load output: ${item.name} ${phase}`);
        }
        const row = { case: item.name, phase, round: round + 1, args, loadavg, result,
          resource: JSON.parse(await readFile(resourceFile, "utf8")) };
        all.push(row);
        await appendFile(path.join(output, "runs.jsonl"), JSON.stringify(row) + "\n");
        console.log(`[load ${round + 1}/${metadata.repeats}] ${item.name} ${phase}: first=${result.first_read_ms.toFixed(3)}ms, timer=${result.max_tick_delay_ms.toFixed(3)}ms, ${row.resource.max_rss_kib} KiB`);
      }
    }
  }
  const summary = [];
  for (const item of cases) for (const phase of ["before", "after"]) {
    const rows = all.filter(row => row.case === item.name && row.phase === phase);
    if (rows.length !== metadata.repeats) throw Error(`Missing runs: ${item.name} ${phase}`);
    const metrics = {};
    for (const key of ["result.first_read_ms", "result.warm_read_ms", "result.max_tick_delay_ms", "result.timer_ticks",
      "result.file_bytes", "resource.wall_seconds", "resource.user_seconds", "resource.system_seconds", "resource.max_rss_kib"]) {
      const values = rows.map(row => key.split(".").reduce((value, part) => value?.[part], row)).sort((a, b) => a - b);
      if (values.some(value => !Number.isFinite(value) || value < 0)) throw Error(`Invalid metric: ${key}`);
      metrics[key] = { median: values[Math.floor(values.length / 2)], min: values[0], max: values.at(-1) };
    }
    summary.push({ case: item.name, phase, rows: item.rows, source_bytes: item.source_bytes, metrics });
  }
  for (const fixture of metadata.fixtures) {
    if ((await stat(fixture.path)).size !== fixture.file_bytes || await hash(fixture.path) !== fixture.sha256) {
      throw Error(`Read-only fixture changed: ${fixture.name}`);
    }
  }
  metadata.fixtures_unchanged = true;
  await writeFile(path.join(output, "summary.json"), JSON.stringify(summary, null, 2) + "\n");
  await unlink(resourceFile);
  metadata.measured_processes = all.length;
  metadata.complete = true;
  console.log(`[load] ${all.length} measured runs complete: ${output}`);
} finally {
  for (const file of ownedFiles) await unlink(file);
  await rmdir(temporary);
  metadata.temporary_fixtures_cleaned = true;
  metadata.finished_at = new Date().toISOString();
  metadata.final_loadavg = os.loadavg();
  await saveMetadata();
}
