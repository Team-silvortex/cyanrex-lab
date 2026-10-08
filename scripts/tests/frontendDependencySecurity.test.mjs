import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

// Offline guards for these named advisories, not a replacement for the live npm audit.
const floors = { dompurify: "3.4.16", sharp: "0.35.5", "source-map-js": "1.2.2" };
// GHSA-4jqv-mc3x-m676 and GHSA-mcj8-r9mp-w47p are fixed in this stable Next release.
const nextFloor = "15.5.27";
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

function supportedNext(version) {
  // A higher major is not automatically patched or compatible; review that branch separately.
  return atLeast(version, nextFloor) && version.split(".")[0] === "15";
}

test("frontend dependencies retain the Next, DOMPurify, sharp and source-map-js security floors", async () => {
  assert.equal(supportedNext("15.5.26"), false);
  assert.equal(supportedNext("15.5.27"), true);
  assert.equal(supportedNext("16.0.0"), false);
  const manifest = await readJson("../../frontend/package.json");
  const nextRange = manifest.dependencies?.next;
  assert.match(nextRange ?? "", /^[~^]?\d+\.\d+\.\d+$/, "explicit minimum for next");
  assert.ok(supportedNext(nextRange.replace(/^[~^]/, "")), "next dependency must use the reviewed patched 15.x branch");
  for (const [name, floor] of Object.entries(floors)) {
    const range = manifest.overrides?.[name];
    assert.match(range ?? "", /^[~^]?\d+\.\d+\.\d+$/, `explicit minimum for ${name}`);
    assert.ok(atLeast(range.replace(/^[~^]/, ""), floor), `${name} override must exclude unsafe releases`);
  }
});

test("every locked Next, DOMPurify, sharp and source-map-js copy includes its named advisory fix", async () => {
  const lock = await readJson("../../frontend/package-lock.json");
  for (const [name, floor] of Object.entries({ ...floors, next: nextFloor })) {
    const copies = Object.entries(lock.packages).filter(([file]) => file.endsWith(`node_modules/${name}`));
    assert.ok(copies.length > 0, `expected locked ${name}`);
    for (const [file, value] of copies) {
      if (name === "next") {
        assert.ok(supportedNext(value.version), `${file}@${value.version} must use the reviewed patched 15.x branch`);
      } else {
        assert.ok(atLeast(value.version, floor), `${file}@${value.version} must be at least ${floor}`);
      }
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
