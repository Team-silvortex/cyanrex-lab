#!/usr/bin/env node

import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { cleanMacosMetadata } from "../frontend/scripts/clean-macos-metadata.mjs";

export { cleanMacosMetadata };

// Preserve the repository-wide CLI while the frontend owns its self-contained build hook.
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const projectRoot = fileURLToPath(new URL("../", import.meta.url));
  const targetRoot = process.argv[2] ? path.resolve(process.cwd(), process.argv[2]) : projectRoot;
  const removed = await cleanMacosMetadata(targetRoot);
  console.log(`[metadata] removed ${removed} macOS filesystem metadata entries`);
}
