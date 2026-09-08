import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const root = fileURLToPath(new URL("../../", import.meta.url));
const run = promisify(execFile);

test("Engine Docker context admits compile-time OpenAPI and migration assets", async () => {
  const rules = (await readFile(path.join(root, ".dockerignore"), "utf8"))
    .split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  assert.equal(rules[0], "**", "keep the Engine build context deny-by-default");
  for (const directory of ["openapi", "migrations"]) {
    assert.ok(rules.includes(`!engine/${directory}/`), `missing ${directory} directory allowance`);
    assert.ok(rules.includes(`!engine/${directory}/**`), `missing ${directory} content allowance`);
  }
});

test("Engine Docker builder copies compile-time assets before compiling", async () => {
  const dockerfile = await readFile(path.join(root, "engine/Dockerfile"), "utf8");
  const builder = dockerfile.split(/^FROM .* AS runtime$/m)[0];
  const build = builder.indexOf("cargo build");
  assert.ok(build >= 0, "builder must compile the Engine");
  for (const directory of ["openapi", "migrations"]) {
    const copy = builder.indexOf(`COPY engine/${directory} ./${directory}`);
    assert.ok(copy >= 0 && copy < build, `${directory} must be copied before cargo build`);
  }
});

test("frontend prebuild works with only its Docker context and committed course copy", async () => {
  const fixture = await mkdtemp(path.join(os.tmpdir(), "cyanrex-frontend-context-"));
  const frontend = path.join(fixture, "frontend");
  const course = path.join(frontend, "public/course/en");
  try {
    await mkdir(course, { recursive: true });
    await cp(path.join(root, "frontend/package.json"), path.join(frontend, "package.json"));
    await cp(path.join(root, "frontend/scripts"), path.join(frontend, "scripts"), { recursive: true });
    await writeFile(path.join(course, "lesson.md"), "committed lesson\n");
    await writeFile(path.join(frontend, "._sidecar"), "remove this");
    await writeFile(path.join(frontend, ".DS_Store"), "remove this too");
    await writeFile(path.join(fixture, "._outside"), "outside frontend scope");
    const result = await run("npm", ["run", "prebuild"], { cwd: frontend, timeout: 15_000 });
    assert.match(result.stdout, /removed 2 macOS filesystem metadata entries/);
    assert.equal(await readFile(path.join(course, "lesson.md"), "utf8"), "committed lesson\n");
    assert.equal(await readFile(path.join(fixture, "._outside"), "utf8"), "outside frontend scope");
    await assert.rejects(readFile(path.join(frontend, "._sidecar")), { code: "ENOENT" });
    await assert.rejects(readFile(path.join(frontend, ".DS_Store")), { code: "ENOENT" });
  } finally {
    await rm(fixture, { recursive: true, force: true });
  }
});
