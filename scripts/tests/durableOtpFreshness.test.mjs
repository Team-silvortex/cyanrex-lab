import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = (file) => readFile(new URL(`../../engine/src/services/auth_service/${file}`, import.meta.url), "utf8");

function checkFreshnessStages(body, mutation, label, stages = [
  /self\.verify_current_totp\s*\(/g,
  /self\.prepare_otp_consumption\s*\(/g,
  /self\.recheck_otp_consumption\s*\(/g,
]) {
  const calls = stages.map((stage) => {
    const matches = [...body.matchAll(stage)];
    assert.equal(matches.length, 1, `${label}: exactly one ${stage} stage`);
    return matches[0].index;
  });
  const password = body.indexOf("verify_password(");
  const passwordAwait = body.indexOf(".await", password);
  const recheckedCredentials = body.indexOf("same_credentials(", password);
  const write = body.indexOf(mutation);
  assert.ok(password >= 0 && calls[0] > password && calls[0] < recheckedCredentials, `${label}: first OTP check follows password verification`);
  assert.ok(passwordAwait > password && passwordAwait < calls[0], `${label}: await password verification before checking OTP, not merely constructing its future`);
  assert.ok(calls[1] > recheckedCredentials && calls[1] < write, `${label}: pin the consumption proposal after current credential checks and before mutation`);
  const commits = [...body.matchAll(/tx\.commit\s*\(/g)];
  const finalCommit = commits.at(-1)?.index ?? -1;
  assert.ok(calls[2] > write && calls[2] < finalCommit, `${label}: final exact-counter/credential recheck precedes commit`);
  assert.doesNotMatch(body.slice(calls[2], finalCommit), /\.await\b/, `${label}: no further SQL wait may age the final OTP decision before commit`);
  assert.doesNotMatch(body, /\bverify_totp\s*\(/, `${label} must not bypass the source clock boundary`);
}

test("durable login checks current OTP after password work, before write and after final SQL", async () => {
  const sessions = await source("durable_source/sessions.rs");
  const login = sessions.slice(sessions.indexOf("pub async fn login("), sessions.indexOf("pub(super) async fn session_record("));
  assert.match(login, /let context = LoginContext::from_source\(self\)/);
  assert.match(login, /PreparedLoginRead::prepare\(&context, &mut tx, username\)\.await\?;\s*tx\.commit\(\)\.await\?;/);
  assert.match(login, /prepared\.verify_password\(&context, password, otp\)\.await\?;[\s\S]*?verified\.prepare\(&context, &mut tx, otp, limits\)\.await\?;\s*prepared\.commit\(&context, tx, otp\)\.await/);
  const shared = await source("durable_source/login_transaction.rs");
  const protocol = shared.slice(shared.indexOf("let (salt, hash) ="));
  checkFreshnessStages(protocol, "INSERT INTO sessions", "shared login", [
    /otp::verify_at\s*\(/g,
    /otp_consumption::prepare\s*\(/g,
    /self\.pending_otp\s*\.recheck\s*\(/g,
  ]);
  assert.equal([...protocol.matchAll(/context\.now\(\)/g)].length, 3, "each login OTP stage uses the same source clock context");
  assert.match(shared, /#\[cfg\(test\)\]\s+otp_clock\s*:/);
  assert.match(shared, /#\[cfg\(test\)\]\s+otp_clock:\s*source\.otp_clock\.clone\(\)/);
  assert.match(shared, /#\[cfg\(test\)\][\s\S]*?self\.otp_clock/);
});

test("durable password rotation checks current OTP after password work, before write and after final SQL", async () => {
  const credentials = await source("durable_source/credentials.rs");
  const rotation = credentials.slice(credentials.indexOf("pub async fn change_session_password("));
  checkFreshnessStages(rotation, "UPDATE users SET password_salt", "rotation");
});

test("the source-only clock seam is test-only and legacy OTP behavior stays separate", async () => {
  const module = await source("durable_source/mod.rs");
  const otp = await source("durable_source/otp.rs");
  const legacy = await source("crypto.inc.rs");
  assert.match(module, /#\[cfg\(test\)\]\s+otp_clock\s*:/);
  assert.match(module, /#\[cfg\(test\)\]\s+otp_clock\s*:\s*None/);
  assert.doesNotMatch(module, /pub(?:\([^)]*\))?\s+(?:mod\s+otp\b|otp_clock\s*:)/);
  assert.doesNotMatch(`${module}\n${otp}`, /pub(?:\([^)]*\))?\s+fn\s+(?:set|with)_\w*(?:otp|clock)/);
  assert.match(otp, /#\[cfg\(test\)\][\s\S]*?self\.otp_clock/);
  assert.match(otp, /verify_at\(secret, otp, self\.otp_now\(\)\)/);
  assert.doesNotMatch(otp, /std::env|OnceLock|LazyLock/);
  assert.match(legacy, /fn verify_totp\(secret: &str, otp: &str\) -> bool/);
  assert.match(legacy, /Utc::now\(\)\.timestamp\(\)/);
  assert.doesNotMatch(legacy, /verify_current_totp|otp_clock|otp::verify_at/);
});
