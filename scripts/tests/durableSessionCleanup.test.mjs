import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = file => readFile(new URL(`../../engine/src/services/auth_service/durable_source/${file}`, import.meta.url), "utf8");

test("expired Session cleanup pins a bounded complete batch and checks every deleted coordinate", async () => {
  const source = await read("session_cleanup.rs");
  assert.match(source, /const BATCH_SIZE: i64 = 128;/);
  assert.match(source, /CASE WHEN octet_length\(s\.token\) = 64 THEN s\.token END/);
  assert.match(source, /CASE WHEN octet_length\(s\.username\) <= 64 THEN s\.username END/);
  assert.match(source, /s\.account_id,/);
  for (const field of ["expires_at", "created_at"]) {
    assert.ok(source.includes(`CASE WHEN isfinite(s.${field}) AND s.${field} <= $3 THEN s.${field} END AS ${field}`));
    assert.ok(source.includes(`try_get::<Option<DateTime<Utc>>, _>("${field}")`));
  }
  assert.equal((source.match(/\.bind\(DateTime::<Utc>::MAX_UTC\)/g) ?? []).length, 2);
  assert.match(source, /EXISTS \(SELECT 1 FROM users u WHERE u\.username = s\.username AND u\.account_id = s\.account_id\)/);
  assert.match(source, /ORDER BY s\.expires_at, s\.token LIMIT \$2 FOR UPDATE OF s/);
  assert.match(source, /WHERE s\.token = ANY\(\$1\) AND s\.expires_at <= \$2 RETURNING \{PROJECTION\}/);
  const pin = source.indexOf("SessionSourcePin::capture(self, &mut tx, true).await?");
  const cutoff = source.indexOf("let cutoff = Self::now(&mut tx).await?", pin);
  const decodeAll = source.indexOf(".collect::<Result<Vec<_>>>()?", cutoff);
  const deletion = source.indexOf("DELETE FROM sessions", decodeAll);
  const recheck = source.indexOf("pin.verify(self, &mut tx, true).await?", deletion);
  const exactRows = source.indexOf("if actual != expected", recheck);
  const absence = source.indexOf("SELECT EXISTS (SELECT 1 FROM sessions WHERE token = ANY($1)) AS remains", exactRows);
  const finalPin = source.indexOf("pin.verify(self, &mut tx, true).await?", absence);
  const commit = source.indexOf("tx.commit().await?", finalPin);
  assert.ok(pin >= 0 && pin < cutoff && cutoff < decodeAll && decodeAll < deletion && deletion < recheck
    && recheck < exactRows && exactRows < absence && absence < finalPin && finalPin < commit);
  assert.equal((source.match(/Self::now\(/g) ?? []).length, 1, "one fixed cutoff, including deletion");
  assert.equal((source.match(/sort_unstable_by/g) ?? []).length, 2, "compare full unordered RETURNING rows");
  assert.doesNotMatch(source, /password_hash|totp_secret|otp_last_counter|DELETE FROM users|UPDATE users/);
});

test("expired Session cleanup stays explicit and does not turn validation or login into deletion", async () => {
  const source = await read("session_cleanup.rs");
  assert.match(source, /pub async fn prune_expired_sessions\(&self\) -> Result<u32>/);
  assert.match(source, /bounded\(async/);
  assert.doesNotMatch(source, /tokio::spawn|std::env|pub struct ExpiredSession|derive\([^)]*(?:Debug|Serialize)/);
  const sessions = await read("sessions.rs");
  const login = sessions.slice(sessions.indexOf("pub async fn login("), sessions.indexOf("pub(super) async fn session_record("));
  const validation = sessions.slice(sessions.indexOf("pub async fn validate_session("), sessions.indexOf("pub async fn logout("));
  for (const path of [login, validation]) assert.doesNotMatch(path, /prune_expired_sessions|DELETE FROM/);
  const module = await read("mod.rs");
  assert.match(module, /^mod session_cleanup;/m);
  assert.doesNotMatch(module, /pub(?:\([^)]*\))?\s+mod session_cleanup/);
});
