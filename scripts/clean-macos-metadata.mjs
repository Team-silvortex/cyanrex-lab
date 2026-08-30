#!/usr/bin/env node

import { readdir, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SKIPPED_DIRECTORIES = new Set([".git", "node_modules", "target", "dist"]);

export async function cleanMacosMetadata(root) {
  let removed = 0;

  const visit = async (directory) => {
    const entries = await readdir(directory, { withFileTypes: true });
    for (const entry of entries) {
      const target = path.join(directory, entry.name);
      if (entry.name === ".DS_Store" || entry.name.startsWith("._")) {
        await rm(target, { recursive: entry.isDirectory(), force: true });
        removed += 1;
        continue;
      }
      if (entry.isDirectory() && !SKIPPED_DIRECTORIES.has(entry.name)) {
        await visit(target);
      }
    }
  };

  await visit(path.resolve(root));
  return removed;
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const projectRoot = path.resolve(path.dirname(scriptPath), "..");
  const targetRoot = process.argv[2] ? path.resolve(process.cwd(), process.argv[2]) : projectRoot;
  const removed = await cleanMacosMetadata(targetRoot);
  console.log(`[metadata] removed ${removed} macOS filesystem metadata entries`);
}
