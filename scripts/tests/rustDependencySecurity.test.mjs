import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

// Keep this known security floor covered even in offline preflight runs.
// The live cargo audit remains responsible for all other/new advisories.
test("every locked rustls release includes the RUSTSEC-2026-0285 fix", async () => {
  const lock = await readFile(new URL("../../engine/Cargo.lock", import.meta.url), "utf8");
  const packages = lock.split(/^\[\[package\]\]\s*$/m)
    .filter((block) => /^name = "rustls"$/m.test(block));
  assert.ok(packages.length > 0, "expected the reqwest TLS dependency in Cargo.lock");
  for (const block of packages) {
    const version = block.match(/^version = "([^"]+)"$/m)?.[1];
    assert.match(version ?? "", /^\d+\.\d+\.\d+$/, "require a stable rustls release");
    const [major, minor, patch] = version.split(".").map(Number);
    assert.ok(major > 0 || minor > 23 || (minor === 23 && patch >= 45),
      `rustls ${version} is below the patched 0.23.45 floor (RUSTSEC-2026-0285)`);
  }
});

test("the rustls handshake advisory is fixed, not accepted as an exception", async () => {
  const registry = JSON.parse(await readFile(
    new URL("../security-audit-exceptions.json", import.meta.url), "utf8"));
  assert.ok(Array.isArray(registry.ignore));
  assert.equal(registry.ignore.some((entry) => entry.id === "RUSTSEC-2026-0285"), false);
});
