import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

// Offline guards for these named advisories, not a replacement for the live npm audit.
const floors = { dompurify: "3.4.16", sharp: "0.35.5", "source-map-js": "1.2.2" };
const readJson = async file => JSON.parse(await readFile(new URL(file, import.meta.url), "utf8"));

function atLeast(version, floor) {
  assert.match(version ?? "", /^\d+\.\d+\.\d+$/, "require a stable package version");
  const actual = version.split(".").map(Number), minimum = floor.split(".").map(Number);
  assert.ok(actual.every(Number.isSafeInteger));
  for (let index = 0; index < 3; index++) {
    if (actual[index] !== minimum[index]) return actual[index] > minimum[index];
  }
  return true;
}

test("frontend overrides retain the DOMPurify, sharp and source-map-js security floors", async () => {
  const manifest = await readJson("../../frontend/package.json");
  for (const [name, floor] of Object.entries(floors)) {
    const range = manifest.overrides?.[name];
    assert.match(range ?? "", /^[~^]?\d+\.\d+\.\d+$/, `explicit minimum for ${name}`);
    assert.ok(atLeast(range.replace(/^[~^]/, ""), floor), `${name} override must exclude unsafe releases`);
  }
});

test("every locked DOMPurify, sharp and source-map-js copy includes its named advisory fix", async () => {
  const lock = await readJson("../../frontend/package-lock.json");
  for (const [name, floor] of Object.entries(floors)) {
    const copies = Object.entries(lock.packages).filter(([file]) => file.endsWith(`node_modules/${name}`));
    assert.ok(copies.length > 0, `expected locked ${name}`);
    for (const [file, value] of copies) {
      assert.ok(atLeast(value.version, floor), `${file}@${value.version} must be at least ${floor}`);
    }
  }
});

test("cross-platform sharp binaries and libvips are upgraded with the patched sharp release", async () => {
  const lock = await readJson("../../frontend/package-lock.json");
  for (const [prefix, floor] of [["sharp-libvips-", "1.3.4"], ["sharp-", "0.35.5"]]) {
    const binaries = Object.entries(lock.packages).filter(([file]) => {
      const name = file.split("node_modules/@img/")[1];
      return name?.startsWith(prefix) && (prefix !== "sharp-" || !name.startsWith("sharp-libvips-"));
    });
    assert.ok(binaries.length > 0, `expected ${prefix} platform packages`);
    for (const [file, value] of binaries) {
      assert.ok(atLeast(value.version, floor), `${file}@${value.version} must be at least ${floor}`);
    }
  }
});
