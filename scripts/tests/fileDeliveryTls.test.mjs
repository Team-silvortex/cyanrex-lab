import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = s => s.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const client = "engine/src/services/file_chunk_client/";

// Inventory/composition only. The explicit native OpenSSL runner establishes TLS behavior.
test("whole TLS delivery has three explicit ignored cases and exact native runner inventory", async () => {
  const main = await read("engine/tests/file_delivery_tls_tdd.rs");
  assert.match(main, /#!\[cfg\(unix\)\]/);
  const ignored = [];
  let total = 0;
  for (const file of await readdir(new URL("engine/tests/file_delivery_tls/", root))) {
    if (!file.endsWith(".rs")) continue;
    const s = await read(`engine/tests/file_delivery_tls/${file}`);
    const module = file.replace(/\.rs$/, "");
    const names = [...s.matchAll(/#\[ignore[^\n]*\][\s\S]*?async fn (openssl_\w+)\(/g)].map(m => m[1]);
    total += (s.match(/#\[(?:tokio::)?test\]/g) || []).length;
    if (names.length) assert.match(main, new RegExp(`\\bmod ${module};`));
    ignored.push(...names.map(name => `${module}::${name}`));
  }
  assert.equal(total, 3);
  assert.equal(ignored.length, total, "zero default cases pretending to run OpenSSL");
  const runner = await read("scripts/test-file-delivery-tls.sh");
  const inventory = [...runner.matchAll(/\b\w+::openssl_\w+/g)].map(m => m[0]);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.deepEqual(inventory.sort(), ignored.sort());
  for (const pattern of [/command -v openssl/, /--test file_delivery_tls_tdd/, /--ignored --list/, /grep -Fx/, /--ignored --exact --nocapture/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /DATABASE_URL|sudo|update-ca-certificates|git push|curl/);
  assert.ok((await read(".github/workflows/ci.yml")).includes("bash scripts/test-file-delivery-tls.sh"));
});

test("native TLS verification keeps the original production peer and transport policies", async () => {
  const s = code(await read(`${client}policy.rs`));
  for (const phrase of ["FileChunkTransport::Https", ".use_rustls_tls()", ".https_only(", "reqwest::redirect::Policy::none()", "reqwest::retry::never()", ".no_proxy()", ".no_gzip()", ".no_brotli()", ".no_deflate()", ".no_zstd()", ".http1_only()", ".pool_max_idle_per_host(0)", "reqwest::Certificate::from_pem_bundle", "builder.add_root_certificate(certificate)"]) assert.ok(s.includes(phrase), phrase);
  assert.doesNotMatch(s, /danger_accept|insecure|tls_built_in_root_certs\(false\)|cookie_store|identity\(|rustls::ClientConfig/);
  const controlled = code(await read(`${client}controlled.rs`)).split("\n#[cfg(test)]")[0];
  assert.ok(controlled.includes("delivery::run_observed("));
  assert.match(controlled, /biased;\s*_ = cancellation\.requested\(\) => Err\(FileControlledSaveError::Cancelled\)/);
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(code(await read(path)), /file_delivery_tls|test-file-delivery-tls|FileDeliveryCancelHandle|fetch_granted_and_save_controlled/, path);
});

test("disposable certificate generation and native pipe ownership are bounded without trust installation", async () => {
  const certificates = await read("engine/tests/file_delivery_tls/certificates.rs");
  for (const phrase of ["CommandGuard", "Command::new(\"openssl\")", ".mode(0o700)", "from_mode(0o600)", "subjectAltName=IP:127.0.0.1", "Duration::from_secs(10)", ".try_wait()", ".kill()", ".wait()", ".stdout(Stdio::null())", ".stderr(Stdio::null())"]) assert.ok(certificates.includes(phrase), phrase);
  assert.doesNotMatch(code(certificates), /sudo|update-ca-certificates|\.output\(|println!|eprintln!/);
  const server = await read("engine/tests/file_delivery_tls/server.rs");
  for (const phrase of ["s_server", "-quiet", "-ign_eof", "impl Drop for", "AtomicUsize", "received.fetch_add", "read_request", "staged_before_request", "libc::O_NONBLOCK", "libc::poll", "deadline", "HoldAt", "Condvar"]) assert.ok(server.includes(phrase), phrase);
  assert.doesNotMatch(code(server), /danger_accept|sudo|update-ca-certificates|sqlx|SessionBind/);
});

test("whole TLS cases use original controlled calls and independent actual bytes rather than progress receipts", async () => {
  const positive = await read("engine/tests/file_delivery_tls/acceptance.rs");
  const success = /result\s*\.expect\(/.exec(positive);
  assert.ok(success && positive.indexOf("checked(") < success.index);
  for (const phrase of ["FILE_TRANSFER_CHUNK_BYTES + 17", "fetch_granted_and_save_controlled", "observe_output", "verify_chunk", "source_entries", "selected_entries", "FileDeliveryPhase::Complete", "0o400", "handle.cancel()", "drop(handle.take())", "assert_request"]) assert.ok(positive.includes(phrase), phrase);
  const security = await read("engine/tests/file_delivery_tls/security.rs");
  for (const phrase of ["Ipv4Addr::new(127, 0, 0, 2)", "FileManifestClientError::Unavailable", "tls.received_bytes, 0", "tls.responded, 0", "requests.is_empty()", "location", "FileManifestClientError::InvalidResponse"]) assert.ok(security.includes(phrase), phrase);
  assert.match(security, /assert_eq!\(\s*poison\.hits\(\),\s*0,/);
  assert.match(security, /let final_hits = poison\.finish\(\)\.await/);
  assert.match(security, /assert_eq!\(\s*final_hits,\s*0,/);
  const cancellation = await read("engine/tests/file_delivery_tls/cancellation.rs");
  for (const phrase of ["handle.cancel()", "FileControlledSaveError::Cancelled", "FileDeliveryPhase::FetchingManifest", "FileDeliveryPhase::StagingChunks", "FILE_TRANSFER_CHUNK_BYTES", "hold", "closed(", "verify_chunk", "ChunkStagingError::Conflict"]) assert.ok(cancellation.includes(phrase), phrase);
});

test("current bilingual HTTPS notes separate transport identity from authorization and deployment", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const s = await read(`docs/${lang}/large-file-delivery-https.md`);
    for (const phrase of ["0.5.7", "0.3.0", "2026-10-13", "OpenSSL", "file_delivery_tls_tdd", "trusted_root_pem", "Cancelled", "Complete", "0700", "test-file-delivery-tls.sh", "fileDeliveryTls", "106", "52", "8"]) assert.ok(s.includes(phrase), `${lang}: ${phrase}`);
    for (const file of ["README", "architecture", "security", "platform-network", "project-status", "testing-guide", "capability-maturity", "large-file-distribution", "large-file-delivery-client", "large-file-delivery-cancellation"]) await assertDocumentationReference(`docs/${lang}/${file}.md`, `docs/${lang}/large-file-delivery-https.md`);
  }
});
