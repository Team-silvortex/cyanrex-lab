import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const directory = "engine/src/platform_http/";
const read = path => readFile(new URL(directory + path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const production = async path => code(await read(path))
  .replace(/#\[cfg\(test\)\]\s*use[^;]+;\s*/g, "")
  .split("#[cfg(test)]")[0];
const commonNames = ["grant_id", "upload_id", "file_id", "version_id", "byte_length", "sha256"];
const inventory = source => {
  const call = source.match(/file_selectors::parse\(\s*query,\s*\[([\s\S]*?)\]\s*,?\s*\)\?/);
  assert.ok(call, "each adapter declares its own fixed raw selector inventory");
  return [...call[1].matchAll(/"([a-z_][a-z0-9_]*)"/g)].map(match => match[1]);
};

// Architecture/composition only. Compiled contract tests and explicit Source SQL prove behavior.
// Initial Red records duplicate responsibilities, not a demonstrated behavioral defect.
test("raw file selector mechanics are private pure borrowed fixed-width parsing", async () => {
  const helper = await production("file_selectors.rs");
  const module = code(await read("mod.rs"));
  assert.match(module, /(?:^|\n)#\[cfg\(unix\)\]\s*mod file_selectors;/);
  assert.doesNotMatch(module, /pub(?:\([^)]*\))?\s+(?:mod|use)\s+file_selectors\b/);
  assert.match(helper, /pub\(super\) const MAX_QUERY_BYTES: usize = 512;/);
  assert.match(helper, /pub\(super\) fn parse<'a,\s*const N: usize>\(\s*query: Option<&'a str>,\s*names: \[&str; N\],?\s*\) -> Result<\[&'a str; N\], HttpError>/);
  assert.match(helper, /\[""; N\]/);
  assert.doesNotMatch(helper, /\b(?:Vec|String|HashMap|BTreeMap|HashSet|BTreeSet|Value|Arc|Semaphore|OnceLock|LazyLock)\b|\.collect\(|to_owned|to_string|\.clone\(|\.await|async\s|\bstatic\b|std::(?:fs|net|env)|tokio::|sqlx|serde_json|serde_urlencoded|percent_decode|form_urlencoded/);
});

test("one raw parser retains byte cap duplicate rejection exact names and no decoder", async () => {
  const helper = await production("file_selectors.rs");
  for (const phrase of ["query.ok_or_else(HttpError::invalid_request)?", "query.is_empty()", "query.len() > MAX_QUERY_BYTES", "query.contains(['%', '+'])", "query.split('&')", ".split_once('=')", "value.is_empty()", "value.contains('=')", "HttpError::invalid_request", "Ok(fields)"])
    assert.ok(helper.includes(phrase), phrase);
  assert.match(helper, /names\s*\.iter\(\)\s*\.position\(/);
  assert.match(helper, /!fields\[index\]\.is_empty\(\)/);
  assert.match(helper, /fields\[index\] = value;/);
  assert.match(helper, /fields\.iter\(\)\.any\([^\n]*\.is_empty\(\)/);
  assert.ok(helper.indexOf("!fields[index].is_empty()") < helper.indexOf("fields[index] = value;"));
  assert.doesNotMatch(helper, /unwrap|expect\(|\.or_else\(|normalize|decode|\.trim\(|\.to_lowercase\(/);
});

test("canonical decimal is one shared unsigned lexical and overflow boundary", async () => {
  const helper = await production("file_selectors.rs");
  const decimal = helper.slice(helper.indexOf("pub(super) fn decimal("));
  assert.match(decimal, /pub\(super\) fn decimal\(value: &str\) -> Result<u64, HttpError>/);
  for (const phrase of ["value.is_empty()", "value.len() > 1", "value.starts_with('0')", "value.bytes().all(|byte| byte.is_ascii_digit())", "value.parse().map_err(|_| HttpError::invalid_request())"])
    assert.ok(decimal.includes(phrase), phrase);
  assert.doesNotMatch(decimal, /MAX_FILE_TRANSFER|chunk_index|byte_length|Sha256Digest|FileVersionRef|workspace|scope|serde|saturating|wrapping|unwrap/);
});

test("manifest and chunk retain distinct selector inventories typed identities limits and host scope", async () => {
  await production("file_selectors.rs");
  const manifest = await production("file_download_manifest/contract.rs");
  const chunk = await production("file_chunk_read/contract.rs");
  assert.deepEqual(inventory(manifest), commonNames);
  assert.deepEqual(inventory(chunk), [...commonNames, "chunk_index"]);
  for (const source of [manifest, chunk]) {
    for (const phrase of ["FileGrantId", "FileUploadId", "FileId", "FileVersionId", "Sha256Digest", "FileVersionRef", "MAX_FILE_TRANSFER_BYTES", '"workspace": scope', "SessionFileDownloadRequest", "file_selectors::decimal("])
      assert.ok(source.includes(phrase), phrase);
    assert.match(source, /byte_length > MAX_FILE_TRANSFER_BYTES/);
    assert.doesNotMatch(source, /query\.(?:split|contains)|split_once|fields\[index\]|fn decimal\(|starts_with\('0'\)|byte\.is_ascii_digit\(\)|const MAX_QUERY_BYTES|owner_id|recipient_id|namespace|percent_decode|serde_urlencoded|std::fs|sqlx|tokio::spawn/);
  }
  assert.equal((manifest.match(/file_selectors::decimal\(/g) || []).length, 1);
  assert.equal((chunk.match(/file_selectors::decimal\(/g) || []).length, 2);
  assert.doesNotMatch(manifest, /chunk_index|MAX_FILE_TRANSFER_CHUNKS/);
  assert.match(chunk, /chunk_index >= MAX_FILE_TRANSFER_CHUNKS as u64/);
  assert.match(chunk, /u32::try_from\(chunk_index\)\.map_err\(\|_\| HttpError::invalid_request\(\)\)/);
});

test("shared raw parsing does not absorb responses Source calls or binary owner lifetimes", async () => {
  const helper = await production("file_selectors.rs");
  assert.doesNotMatch(helper, /Body|Response|Header|StatusCode|SessionFile|FileVersion|Sha256|scope|workspace|admission|source|chunk|manifest|first_frame|Bytes|\.begin\(|\.connect\(/);
  const response = await production("file_download_manifest/response.rs");
  for (const phrase of ["serde_json::to_vec(manifest)", "MAX_FILE_TRANSFER_MANIFEST_BYTES", "Body::from(encoded)", "application/json"])
    assert.ok(response.includes(phrase), phrase);
  const chunk = await production("file_chunk_read/contract.rs");
  for (const phrase of ["pub(super) fn response_headers(chunk: &SessionFileChunkRead)", "let file = chunk.file()", "chunk.chunk_sha256()", "application/octet-stream", "chunk.bin"])
    assert.ok(chunk.includes(phrase), phrase);
  const body = await production("file_chunk_read/body.rs");
  for (const phrase of ["chunk: SessionFileChunkRead", "OwnedSemaphorePermit", "Bytes::from_owner", "data: Option<Bytes>", "self.data.take()", "Instant::now() >= self.deadline", "drop(data)"])
    assert.ok(body.includes(phrase), phrase);
  for (const [path, command] of [["file_download_manifest/handlers.rs", "get_session_file_download_manifest"], ["file_chunk_read/handlers.rs", "read_session_file_chunk"]]) {
    const source = await production(path);
    assert.equal((source.match(new RegExp(`\\.${command}\\(`, "g")) || []).length, 1, path);
    assert.doesNotMatch(source, /file_selectors|sqlx|\.begin\(|\.commit\(|std::(?:fs|net|env)|tokio::(?:spawn|fs)|reqwest/);
  }
});
