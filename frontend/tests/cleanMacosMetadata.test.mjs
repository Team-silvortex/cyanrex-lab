import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { cleanMacosMetadata } from "../../scripts/clean-macos-metadata.mjs";

test("cleanMacosMetadata removes Finder sidecars without touching source files", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cyanrex-macos-metadata-"));

  try {
    await mkdir(path.join(root, "pages", "nested"), { recursive: true });
    await writeFile(path.join(root, "pages", "index.tsx"), "export default function Page() {}\n");
    await writeFile(path.join(root, "pages", "._index.tsx"), Buffer.from([0, 5, 22, 7]));
    await writeFile(path.join(root, "pages", ".DS_Store"), Buffer.from([1, 2, 3]));
    await writeFile(path.join(root, "pages", "nested", "._helper.tsx"), Buffer.from([4, 5]));

    const removed = await cleanMacosMetadata(root);

    assert.equal(removed, 3);
    assert.equal(
      await readFile(path.join(root, "pages", "index.tsx"), "utf8"),
      "export default function Page() {}\n",
    );
    await assert.rejects(readFile(path.join(root, "pages", "._index.tsx")));
    await assert.rejects(readFile(path.join(root, "pages", ".DS_Store")));
    await assert.rejects(readFile(path.join(root, "pages", "nested", "._helper.tsx")));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
