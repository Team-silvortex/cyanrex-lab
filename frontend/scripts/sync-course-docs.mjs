#!/usr/bin/env node

import { cp, mkdir, readdir, readFile, rm, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendDir = path.resolve(scriptDir, "..");
const defaultSource = path.resolve(frontendDir, "..", "docs");
const defaultDestination = path.resolve(frontendDir, "public", "course");

async function collectFiles(root, allowMissing = false) {
  const files = new Map();

  const visit = async (directory, relativeDirectory = "") => {
    let entries;
    try {
      entries = await readdir(directory, { withFileTypes: true });
    } catch (error) {
      if (allowMissing && error?.code === "ENOENT" && relativeDirectory === "") return;
      throw error;
    }

    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const relative = path.posix.join(relativeDirectory, entry.name);
      const target = path.join(directory, entry.name);
      if (entry.isDirectory()) await visit(target, relative);
      else if (entry.isFile()) files.set(relative, await readFile(target));
      else throw new Error(`course documentation contains unsupported entry ${relative}`);
    }
  };

  await visit(path.resolve(root));
  return files;
}

export async function compareCourseDocs(source, destination) {
  await stat(source);
  const [sourceFiles, destinationFiles] = await Promise.all([
    collectFiles(source),
    collectFiles(destination, true),
  ]);
  const differences = [];

  for (const [relative, expected] of sourceFiles) {
    const actual = destinationFiles.get(relative);
    if (!actual) differences.push(`missing: ${relative}`);
    else if (!actual.equals(expected)) differences.push(`changed: ${relative}`);
  }
  for (const relative of destinationFiles.keys()) {
    if (!sourceFiles.has(relative)) differences.push(`unexpected: ${relative}`);
  }
  return differences;
}

export async function syncCourseDocs(source, destination) {
  await stat(source);
  const parent = path.dirname(path.resolve(destination));
  await mkdir(parent, { recursive: true });
  await rm(destination, { recursive: true, force: true });
  await cp(source, destination, { recursive: true, force: true });
  return [...(await readdir(source, { withFileTypes: true }))]
    .filter((entry) => entry.isDirectory() && !entry.name.startsWith("."))
    .map((entry) => entry.name)
    .sort();
}

function usage() {
  console.log(`Usage: node frontend/scripts/sync-course-docs.mjs [--check]

With no arguments, replace frontend/public/course with the authoritative docs tree.
Use --check to reject missing, changed, or unexpected files without modifying either tree.`);
}

const isMain = process.argv[1]
  && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMain) {
  const arguments_ = process.argv.slice(2);
  if (arguments_.includes("-h") || arguments_.includes("--help")) {
    usage();
  } else if (arguments_.length > 1 || (arguments_.length === 1 && arguments_[0] !== "--check")) {
    throw new Error(`unknown course sync argument: ${arguments_.join(" ")}`);
  } else if (arguments_[0] === "--check") {
    const differences = await compareCourseDocs(defaultSource, defaultDestination);
    if (differences.length > 0) {
      console.error("[course] committed frontend course copy differs from docs:");
      for (const difference of differences) console.error(`  ${difference}`);
      process.exitCode = 1;
    } else {
      console.log("[course] committed frontend course copy matches docs.");
    }
  } else {
    try {
      const locales = await syncCourseDocs(defaultSource, defaultDestination);
      console.log(`[course] synchronized locale copies: ${locales.join(", ")}`);
    } catch (error) {
      if (error?.code === "ENOENT") {
        console.log("[course] source docs are outside this build context; using committed static copy");
      } else {
        throw error;
      }
    }
  }
}
