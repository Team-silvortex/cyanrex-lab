import { labels, chooseLocale } from './labels.js';
import { createTaskReader } from './tasks.js';

const element = id => document.getElementById(id);
const form = element("session-form");
const fields = { username: element("username"), password: element("password"), otp: element("otp") };
const controls = [element("login"), element("check-session"), element("logout"),
  element("confirm-logout"), element("cancel-logout"), element("locale"), ...Object.values(fields)];
const status = element("status"), dialog = element("logout-dialog"), locale = element("locale");
const encoder = new TextEncoder();
let language = chooseLocale(navigator.languages);
let busy = false, mounted = true, generation = 0, activeRequest = null;
let taskReader = null;

function translate() {
  document.documentElement.lang = language;
  for (const node of document.querySelectorAll("[data-i18n]")) {
    const value = labels[language][node.dataset.i18n];
    if (typeof value === "string") node.textContent = value;
  }
  taskReader?.translate();
}

function observe(state, key) {
  status.dataset.state = state;
  status.dataset.i18n = key;
  status.textContent = labels[language][key];
  if (state !== "active") taskReader?.clear();
}

function setBusy(value) {
  busy = value;
  document.body.dataset.busy = String(value);
  for (const control of controls) control.disabled = value;
  form.setAttribute("aria-busy", String(value));
  taskReader?.setBusy(value);
}

function clearSecrets() {
  fields.password.value = "";
  fields.otp.value = "";
}

// Only canonical literal keys/tokens are accepted. Do not erase duplicate or escaped alias keys
// through JSON.parse before validating these tiny, fixed response envelopes.
const ws = "[\\x20\\x09\\x0a\\x0d]*";
const field = (key, value) => `"${key}"${ws}:${ws}${value}`;
const object = (first, second) => `\\{${ws}(?:${first}${ws},${ws}${second}|${second}${ws},${ws}${first})${ws}\\}`;
const success = key => new RegExp(`^${ws}${object(field("schema_version", "1"), field(key, "true"))}${ws}$`);
const authenticated = success("authenticated"), signedOut = success("ok");
function knownError(raw, code, outcome) {
  const error = object(field("code", `"${code}"`), field("outcome", outcome));
  return new RegExp(`^${ws}${object(field("schema_version", "1"), field("error", error))}${ws}$`).test(raw);
}
const invalidSession = raw => knownError(raw, "invalid_session", '"(?:not_attempted|unconfirmed)"');
const sessionPresent = raw => knownError(raw, "session_present", '"not_attempted"');

async function readBounded(response, signal) {
  if (!/^application\/json(?:;\s*charset=utf-8)?$/i.test(response.headers.get("content-type") || "")
    || !response.body) {
    void response.body?.cancel().catch(() => undefined);
    throw new Error("unconfirmed");
  }
  const reader = response.body.getReader(), chunks = [];
  let length = 0, frames = 0;
  const cancel = () => { void reader.cancel().catch(() => undefined); };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    signal.throwIfAborted();
    while (true) {
      const { value, done } = await reader.read();
      signal.throwIfAborted();
      if (done) break;
      if (++frames > 4096) {
        cancel();
        throw new Error("unconfirmed");
      }
      length += value.byteLength;
      if (length > 4096) {
        cancel();
        throw new Error("unconfirmed");
      }
      chunks.push(value);
    }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } finally {
    signal.removeEventListener("abort", cancel);
    reader.releaseLock();
  }
}

async function request(path, method, body, handedOff = () => undefined) {
  const controller = new AbortController();
  activeRequest = controller;
  let rejectAbort;
  const interrupted = new Promise((_resolve, reject) => { rejectAbort = reject; });
  const abort = () => rejectAbort(new Error("unconfirmed"));
  controller.signal.addEventListener("abort", abort, { once: true });
  // This limits a browser wait, not server work, a transaction or a rollback guarantee.
  const timer = setTimeout(() => controller.abort(), 10_000);
  try {
    const work = (async () => {
      let pending;
      try {
        pending = fetch(path, {
          method, mode: "same-origin", credentials: "same-origin", cache: "no-store",
          redirect: "error", signal: controller.signal,
          headers: { "X-Cyanrex-Platform-Request": "1",
            ...(method === "POST" ? { "Content-Type": "application/json" } : {}) },
          ...(body === undefined ? {} : { body }),
        });
      } finally { handedOff(); }
      const response = await pending;
      if (controller.signal.aborted) {
        void response.body?.cancel().catch(() => undefined);
        controller.signal.throwIfAborted();
      }
      const raw = await readBounded(response, controller.signal);
      controller.signal.throwIfAborted();
      return { status: response.status, raw };
    })();
    return await Promise.race([work, interrupted]);
  } finally {
    clearTimeout(timer);
    controller.signal.removeEventListener("abort", abort);
    if (activeRequest === controller) activeRequest = null;
  }
}

async function run(action) {
  if (busy || !mounted) return;
  const epoch = generation;
  const current = () => mounted && generation === epoch;
  setBusy(true);
  try { await action(current); }
  catch { if (current()) observe("unknown", "unknown"); }
  finally { if (current()) setBusy(false); }
}

async function inspect(current, afterLogin = false) {
  observe("checking", afterLogin ? "checkingCookie" : "checking");
  const response = await request("/platform/v1/session", "GET");
  if (!current()) return;
  if (response.status === 200 && authenticated.test(response.raw)) observe("active", "active");
  else if (!afterLogin && response.status === 401 && invalidSession(response.raw)) {
    observe("unauthenticated", "unauthenticated");
  } else observe("unknown", "unknown");
}

form.addEventListener("submit", event => {
  event.preventDefault();
  if (busy || !mounted) return;
  if (taskReader?.hasWork()) { taskReader.leave(() => {}, "login"); return; }
  const username = fields.username.value, password = fields.password.value, otp = fields.otp.value;
  if (!/^[a-z0-9_.-]{3,64}$/.test(username) || !password || /[\uD800-\uDFFF]/u.test(password)
    || encoder.encode(password).byteLength > 4096 || !/^[0-9]{6}$/.test(otp)) {
    observe("invalid-input", "invalidInput");
    return;
  }
  const body = JSON.stringify({ schema_version: 1, username, password, otp });
  void run(async current => {
    observe("checking", "checking");
    const response = await request("/platform/v1/session/login", "POST", body, clearSecrets);
    if (!current()) return;
    if (response.status === 200 && authenticated.test(response.raw)) await inspect(current, true);
    else if (response.status === 409 && sessionPresent(response.raw)) observe("session-present", "sessionPresent");
    else observe("unknown", "unknown");
  });
});

element("check-session").addEventListener("click", () => {
  taskReader.leave(() => { void run(current => inspect(current)); }, "check");
});
function logoutNow() {
  void run(async current => {
    observe("checking", "checking");
    const response = await request("/platform/v1/session/logout", "POST");
    if (!current()) return;
    if (response.status === 200 && signedOut.test(response.raw)) observe("unauthenticated", "unauthenticated");
    else observe("unknown", "unknown");
    element("logout").focus();
  });
}
element("logout").addEventListener("click", () => {
  if (busy || !mounted || dialog.open) return;
  if (taskReader.hasWork()) { taskReader.leave(logoutNow, "logout"); return; }
  dialog.showModal();
  element("cancel-logout").focus();
});
element("cancel-logout").addEventListener("click", () => {
  if (!busy) { dialog.close(); element("logout").focus(); }
});
dialog.addEventListener("cancel", event => {
  event.preventDefault();
  if (!busy) { dialog.close(); element("logout").focus(); }
});
element("confirm-logout").addEventListener("click", () => {
  if (busy || !mounted || !dialog.open) return;
  dialog.close();
  logoutNow();
});

locale.addEventListener("change", () => {
  if (busy || !Object.hasOwn(labels, locale.value)) return;
  language = locale.value;
  translate();
});
window.addEventListener("pagehide", () => {
  mounted = false;
  generation++;
  activeRequest?.abort();
  clearSecrets();
  observe("unknown", "unknown");
  if (dialog.open) dialog.close();
  setBusy(true);
});
window.addEventListener("pageshow", event => {
  if (!event.persisted) return;
  mounted = true;
  generation++;
  clearSecrets();
  setBusy(false);
  void run(current => inspect(current));
});

taskReader = createTaskReader({ run, isActive: () => mounted && status.dataset.state === "active",
  sessionRejected: response => {
    if (response.status === 401 && invalidSession(response.raw)) observe("unauthenticated", "unauthenticated");
    else observe("unknown", "unknown");
  }, label: key => labels[language][key] });
locale.value = language;
translate();
element("startup-note").hidden = true;
void run(current => inspect(current));
