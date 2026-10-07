import { readFile, readdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { MONACO_SANITIZER, prepareMonacoSanitizer, publishMonacoAssets } from "./monaco-sanitizer.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const source = resolve(root, "node_modules/monaco-editor/min/vs");
const target = resolve(root, "public/monaco/vs");

async function readScripts(directory, prefix = "", files = new Map()) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const relative = `${prefix}${entry.name}`, absolute = resolve(directory, entry.name);
    if (entry.isDirectory()) await readScripts(absolute, `${relative}/`, files);
    else if (entry.name.endsWith(".js")) files.set(relative, await readFile(absolute, "utf8"));
  }
  return files;
}

async function prepare() {
  const [monaco, dompurify, dompurifySource, files] = await Promise.all([
    readFile(resolve(root, "node_modules/monaco-editor/package.json"), "utf8").then(JSON.parse),
    readFile(resolve(root, "node_modules/dompurify/package.json"), "utf8").then(JSON.parse),
    readFile(resolve(root, "node_modules/dompurify/dist/purify.es.mjs"), "utf8"),
    readScripts(source),
  ]);
  // Verify and prepare everything before touching generated output. Never patch node_modules.
  return prepareMonacoSanitizer({
    files, dompurifySource, monacoVersion: monaco.version, dompurifyVersion: dompurify.version,
  });
}

async function main() {
  const { chunkFile } = await publishMonacoAssets(root, prepare);
  console.log(`[monaco] synced assets to ${target}`);
  console.log(`[monaco] DOMPurify ${MONACO_SANITIZER.dompurifyVersion} in ${chunkFile}`);
}

main().catch((err) => {
  console.error("[monaco] sync failed:", err);
  process.exit(1);
});
