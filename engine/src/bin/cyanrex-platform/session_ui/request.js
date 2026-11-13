// Mounted same-origin transport only. Callers still own strict JSON/receipt and current-epoch checks.
// A stopped wait cannot establish server rollback, authorize replay or select another credential source.
const encoder = new TextEncoder();
const UUID = '[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}';
const ZERO = '00000000-0000-0000-0000-000000000000';
const taskPath = new RegExp(`^/platform/v1/tasks/(${UUID})(/content|/text-artifacts)?$`);
const indexPath = new RegExp(`^/platform/v1/task-index\\?limit=20(?:&after=(${UUID}))?$`);
const unknown = deniedStatus => {
  const error = new Error('platformRequest.unknown');
  if (deniedStatus === 401 || deniedStatus === 403) error.deniedStatus = deniedStatus;
  return error;
};

function profile(path, method, body) {
  if (typeof path !== 'string' || typeof method !== 'string') throw unknown();
  if (path === '/platform/v1/session' && method === 'GET' && body === undefined)
    return { bytes: 4096, frames: 4096 };
  if (path === '/platform/v1/session/logout' && method === 'POST' && body === undefined)
    return { bytes: 4096, frames: 4096 };
  if (path === '/platform/v1/session/login' && method === 'POST' && typeof body === 'string'
    && encoder.encode(body).length <= 32 * 1024) return { bytes: 4096, frames: 4096 };
  const index = indexPath.exec(path);
  if (index && index[1] !== ZERO && method === 'GET' && body === undefined)
    return { bytes: 128 * 1024, frames: 16_384 };
  const task = taskPath.exec(path);
  if (task && task[1] !== ZERO) {
    if (!task[2] && method === 'GET' && body === undefined)
      return { bytes: 50 * 1024 * 1024, frames: 16_384 };
    if (task[2] === '/content' && method === 'PUT' && typeof body === 'string'
      && encoder.encode(body).length <= 64 * 1024) return { bytes: 128 * 1024, frames: 4096 };
    if (task[2] === '/text-artifacts' && method === 'POST' && typeof body === 'string'
      && encoder.encode(body).length <= 2 * 1024 * 1024) return { bytes: 128 * 1024, frames: 4096 };
  }
  throw unknown();
}

function options(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw unknown();
  const prototype = Object.getPrototypeOf(value), descriptors = Object.getOwnPropertyDescriptors(value);
  if ((prototype !== Object.prototype && prototype !== null)
    || Reflect.ownKeys(value).some(key => typeof key !== 'string'
      || !['body', 'signal', 'onHandedOff', 'onDenied'].includes(key))
    || Object.values(descriptors).some(property => !('value' in property))) throw unknown();
  const { body, signal, onHandedOff, onDenied } = value;
  if ((signal !== undefined && !(signal instanceof AbortSignal))
    || (onHandedOff !== undefined && typeof onHandedOff !== 'function')
    || (onDenied !== undefined && typeof onDenied !== 'function')) throw unknown();
  if (signal !== undefined) {
    if (Object.getPrototypeOf(signal) !== AbortSignal.prototype
      || ['aborted', 'addEventListener', 'removeEventListener'].some(key => Object.hasOwn(signal, key))) throw unknown();
    // instanceof alone admits a forged prototype. Check the native brand without running a caller getter.
    Object.getOwnPropertyDescriptor(AbortSignal.prototype, 'aborted').get.call(signal);
  }
  return { body, signal, onHandedOff, onDenied };
}

function cancelBody(response) {
  try { void response.body?.cancel().catch(() => undefined); } catch { /* no fallback or private error */ }
}

/** Fixed routes only: no arbitrary URL, Cookie/header injection, unbound publication or retries. */
export async function platformRequest(path, method, config = {}) {
  let deniedStatus;
  let selected, budget;
  try { selected = options(config); budget = profile(path, method, selected.body); }
  catch { throw unknown(); }
  const { body, signal, onHandedOff, onDenied } = selected;
  if (signal?.aborted) throw unknown();
  const own = new AbortController();
  let rejectAbort;
  const interrupted = new Promise((_resolve, reject) => { rejectAbort = reject; });
  const abort = () => rejectAbort(unknown(deniedStatus));
  const externalAbort = () => own.abort();
  own.signal.addEventListener('abort', abort, { once: true });
  signal?.addEventListener('abort', externalAbort, { once: true });
  if (signal?.aborted) own.abort();
  // Bounds browser waiting only; it is not a transaction deadline or synchronous parser deadline.
  const timer = setTimeout(() => own.abort(), 10_000);
  const work = (async () => {
    own.signal.throwIfAborted();
    let pending, handoffFailed = false;
    try {
      pending = fetch(path, { method, mode: 'same-origin', credentials: 'same-origin',
        cache: 'no-store', redirect: 'error', signal: own.signal,
        headers: { 'X-Cyanrex-Platform-Request': '1',
          ...(method === 'POST' || method === 'PUT' ? { 'Content-Type': 'application/json' } : {}) },
        ...(body === undefined ? {} : { body }) });
    } finally {
      try { onHandedOff?.(); } catch { handoffFailed = true; own.abort(); }
    }
    if (handoffFailed) {
      // Even an exceptional caller hook must consume a pending rejection/cancel a late body.
      void Promise.resolve(pending).then(cancelBody).catch(() => undefined);
      throw unknown();
    }
    const response = await pending;
    if (response.status === 401 || response.status === 403) deniedStatus = response.status;
    if (own.signal.aborted) { cancelBody(response); own.signal.throwIfAborted(); }
    // Callers can hide private DOM at denial headers, before collecting even an unusable error body.
    if (deniedStatus !== undefined) {
      try { onDenied?.(deniedStatus); } catch { cancelBody(response); throw unknown(deniedStatus); }
    }
    if (own.signal.aborted) { cancelBody(response); own.signal.throwIfAborted(); }
    if (!/^application\/json(?:;\s*charset=utf-8)?$/i.test(response.headers.get('content-type') || '') || !response.body) {
      cancelBody(response); throw unknown(deniedStatus);
    }
    let guarded, reader;
    try { guarded = response.clone(); reader = guarded.body.getReader(); }
    catch { cancelBody(guarded); cancelBody(response); throw unknown(deniedStatus); }
    const chunks = [];
    let size = 0, frames = 0;
    // Cancel both tee branches without awaiting either: one cancellation can wait for
    // the other branch. This also exits synthetic transports that ignore fetch's signal.
    const cancel = () => {
      try { void reader.cancel().catch(() => undefined); } catch { /* fixed failure only */ }
      cancelBody(response);
    };
    own.signal.addEventListener('abort', cancel, { once: true });
    try {
      own.signal.throwIfAborted();
      while (true) {
        const { value, done } = await reader.read(); own.signal.throwIfAborted();
        if (done) break;
        size += value.byteLength;
        if (++frames > budget.frames || size > budget.bytes) { cancel(); throw unknown(deniedStatus); }
        chunks.push(value);
      }
      const bytes = new Uint8Array(size); let offset = 0;
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
      const raw = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
      chunks.length = 0;
      own.signal.throwIfAborted();
      // Only the guarded branch drives network input. The original native consumer starts
      // after complete bounded EOF, preserving Chromium's Fetch body completion lifecycle.
      const native = new Uint8Array(await response.arrayBuffer()); own.signal.throwIfAborted();
      if (native.byteLength !== size || bytes.some((value, index) => value !== native[index]))
        throw unknown(deniedStatus);
      return Object.freeze({ status: response.status, raw });
    } catch (error) {
      cancel(); throw error;
    } finally { own.signal.removeEventListener('abort', cancel); reader.releaseLock(); }
  })();
  try { return await Promise.race([work, interrupted]); }
  catch { own.abort(); throw unknown(deniedStatus); }
  finally {
    clearTimeout(timer); own.signal.removeEventListener('abort', abort);
    signal?.removeEventListener('abort', externalAbort);
  }
}
