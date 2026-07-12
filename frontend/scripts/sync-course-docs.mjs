import { cp, mkdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendDir = path.resolve(scriptDir, "..");
const source = path.resolve(frontendDir, "..", "docs", "zh-CN");
const destination = path.resolve(frontendDir, "public", "course", "zh-CN");

try {
  await stat(source);
  await mkdir(destination, { recursive: true });
  await cp(source, destination, { recursive: true, force: true });
  console.log(`[course] synced ${source} -> ${destination}`);
} catch (error) {
  if (error?.code === "ENOENT") {
    console.log("[course] source docs are outside this build context; using committed static copy");
  } else {
    throw error;
  }
}
