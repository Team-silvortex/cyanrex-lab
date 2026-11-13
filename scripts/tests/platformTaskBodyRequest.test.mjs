import assert from 'node:assert/strict';
import test from 'node:test';
import { platformRequest } from '../../engine/src/bin/cyanrex-platform/session_ui/request.js';

const path = '/platform/v1/tasks/00000000-0000-4000-8000-000000000001/text-artifacts';
test('only the exact Task-bound publication route admits a bounded POST, without unbound fallback', async t => {
  const calls = [];
  t.mock.method(globalThis, 'fetch', async (target, options) => {
    calls.push({ target, options });
    return new Response('{}', { status: 201, headers: { 'content-type': 'application/json' } });
  });
  assert.deepEqual(await platformRequest(path, 'POST', { body: '{}' }), { status: 201, raw: '{}' });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].target, path);
  assert.equal(calls[0].options.credentials, 'same-origin');
  assert.equal(calls[0].options.redirect, 'error');
  for (const [target, method, body] of [
    ['/platform/v1/text-artifacts', 'POST', '{}'], [path, 'GET', undefined], [path, 'PUT', '{}'],
    [path + '?', 'POST', '{}'], [path + '/', 'POST', '{}'], [path, 'POST', undefined],
    [path, 'POST', 'é'.repeat(1024 * 1024) + 'a'],
  ]) await assert.rejects(platformRequest(target, method, { body }), { message: 'platformRequest.unknown' });
  assert.equal(calls.length, 1);
  await platformRequest(path, 'POST', { body: 'é'.repeat(1024 * 1024) });
  assert.equal(calls.length, 2);
});

test('Task publication retains its independent bounded reply and immediate denial hook', async t => {
  let denied = 0;
  t.mock.method(globalThis, 'fetch', async () => new Response('x'.repeat(128 * 1024 + 1), {
    status: 403, headers: { 'content-type': 'application/json' },
  }));
  await assert.rejects(platformRequest(path, 'POST', { body: '{}', onDenied(status) {
    assert.equal(status, 403); denied++;
  } }), error => error.message === 'platformRequest.unknown' && error.deniedStatus === 403);
  assert.equal(denied, 1);
});
