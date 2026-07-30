import { cp, mkdir, readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendDir = path.resolve(scriptDir, "..");
const source = path.resolve(frontendDir, "..", "docs");
const destination = path.resolve(frontendDir, "public", "course");

try {
  await stat(source);
  await mkdir(destination, { recursive: true });

  const entries = await readdir(source, { withFileTypes: true });
  const localeDirs = entries.filter((entry) => entry.isDirectory() && !entry.name.startsWith("."));

  if (localeDirs.length === 0) {
    console.log("[course] no locale directories found under docs; skipping sync");
  } else {
    await Promise.all(localeDirs.map(async (entry) => {
      const from = path.resolve(source, entry.name);
      const to = path.resolve(destination, entry.name);
      await cp(from, to, { recursive: true, force: true });
      console.log(`[course] synced ${from} -> ${to}`);
    }));
  }
} catch (error) {
  if (error?.code === "ENOENT") {
    console.log("[course] source docs are outside this build context; using committed static copy");
  } else {
    throw error;
  }
}
