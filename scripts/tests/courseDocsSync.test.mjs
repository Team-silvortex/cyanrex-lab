import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  compareCourseDocs,
  syncCourseDocs,
} from "../../frontend/scripts/sync-course-docs.mjs";

test("course documentation sync detects drift and replaces the generated tree", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cyanrex-course-sync-"));
  const source = path.join(root, "docs");
  const destination = path.join(root, "public", "course");

  try {
    await mkdir(path.join(source, "en"), { recursive: true });
    await mkdir(path.join(source, "zh-CN"), { recursive: true });
    await mkdir(path.join(destination, "en"), { recursive: true });
    await writeFile(path.join(source, "en", "README.md"), "current\n");
    await writeFile(path.join(source, "zh-CN", "README.md"), "当前\n");
    await writeFile(path.join(destination, "en", "README.md"), "stale\n");
    await writeFile(path.join(destination, "obsolete.md"), "remove me\n");

    assert.deepEqual(await compareCourseDocs(source, destination), [
      "changed: en/README.md",
      "missing: zh-CN/README.md",
      "unexpected: obsolete.md",
    ]);

    assert.deepEqual(await syncCourseDocs(source, destination), ["en", "zh-CN"]);
    assert.deepEqual(await compareCourseDocs(source, destination), []);
    assert.equal(await readFile(path.join(destination, "en", "README.md"), "utf8"), "current\n");
    await assert.rejects(readFile(path.join(destination, "obsolete.md")));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
