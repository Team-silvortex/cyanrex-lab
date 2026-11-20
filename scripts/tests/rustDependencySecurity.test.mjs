import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

// Keep these known security floors covered even in offline preflight runs.
// The live cargo audit remains responsible for all other/new advisories.
const floors = [
  { name: "rustls", floor: [0, 23, 46], advisories: [
    "RUSTSEC-2026-0285", "RUSTSEC-2026-0346", "RUSTSEC-2026-0347",
    "RUSTSEC-2026-0348", "RUSTSEC-2026-0349",
  ] },
  { name: "quinn-proto", floor: [0, 11, 19], advisories: [
    "RUSTSEC-2026-0338", "RUSTSEC-2026-0339", "RUSTSEC-2026-0340",
    "RUSTSEC-2026-0341", "RUSTSEC-2026-0342", "RUSTSEC-2026-0523", "RUSTSEC-2026-0524",
  ] },
  // RUSTSEC-2026-0538: >=0.5.16,<0.6.0 OR >=0.6.3, not a single minimum.
  { name: "quinn-udp", patchedRanges: [[[0, 5, 16], [0, 6, 0]], [[0, 6, 3]]],
    advisories: ["RUSTSEC-2026-0538"] },
  { name: "rustls-webpki", floor: [0, 103, 16], advisories: ["RUSTSEC-2026-0382"] },
  { name: "webpki-roots", floor: [1, 0, 10], advisories: ["RUSTSEC-2026-0381"] },
];

function compareVersions(left, right) {
  return left.map((part, index) => part - right[index]).find((part) => part !== 0) ?? 0;
}

function isPatched(version, ranges) {
  if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) return false;
  const parts = version.split(".").map(Number);
  return ranges.some(([minimum, maximum]) => compareVersions(parts, minimum) >= 0
    && (maximum === undefined || compareVersions(parts, maximum) < 0));
}

test("quinn-udp security ranges reject the unpatched 0.6 branch and unstable versions", () => {
  const { patchedRanges } = floors.find(({ name }) => name === "quinn-udp");
  for (const [version, expected] of [
    ["0.5.15", false], ["0.5.16", true], ["0.5.17", true],
    ["0.6.0", false], ["0.6.1", false], ["0.6.2", false], ["0.6.3", true],
    ["0.6.4", true], ["0.7.0", true], ["1.0.0", true],
    ["0.5.16-rc.1", false], ["0.6.3+local", false], ["0.6", false], ["", false],
  ]) assert.equal(isPatched(version, patchedRanges), expected, version);
});

for (const { name, floor, patchedRanges, advisories } of floors) {
  test(`every locked ${name} release includes the known security fixes`, async () => {
    const lock = await readFile(new URL("../../engine/Cargo.lock", import.meta.url), "utf8");
    const packages = lock.split(/^\[\[package\]\]\s*$/m)
      .filter((block) => block.includes(`name = "${name}"\n`));
    assert.ok(packages.length > 0, `expected ${name} in Cargo.lock`);
    for (const block of packages) {
      const version = block.match(/^version = "([^"]+)"$/m)?.[1];
      assert.match(version ?? "", /^\d+\.\d+\.\d+$/, `require a stable ${name} release`);
      assert.ok(isPatched(version, patchedRanges ?? [[floor]]),
        `${name} ${version} lacks known security fixes (${advisories.join(", ")})`);
    }
  });

  test(`known ${name} advisories are fixed, not accepted as exceptions`, async () => {
    const registry = JSON.parse(await readFile(
      new URL("../security-audit-exceptions.json", import.meta.url), "utf8"));
    assert.ok(Array.isArray(registry.ignore));
    for (const id of advisories)
      assert.equal(registry.ignore.some((entry) => entry.id === id), false, id);
  });
}
