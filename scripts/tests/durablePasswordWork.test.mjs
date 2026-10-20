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
    ["sessions.rs", { derive_password_hash: 0, verify_password: 0 }],
    ["login_transaction.rs", { derive_password_hash: 0, verify_password: 1 }],
    ["credentials.rs", { derive_password_hash: 1, verify_password: 1 }],
    ["bootstrap.rs", { derive_password_hash: 1, verify_password: 0 }],
  ];
  for (const [file, expected] of operations) {
    const contents = await source(`durable_source/${file}`);
    assert.doesNotMatch(contents, /spawn_blocking\s*\(|Argon2::|(?:derive_password_hash_async|verify_password_async)\s*\(/, `${file} must not bypass password admission`);
    assert.doesNotMatch(contents, /\b(?:super|crate)(?:::\w+)*::(?:derive_password_hash|verify_password)\s*\(/, `${file} must not call legacy crypto directly`);
    for (const [operation, count] of Object.entries(expected)) {
      const executable = contents.replace(/\bfn\s+(?:derive_password_hash|verify_password)\s*\(/g, "fn phase(");
      const calls = [...executable.matchAll(new RegExp(`(?<![.\\w])\\b${operation}\\s*\\(`, "g"))];
      assert.equal(calls.length, count, `${file}: every ${operation} call requires deliberate worker coverage`);
      const propagated = [...executable.matchAll(new RegExp(`(?<![.\\w])\\b${operation}\\s*\\([\\s\\S]*?\\)\\s*\\.await\\?`, "g"))];
      assert.equal(propagated.length, count, `${file}: saturation and worker failure must not become wrong-password results`);
    }
  }
  const sessions = await source("durable_source/sessions.rs");
  assert.match(sessions, /PreparedLoginRead::prepare\(&context, &mut tx, username\)\.await\?;\s*tx\.commit\(\)\.await\?;/);
  assert.equal([...sessions.matchAll(/prepared\.verify_password\(&context, password, otp\)\.await\?/g)].length, 1, "the pooled login must invoke exactly the shared fallible password stage");
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
  assert.match(accounts, /account_record\(self\.authority_id, connection, username\)\.await/);
  assert.equal([...accounts.matchAll(/pub\(super\) async fn account_record\(/g)].length, 2, "one original wrapper and one shared record reader, not duplicate validation paths");
  const accountRead = accounts.slice(accounts.lastIndexOf("pub(super) async fn account_record("));
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

test("missing-account work preserves the absent snapshot through verification and cannot authorize writes", async () => {
  const sessions = await source("durable_source/sessions.rs");
  const login = sessions.slice(sessions.indexOf("pub async fn login("), sessions.indexOf("pub(super) async fn session_record("));
  const snapshotCall = login.indexOf("PreparedLoginRead::prepare(&context, &mut tx, username).await?");
  const commit = login.indexOf("tx.commit().await?", snapshotCall);
  const verifyCall = login.indexOf("prepared.verify_password(&context, password, otp).await?", commit);
  const writer = login.indexOf("self.transaction().await?", verifyCall);
  assert.ok(snapshotCall >= 0 && snapshotCall < commit && commit < verifyCall && verifyCall < writer,
    "the original Source read commits before the fallible shared verifier, and the writer follows it");
  const shared = await source("durable_source/login_transaction.rs");
  const readStage = shared.slice(shared.indexOf("impl PreparedLoginRead"), shared.indexOf("pub(crate) struct VerifiedLogin"));
  assert.match(readStage, /let verified = accounts::account_record\(context\.authority_id, tx, username\)\.await\?;/);
  const snapshot = readStage.indexOf("let verified = accounts::account_record");
  const pin = readStage.indexOf("pin.verify_for", snapshot);
  const frozen = readStage.indexOf("Ok(Self", pin);
  const selection = readStage.indexOf("password_profile::MISSING_ACCOUNT_HASH", frozen);
  const work = readStage.indexOf("verify_password(password, salt, hash).await?", selection);
  const realAccount = readStage.indexOf("self.verified.ok_or(DurableAuthError::InvalidCredentials)?", work);
  const otp = readStage.indexOf("otp::verify_at", realAccount);
  const verified = readStage.indexOf("Ok(VerifiedLogin", otp);
  assert.ok(snapshot >= 0 && snapshot < pin && pin < frozen && frozen < selection && selection < work
    && work < realAccount && realAccount < otp && otp < verified,
  "only genuine absence selects dummy work; the frozen account remains mandatory after verification");
  assert.doesNotMatch(readStage.slice(snapshot, work), /\.unwrap_or_default\(|\.ok\(\)|derive_password_hash\(/);
  assert.match(readStage, /let password_matches = verify_password/);
  assert.match(readStage, /if !password_matches\s*\|\| !otp::verify_at/);
  assert.doesNotMatch(readStage.slice(work), /account_record\(|INSERT INTO sessions|\.begin\(/, "password-stage completion cannot recapture a newly registered account or write");
  const profile = await source("durable_source/password_profile.rs");
  assert.match(profile, /pub\(super\) const MISSING_ACCOUNT_HASH: &str/);
  assert.equal((profile.match(/MISSING_ACCOUNT_HASH/g) ?? []).length, 1, "fixed PHC must not be derived on first use");
  const legacy = await source("crypto.inc.rs");
  assert.doesNotMatch(legacy, /MISSING_ACCOUNT_HASH/);
});
