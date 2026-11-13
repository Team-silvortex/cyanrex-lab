// Source/runner inventory only: none of these checks establishes a live SQL or TLS pass.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const root = new URL('../../', import.meta.url);
const read = file => readFile(new URL(file, root), 'utf8');
const selectors = ['plain', 'tls'].map(transport =>
  `behavior::postgres_platform_browser_task_body_${transport}_confirmed_replace_conflict_and_revocation`);
const partialSelectors = [
  'partial::postgres_platform_browser_task_body_partial_prefix_second_lock_timeout_retains_confirmed_artifact',
  'partial::postgres_platform_browser_task_body_partial_prefix_second_session_revoked_retains_confirmed_artifact',
];

test('real body runner selects independent plain and TLS cases explicitly, without fallback or provisioning', async () => {
  const runner = await read('scripts/test-platform-browser-task-body.sh');
  for (const selector of selectors) assert.equal(runner.split(selector).length - 1, 1);
  for (const value of ['CYANREX_TEST_DATABASE_URL:?', 'CYANREX_PLAYWRIGHT_MODULE:?', 'CYANREX_CHROMIUM_PATH:?',
    '--locked --test platform_browser_task_body_tdd', '--ignored --list', 'grep -Fx', '--ignored --exact'])
    assert.ok(runner.includes(value), value);
  assert.doesNotMatch(runner, /initdb|pg_ctl|\bGRANT\b|\bCREATE\s+(?:ROLE|DATABASE)|\bHOME=|ignoreHTTPSErrors|\|\|\s*true|\bretry\b/);
  const bootstrap = await read('scripts/test-platform-browser-ci.sh');
  assert.equal(bootstrap.split('bash scripts/test-platform-browser-task-body.sh').length - 1, 1);
  assert.equal(bootstrap.split('node --test scripts/tests/platformRequestCdp.browser.mjs').length - 1, 1);
  for (const prior of ['session-ui-browser', 'task-reader-browser', 'task-metadata-browser',
    'task-body-browser', 'browser-session', 'browser-tasks', 'browser-task-metadata'])
    assert.equal(bootstrap.split(`bash scripts/test-platform-${prior}.sh`).length - 1, 1, prior);
});

test('both real partial-prefix cases are mandatory explicit selections alongside the original two', async () => {
  const runner = await read('scripts/test-platform-browser-task-body.sh');
  for (const selector of [...selectors, ...partialSelectors])
    assert.equal(runner.split(selector).length - 1, 1, selector);
  const entry = await read('engine/tests/platform_browser_task_body_tdd.rs');
  assert.match(entry, /platform_browser_task_body\/partial\.rs/);
  const partial = await read('engine/tests/platform_browser_task_body/partial.rs');
  assert.equal([...partial.matchAll(/#\[tokio::test\]/g)].length, 2);
  assert.equal([...partial.matchAll(/#\[ignore = /g)].length, 2);
  for (const selector of partialSelectors)
    assert.match(partial, new RegExp(`async fn ${selector.split('::')[1]}\\(\\s*\\)`));
  const waits = await read('engine/tests/platform_browser_task_body/partial-waits.rs');
  for (const value of ['pg_catalog.pg_blocking_pids', "application_name='cyanrex-platform-connection'",
    'wait_event_type=\'Lock\'', 'FOR UPDATE', 'SELECT pg_backend_pid()', 'self.pending.abort()',
    'DELETE FROM {}.sessions WHERE token=$1', 'ROLLBACK', 'COMMIT']) assert.ok(waits.includes(value), value);
  assert.doesNotMatch(partial + waits, /CREATE\s+(?:FUNCTION|TRIGGER)|pg_terminate_backend|ALTER\s+(?:ROLE|TABLE)|\.route\(/);
});

test('in-flight SQL fences never wait for response-bound Playwright headers before releasing the owner', async () => {
  const browser = await read('engine/tests/platform_browser_task_body/partial-browser.mjs');
  const handler = browser.slice(browser.indexOf('page.on("request", request =>'), browser.indexOf('page.on("response", response =>'));
  const body = handler.indexOf('record.body = JSON.parse(request.postData())');
  const notification = handler.indexOf('secondRequest()');
  const headers = handler.indexOf('await request.allHeaders()');
  assert.ok(body >= 0 && notification > body && headers > notification);
  const prefix = browser.slice(browser.indexOf('const incoming = responses.expectMany('), browser.indexOf('emit({ phase: "first_confirmed"'));
  assert.ok(prefix.includes('await incoming[0]') && prefix.includes('await secondRequested'));
  assert.doesNotMatch(prefix, /await observations\.drain\(|await Promise\.all\(requestChecks\)/);
  assert.ok(handler.includes('observations.track("request-contract", () => checked)'));
  assert.ok(browser.includes('await clean(); await flush(); emit({ phase: "terminal" })'));
});

test('original Session SQL browser recognizes every compiled static dependency with the default capability disabled', async () => {
  const host = await read('engine/src/bin/cyanrex-platform/session_ui.rs');
  const paths = [...host.matchAll(/\.route\(\s*"([^"]+)"/g)].map(match => match[1]);
  assert.equal(paths.length, 13);
  const browser = await read('engine/tests/platform_browser_session/browser.mjs');
  const allowlist = browser.match(/const paths = new Set\(\[([\s\S]+?)\]\);/)[1];
  for (const path of paths) assert.ok(allowlist.includes(`"${path}"`), path);
  assert.ok(browser.includes('getAttribute("data-task-text-publication"), "disabled"'));
  assert.match(browser, /ignoreHTTPSErrors: false/);
  assert.doesNotMatch(browser, /ignore-certificate-errors|ignoreHTTPSErrors:\s*true|\.route\(|addCookies\(/);
});

test('body storage cases remain ignored separate transports and reject legacy TLS trust before database setup', async () => {
  const entry = await read('engine/tests/platform_browser_task_body_tdd.rs');
  const behavior = await read('engine/tests/platform_browser_task_body/behavior.rs');
  assert.match(entry, /#!\[cfg\(target_os = "linux"\)\]/);
  assert.match(entry, /platform_host_preflight\/fixture\.rs/);
  assert.equal([...behavior.matchAll(/#\[tokio::test\]/g)].length, 2);
  assert.equal([...behavior.matchAll(/#\[ignore = /g)].length, 2);
  for (const selector of selectors) assert.ok(behavior.includes(`async fn ${selector.split('::')[1]}()`));
  assert.match(behavior, /exercise\(false\)\.await/); assert.match(behavior, /exercise\(true\)\.await/);
  assert.ok(behavior.indexOf('home.join(".pki/nssdb")') < behavior.indexOf('Fixture::checked('));
  for (const value of ['"ca_rejected"', '"authenticated"', '"saved"', '"conflict_ready"',
    '"conflict"', '"revoked_ready"', '"denied"', '"complete"', 'safe_threads(', 'stopped(',
    'storage::assert_saved(', 'storage::exact_profile(', 'p.assert_head_event(', 'no_connections(f)', 'p.finish()'])
    assert.ok(behavior.includes(value), value);
  assert.doesNotMatch(behavior, /eprintln!|println!|pg_terminate_backend|DROP\s+(?:ROLE|DATABASE)/);
});

test('the default-off UI convenience constructor stays test-only rather than unused production composition', async () => {
  const host = await read('engine/src/bin/cyanrex-platform/session_ui.rs');
  assert.match(host, /#\[cfg\(test\)\]\s*pub\(super\) fn routes\(origin: &str\)/);
  assert.match(host, /fn routes\(origin: &str\) -> Router \{\s*with_task_text_publication\(origin, false\)/);
});

test('real body browser retains natural credentials, one-shot passive observations and exact independent receipts', async () => {
  const browser = await read('engine/tests/platform_browser_task_body/browser.mjs');
  for (const value of ['ignoreHTTPSErrors: false', '"ERR_CERT_AUTHORITY_INVALID"', '".pki/nssdb"',
    'XDG_DATA_HOME: data', '"certutil"', 'responses.expectMany(', 'responses.check()', 'context.newCDPSession(page)',
    'cookie.secure, config.tls', 'cookie.httpOnly, true', 'cookie.sameSite, "Strict"',
    'record.body.expected, { task: baseline.task, manifest: baseline.manifest }',
    'artifact.title, baseline.task.title', 'reply.status, 201', 'replaced[0].status, 200',
    '["POST", "POST", "PUT"]', 'expected_revision: 1', '"cancel-save-metadata"',
    '"conflict_ready"', '"revoked_ready"', '"quarantined"']) assert.ok(browser.includes(value), value);
  assert.doesNotMatch(browser, /ignoreHTTPSErrors:\s*true|ignore-certificate-errors|\.route\(|route\.fulfill|addCookies\(|setExtraHTTPHeaders\(|replayXHR|loadNetworkResource|\bfetch\(/);
  assert.doesNotMatch(browser, /emit\([^\n]*(?:error\.message|error\.stack|reply\.body|config)/);
  const host = await read('engine/src/bin/cyanrex-platform/session_ui.rs');
  const paths = [...host.matchAll(/\.route\(\s*"([^"]+)"/g)].map(match => match[1]);
  const allowed = browser.match(/const staticPaths = new Set\(\[([\s\S]+?)\]\);/)[1];
  for (const path of paths) assert.ok(allowed.includes(`"${path}"`), path);
});

test('independent SQL and actual-file oracles do not mistake browser acknowledgement for storage provenance', async () => {
  const fixture = await read('engine/tests/platform_browser_task_body/fixture.rs');
  const storage = await read('engine/tests/platform_browser_task_body/storage.rs');
  for (const value of ['.mode(0o600)', '"serve-loopback-tls"', '"serve-loopback"',
    '["--task-text-publication", "--session-ui"]', 'clear_child_capabilities(&mut command)',
    'tls.unchanged(bytes)', '"collaboration_artifacts"', '"collaboration_artifact_revisions"',
    '"collaboration_artifact_outbox"']) assert.ok(fixture.includes(value), value);
  assert.match(fixture, /GRANT INSERT ON/);
  assert.doesNotMatch(fixture, /GRANT\s+ALL|GRANT\s+(?:SUPERUSER|CREATEDB|CREATEROLE)|pg_terminate_backend/);
  for (const value of ['original_artifacts.len() + 6', 'original_files.len() + 2',
    'original_artifacts.iter().all(|old| rows.contains(old))', 'original_files.iter().all(|old| files.contains(old))',
    'Sha256::digest(SAVED_TEXTS[index].as_bytes())', '"cyanrex.task-text"', '"text/plain"',
    'original.task.title', '0o400', 'metadata.nlink(), 1', 'fs::read(path).unwrap(), SAVED_TEXTS[index].as_bytes()',
    'has_table_privilege', 'has_column_privilege', 'saved.task.input_refs, saved.manifest.input_refs()'])
    assert.ok(storage.includes(value), value);
});
