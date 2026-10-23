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
  { name: "quinn-proto", floor: [0, 11, 18], advisories: [
    "RUSTSEC-2026-0338", "RUSTSEC-2026-0339", "RUSTSEC-2026-0340",
    "RUSTSEC-2026-0341", "RUSTSEC-2026-0342",
  ] },
  { name: "rustls-webpki", floor: [0, 103, 16], advisories: ["RUSTSEC-2026-0382"] },
  { name: "webpki-roots", floor: [1, 0, 10], advisories: ["RUSTSEC-2026-0381"] },
];

for (const { name, floor, advisories } of floors) {
  test(`every locked ${name} release includes the known security fixes`, async () => {
    const lock = await readFile(new URL("../../engine/Cargo.lock", import.meta.url), "utf8");
    const packages = lock.split(/^\[\[package\]\]\s*$/m)
      .filter((block) => block.includes(`name = "${name}"\n`));
    assert.ok(packages.length > 0, `expected ${name} in Cargo.lock`);
    for (const block of packages) {
      const version = block.match(/^version = "([^"]+)"$/m)?.[1];
      assert.match(version ?? "", /^\d+\.\d+\.\d+$/, `require a stable ${name} release`);
      const parts = version.split(".").map(Number);
      const difference = parts.map((part, index) => part - floor[index]).find((part) => part !== 0) ?? 0;
      assert.ok(difference >= 0,
        `${name} ${version} is below the patched ${floor.join(".")} floor (${advisories.join(", ")})`);
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
