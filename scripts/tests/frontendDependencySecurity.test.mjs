import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

// Offline guards for these named advisories, not a replacement for the live npm audit.
// DOMPurify GHSA-cg2g-xh64-v4q7, GHSA-mrv2-3pqr-j5jr and GHSA-c6gr-qvv4-5v3f.
const floors = { dompurify: "3.4.17", sharp: "0.35.5", "source-map-js": "1.2.2" };
// Retain the earlier cache fixes (GHSA-2464-fhr4-w9xw, GHSA-p3m5-59mp-jv5r).
// GHSA-7qx7-33qx-rqx2, GHSA-f3mh-rxg3-f5g2 and GHSA-pc58-hwjq-p385 require 15.5.29.
const nextFloor = "15.5.29";
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
  assert.equal(supportedNext("15.5.27"), false);
  assert.equal(supportedNext("15.5.28"), false);
  assert.equal(supportedNext("15.5.29"), true);
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

test("locked Next environment and cross-platform compilers match the reviewed patched release", async () => {
  const { packages } = await readJson("../../frontend/package-lock.json");
  const version = packages["node_modules/next"]?.version;
  assert.ok(supportedNext(version));
  for (const suffix of ["env", "swc-darwin-arm64", "swc-darwin-x64", "swc-linux-arm64-gnu",
    "swc-linux-arm64-musl", "swc-linux-x64-gnu", "swc-linux-x64-musl",
    "swc-win32-arm64-msvc", "swc-win32-x64-msvc"]) {
    const file = `node_modules/@next/${suffix}`;
    assert.equal(packages[file]?.version, version, file);
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
