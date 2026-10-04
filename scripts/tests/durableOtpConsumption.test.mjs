import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const authRoot = new URL("../../engine/src/services/auth_service/", import.meta.url);
const read = (file) => readFile(new URL(file, authRoot), "utf8");
// Source-shape checks complement the real SQL consumption tests; they do not prove atomicity alone.
const codeOnly = (source) => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

function block(source, declaration) {
  const match = declaration.exec(source);
  assert.ok(match, `missing expected declaration: ${declaration}`);
  const opening = source.indexOf("{", match.index);
  assert.ok(opening >= 0);
  let depth = 1;
  for (let index = opening + 1; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}" && --depth === 0) return source.slice(opening + 1, index);
  }
  assert.fail("unclosed expected declaration");
}

test("OTP consumption is private prepared source behavior, never a legacy fallback or public switch", async () => {
  const module = codeOnly(await read("durable_source/mod.rs"));
  assert.match(module, /mod otp_consumption\s*;/);
  assert.match(module, /mod otp_storage\s*;/);
  assert.doesNotMatch(module, /#\[allow\(dead_code\)\]\s*mod otp_consumption/);
  assert.doesNotMatch(module, /\bpub(?:\([^)]*\))?\s+(?:mod|use)\s+(?:[^;\n]*::)?otp_consumption\b/);
  assert.match(module, /const SOURCE_SCHEMA_VERSION: i32 = 2;/);
  for (const file of ["../auth_service.rs", "crypto.inc.rs", "service.inc.rs", "account_mutations.inc.rs"]) {
    assert.doesNotMatch(codeOnly(await read(file)), /\botp_consumption\b|\botp_last_counter\b|\bPendingConsumption\b/, `${file}: no incidental live-auth cutover`);
  }
});

test("consumption uses caller-owned compare-and-update and post-write watermark checks in both commands", async () => {
  const storage = codeOnly(await read("durable_source/otp_storage.rs"));
  assert.match(storage, /UPDATE users SET otp_last_counter = \$1/);
  assert.match(storage, /username = \$2 AND account_id = \$3 AND otp_last_counter = \$4/);
  assert.match(storage, /password_salt = \$5 AND password_hash = \$6 AND totp_secret = \$7/);
  assert.match(storage, /confirmed_one\(/);
  assert.doesNotMatch(storage, /\.begin\(|\.commit\(|std::env|OnceLock|LazyLock/);
  const sessions = await read("durable_source/sessions.rs");
  assert.match(sessions, /consume_login_otp\(&mut tx, &current, &pending_otp\)\.await\?/);
  const rotation = await read("durable_source/credentials.rs");
  assert.match(rotation, /UPDATE users SET password_salt[\s\S]*?otp_last_counter = \$8/);
  assert.match(rotation, /AND otp_last_counter = \$9/);
  for (const body of [sessions, rotation]) {
    assert.match(body, /otp_watermark != pending_otp\.next\(\)/);
    assert.match(body, /\.bind\(pending_otp\.(?:previous|next)\(\)\.stored\(\)\)|consume_login_otp/);
    assert.match(body, /SessionSourcePin::capture/);
    assert.match(body, /recheck_otp_consumption\(&pending_otp,/);
  }
});

test("OTP consumption policy has explicit inputs and no storage, environment or current-time effects", async () => {
  const pure = codeOnly(await read("durable_source/otp_consumption.rs"));
  assert.doesNotMatch(pure, /#!?\[allow\([^\]]*dead_code/);
  assert.doesNotMatch(pure, /\bpub\b(?!\s*\(\s*super\s*\))/, "pure preparation must not become a public or crate-wide API");
  assert.doesNotMatch(pure, /\b(?:sqlx|sqlx_core|sqlx_postgres|PgPool|PgConnection|Postgres|Transaction|DurableAuthSource|tokio|reqwest|OnceLock|LazyLock)\b/);
  assert.doesNotMatch(pure, /\b(?:std::)?(?:env|fs|net)::|\bstd::time::|\benv!\s*\(|\boption_env!\s*\(/);
  assert.doesNotMatch(pure, /\b(?:Utc|Local|SystemTime|Instant)::now\s*\(|\b(?:clock_timestamp|CURRENT_TIMESTAMP|current_timestamp)\b/);
  assert.doesNotMatch(pure, /\b(?:async|await|unsafe)\b|\b(?:query|query_as|query_scalar)\s*\(/);
  assert.doesNotMatch(pure, /\b(?:SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP)\s/, "no hidden SQL strings in the pure policy");
});

test("pending consumption cannot expose, copy or serialize its credential binding", async () => {
  const pure = codeOnly(await read("durable_source/otp_consumption.rs"));
  const declaration = /((?:#\[[^\]]*\]\s*)*)pub\(super\)\s+struct\s+PendingConsumption\b/.exec(pure);
  assert.ok(declaration, "the pending transition stays parent-private");
  assert.doesNotMatch(declaration[1], /\b(?:Debug|Clone|Copy|Serialize|Deserialize)\b/);
  assert.doesNotMatch(pure, /\bimpl(?:\s*<[^>]*>)?\s+(?:[\w:]+::)?(?:Debug|Clone|Copy|Serialize|Deserialize)(?:\s*<[^>]*>)?\s+for\s+PendingConsumption\b/);
  const fields = block(pure, /\bstruct\s+PendingConsumption\b/);
  assert.doesNotMatch(fields, /\bpub\b/, "no credential-binding field is directly visible");
  const methods = block(pure, /\bimpl\s+PendingConsumption\b/);
  const visible = [...methods.matchAll(/\bpub(?:\([^)]*\))?\s+fn\s+(\w+)/g)].map((match) => match[1]);
  assert.deepEqual(visible.sort(), ["next", "previous", "recheck"], "new accessors require deliberate review; no binding getter");
  for (const getter of ["previous", "next"]) {
    assert.match(methods, new RegExp(`pub\\(super\\)\\s+fn\\s+${getter}\\s*\\(\\s*&self\\s*\\)\\s*->\\s*Watermark\\b`), `${getter} exposes only a counter, not the secret/OTP binding`);
  }
  assert.match(methods, /\bfn\s+recheck\s*\([\s\S]*?\)\s*->\s*Result<\s*\(\s*\)\s*,\s*ConsumptionError\s*>/, "recheck reports validity only, never credential binding bytes");
});
