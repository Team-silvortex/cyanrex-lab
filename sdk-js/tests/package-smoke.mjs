import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

const execFileAsync = promisify(execFile);
const sdkRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(await readFile(path.join(sdkRoot, "package.json"), "utf8"));
const manifestPromise = packManifest();

test("package manifest contains the runtime and generated declaration closure", async () => {
  const manifest = await manifestPromise;
  const files = new Set(manifest.files.map((entry) => entry.path));
  for (const required of [
    "README.md",
    "dist/generated/openapi.d.ts",
    "dist/generated/openapi.js",
    "dist/generated/operations.d.ts",
    "dist/generated/operations.js",
    "dist/index.d.ts",
    "dist/index.js",
    "dist/types.d.ts",
    "dist/types.js",
    "package.json",
  ]) {
    assert.ok(files.has(required), `packed SDK is missing ${required}`);
  }
  assert.equal([...files].some((file) => file.startsWith("src/") || file.startsWith("tests/")), false);

  for (const [name, definition] of Object.entries(packageJson.exports)) {
    for (const field of ["types", "import"]) {
      const target = definition[field]?.replace(/^\.\//, "");
      assert.ok(target && files.has(target), `${name} ${field} export is not packed: ${target}`);
    }
  }
  assert.ok(packageJson.exports["./openapi"], "generated OpenAPI types need an explicit subpath export");
  assert.ok(packageJson.exports["./operations"], "generated operations need an explicit subpath export");
});

test("packed JavaScript and declarations have no dangling relative imports", async () => {
  const manifest = await manifestPromise;
  const files = new Set(manifest.files.map((entry) => entry.path));
  const importPattern = /(?:from\s+|import\s*\()\s*["'](\.[^"']+)["']/g;

  for (const entry of manifest.files.filter((item) => /\.(?:js|d\.ts)$/.test(item.path))) {
    const source = await readFile(path.join(sdkRoot, entry.path), "utf8");
    for (const match of source.matchAll(importPattern)) {
      const resolved = path.posix.normalize(path.posix.join(path.posix.dirname(entry.path), match[1]));
      assert.ok(files.has(resolved), `${entry.path} imports missing packed file ${resolved}`);
    }
  }
});

test("built ESM entry point works from a consumer-style import", async () => {
  const { CyanrexClient } = await import(packageJson.name);
  await import(`${packageJson.name}/openapi`);
  const { openApiOperations } = await import(`${packageJson.name}/operations`);
  const client = new CyanrexClient("http://localhost:8080", {
    fetch: async () => Response.json({ status: "ok" }),
  });

  assert.deepEqual(await client.system.health(), { status: "ok" });
  assert.equal(Object.keys(openApiOperations).length, 56);
  assert.deepEqual(openApiOperations.getHealth, {
    method: "GET",
    path: "/health",
    access: "public",
    transport: "json",
  });
});

async function packManifest() {
  const { stdout } = await execFileAsync(
    "npm",
    ["pack", "--dry-run", "--json", "--ignore-scripts"],
    { cwd: sdkRoot, maxBuffer: 1024 * 1024 },
  );
  const result = JSON.parse(stdout);
  assert.equal(result.length, 1, "npm pack should describe exactly one package");
  return result[0];
}
