import { cp, mkdir, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const source = resolve(root, "node_modules/monaco-editor/min/vs");
const target = resolve(root, "public/monaco/vs");

async function main() {
  await mkdir(resolve(root, "public/monaco"), { recursive: true });
  await rm(target, { recursive: true, force: true });
  await cp(source, target, { recursive: true });
  console.log(`[monaco] synced assets to ${target}`);
}

main().catch((err) => {
  console.error("[monaco] sync failed:", err);
  process.exit(1);
});
