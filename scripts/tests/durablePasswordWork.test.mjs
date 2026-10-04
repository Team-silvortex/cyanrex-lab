import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = (file) => readFile(new URL(`../../engine/src/services/auth_service/${file}`, import.meta.url), "utf8");

test("all five durable password operations use the shared fallible worker boundary", async () => {
  const module = await source("durable_source/mod.rs");
  assert.match(module, /use password_work::\{derive_password_hash, verify_password\}/);
  assert.doesNotMatch(module, /derive_password_hash_async|verify_password_async/);
  const operations = [
    ["accounts.rs", { derive_password_hash: 1, verify_password: 0 }],
    ["sessions.rs", { derive_password_hash: 0, verify_password: 1 }],
    ["credentials.rs", { derive_password_hash: 1, verify_password: 1 }],
    ["bootstrap.rs", { derive_password_hash: 1, verify_password: 0 }],
  ];
  for (const [file, expected] of operations) {
    const contents = await source(`durable_source/${file}`);
    assert.doesNotMatch(contents, /spawn_blocking\s*\(|Argon2::|(?:derive_password_hash_async|verify_password_async)\s*\(/, `${file} must not bypass password admission`);
    assert.doesNotMatch(contents, /\b(?:super|crate)(?:::\w+)*::(?:derive_password_hash|verify_password)\s*\(/, `${file} must not call legacy crypto directly`);
    for (const [operation, count] of Object.entries(expected)) {
      const calls = [...contents.matchAll(new RegExp(`\\b${operation}\\s*\\(`, "g"))];
      assert.equal(calls.length, count, `${file}: every ${operation} call requires deliberate worker coverage`);
      const propagated = [...contents.matchAll(new RegExp(`\\b${operation}\\s*\\([\\s\\S]*?\\)\\s*\\.await\\?`, "g"))];
      assert.equal(propagated.length, count, `${file}: saturation and worker failure must not become wrong-password results`);
    }
  }
});

test("durable password admission stays private and does not replace legacy crypto", async () => {
  const worker = await source("durable_source/password_work.rs");
  const module = await source("durable_source/mod.rs");
  const legacy = await source("crypto.inc.rs");
  assert.match(worker, /OnceLock<PasswordWorkGate>/);
  assert.doesNotMatch(module, /pub(?:\([^)]*\))?\s+(?:mod|use)\s+password_work/);
  assert.doesNotMatch(legacy, /password_work|PasswordWorkGate/);
  assert.match(legacy, /async fn verify_password_async\(/);
  assert.match(legacy, /async fn derive_password_hash_async\(/);
  assert.match(legacy, /Argon2::default\(\)/);
});

test("durable PHC validation precedes worker admission and account publication", async () => {
  const worker = await source("durable_source/password_work.rs");
  const verifier = worker.slice(worker.indexOf("async fn verify_password("));
  const preflight = verifier.indexOf("password_profile::validate(expected_hash)?");
  const admission = verifier.indexOf("shared_gate()");
  const copy = verifier.indexOf(".to_owned()");
  assert.ok(preflight >= 0 && preflight < admission && preflight < copy, "PHC preflight must reject before gate admission and secret copies");
  const accounts = await source("durable_source/accounts.rs");
  const accountRead = accounts.slice(accounts.indexOf("async fn account_record("), accounts.indexOf("pub async fn register("));
  assert.match(accountRead, /password_profile::validate\(&user\.password_hash\)\?/);
  assert.ok(accountRead.indexOf("password_profile::validate") < accountRead.indexOf("Ok(AccountRecord"), "unsupported stored profiles cannot become trusted account snapshots");
  const profile = await source("durable_source/password_profile.rs");
  assert.doesNotMatch(profile, /Argon2::default\s*\(|Params::try_from\s*\(|\.verify_password\s*\(/, "durable work must not inherit mutable defaults or stored PHC costs");
  assert.match(worker, /password_profile::derive\(/);
  assert.match(worker, /password_profile::verify\(/);
  assert.doesNotMatch(worker, /super::super::(?:derive_password_hash|verify_password)\s*\(/);
  const legacy = await source("crypto.inc.rs");
  assert.doesNotMatch(legacy, /password_profile/);
});
