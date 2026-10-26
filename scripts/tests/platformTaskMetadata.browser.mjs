import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { setup, saved, waitFor, SESSION, LOGOUT, INDEX, TASK, active, invalid, conflict, failed } from "./platformTaskMetadata/browserFixture.mjs";
import { id, entry, page, content, text, copy } from "./platformTasks/data.mjs";

// Actual Chrome and seven production static assets/CSP, but every API response is MOCKED.
// No real credentials, SQL authorization, server COMMIT, TLS or teaching runtime is exercised.
let browser;
before(async () => {
  assert.ok(process.env.CYANREX_PLAYWRIGHT_MODULE, "set explicit Playwright module path");
  assert.ok(process.env.CYANREX_CHROMIUM_PATH, "set explicit Chrome executable path");
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE);
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
async function run(options, callback) {
  const f = await setup(browser, options);
  try { await callback(f); await f.safe(); } finally { await f.close(); }
}
function standard(record, f) {
  if (record.path === SESSION) return f.respond(record, 200, active);
  if (record.path === INDEX) return f.respond(record, 200, page());
  if (record.method === "PUT") return f.respond(record, 200, saved(record));
  if (record.path === LOGOUT) return f.respond(record, 200, '{"schema_version":1,"ok":true}');
  const number = Number.parseInt(record.path.slice(-12), 16);
  return f.respond(record, 200, content(entry(Number.isFinite(number) ? number : 10)));
}
async function open(f, expected = "opened") {
  await f.page.goto(`${f.origin}/platform/`, { waitUntil: "domcontentloaded" });
  await f.session(); await f.ready();
  await f.page.locator("#load-tasks").click(); await f.readState("listed"); await f.ready();
  await f.page.locator(`#task-list button[data-task-id="${id(10)}"]`).click();
  await f.readState(expected); await f.ready();
  assert.equal(await f.page.locator("#edit-task").count(), 1, "manual metadata edit control must exist");
}
const changedTitle = "Metadata edit retained locally";
async function edit(f) {
  await f.page.locator("#edit-task").click(); await f.state("clean");
  await f.page.locator("#task-edit-title").fill(changedTitle); await f.state("dirty");
}
async function review(f) {
  await f.page.locator("#save-task-metadata").click();
  assert.equal(await f.page.locator("#save-metadata-dialog").evaluate(node => node.open), true);
}
async function confirm(f) { await review(f); await f.page.locator("#confirm-save-metadata").click(); }
async function cleared(f) {
  assert.equal(await f.page.locator("#task-list button[data-task-id]").count(), 0);
  assert.equal(await f.page.locator("#task-content-title").textContent(), "");
  assert.equal(await f.page.locator("#task-content-metadata").textContent(), "");
  assert.equal(await f.page.locator("#task-text").textContent(), "");
  assert.equal(await f.page.locator("#task-payload-select option").count(), 0);
}
async function leave(f, action) {
  const selectors = { load: "#load-tasks", next: "#next-tasks", check: "#check-session", logout: "#logout",
    discard: "#discard-task-edit", open: `#task-list button[data-task-id="${id(11)}"]` };
  await f.page.locator(selectors[action]).click();
  assert.equal(await f.page.locator("#task-leave-dialog").evaluate(node => node.open), true);
}
function fullPage(record, f) {
  if (record.path === INDEX) {
    const after = new URL(record.url, f.origin).searchParams.get("after");
    const rows = Array.from({ length: 20 }, (_, index) => entry(10 + index));
    return f.respond(record, 200, after ? page([entry(30)]) : page(rows, rows[19].reference));
  }
  return standard(record, f);
}

test("manual Draft exposes explicit metadata editing but never sends a write merely by opening", { timeout: 15_000 }, () => run({ api: standard }, async f => {
  await open(f);
  assert.equal(f.puts().length, 0);
  await f.page.locator("#edit-task").click(); await f.state("clean");
  assert.equal(await f.page.locator("#task-edit-title").inputValue(), "Discussion 10");
  assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
  assert.equal(f.puts().length, 0);
}));

test("review Cancel sends nothing; confirmed single PUT preserves references/order/text and advances only matching metadata", { timeout: 20_000 }, () => run({ api(record, f) {
  if (record.method === "PUT") return; // Hold response to exercise the real UI admission lock.
  return standard(record, f);
} }, async f => {
  await open(f); await edit(f);
  await f.page.locator("#task-edit-filename").fill("meeting-notes.txt");
  await f.page.locator("#task-edit-language").fill("future-valid+label");
  await f.page.locator("#task-edit-item").selectOption({ index: 1 });
  await f.page.locator("#task-edit-filename").fill("second-note.txt");
  await f.page.locator("#task-edit-item").selectOption({ index: 0 });
  assert.equal(await f.page.locator("#task-edit-filename").inputValue(), "meeting-notes.txt");
  await review(f);
  assert.equal(await f.page.evaluate(() => document.activeElement.id), "cancel-save-metadata");
  assert.ok((await f.page.locator("#metadata-save-target").textContent()).includes(id(10)));
  const summary = await f.page.locator("#metadata-save-summary").textContent();
  for (const value of [changedTitle, "meeting-notes.txt", "future-valid+label", "second-note.txt"]) assert.ok(summary.includes(value));
  await f.page.locator("#cancel-save-metadata").click(); await f.state("dirty");
  assert.equal(f.puts().length, 0);
  await review(f);
  await f.page.locator("#confirm-save-metadata").evaluate(node => { node.click(); node.click(); });
  await waitFor(() => f.puts().length === 1); await f.state("pending");
  for (const selector of ["#task-edit-title", "#task-edit-item", "#task-edit-filename", "#task-edit-language", "#save-task-metadata"]) {
    assert.equal(await f.page.locator(selector).isDisabled(), true);
  }
  const request = f.puts()[0], body = JSON.parse(request.body), expected = copy(content().manifest);
  expected.title = changedTitle; expected.payload[0].filename = "meeting-notes.txt";
  expected.payload[0].language = "future-valid+label"; expected.payload[1].filename = "second-note.txt";
  assert.deepEqual(body, { schema_version: 1, expected_revision: 5, manifest: expected });
  assert.equal(request.url, `${TASK}${id(10)}/content`);
  assert.equal(request.headers.origin, f.origin); assert.equal(request.headers["content-type"], "application/json");
  assert.equal(request.headers["x-cyanrex-platform-request"], "1");
  assert.equal(request.headers["sec-fetch-mode"], "same-origin");
  assert.equal(request.headers["sec-fetch-site"], "same-origin");
  assert.equal(request.headers.authorization, undefined);
  f.respond(request, 200, saved(request)); await f.state("saved"); await f.ready();
  assert.equal(await f.page.locator("#task-content-title").textContent(), changedTitle);
  assert.equal(await f.page.locator("#task-text").textContent(), text);
  assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
  assert.equal(f.api(`${TASK}${id(10)}`).length, 1, "success cannot auto-reopen or refresh");
  assert.equal(f.puts().length, 1);
  assert.ok(f.records.some(record => record.path === "/platform/ui/task-edit.js"), "use the seventh actual production asset");
}));

test("empty Draft title can be saved while ready/cancelled Tasks never become writable", { timeout: 25_000 }, async () => {
  for (const status of ["draft", "ready", "cancelled"]) await run({ api(record, f) {
    const selected = entry(10, { status, payload_items: 0 }), original = content(selected, []);
    if (record.path === SESSION) return f.respond(record, 200, active);
    if (record.path === INDEX) return f.respond(record, 200, page([selected]));
    return f.respond(record, 200, record.method === "PUT" ? saved(record, original) : original);
  } }, async f => {
    await open(f, "empty");
    if (status !== "draft") {
      assert.ok(!await f.page.locator("#edit-task").isVisible() || await f.page.locator("#edit-task").isDisabled());
      await f.page.locator("#edit-task").evaluate(node => node.click()); assert.equal(f.puts().length, 0); return;
    }
    await edit(f);
    assert.equal(await f.page.locator("#task-edit-item option").count(), 0);
    await confirm(f); await f.state("saved"); await f.ready();
    assert.deepEqual(JSON.parse(f.puts()[0].body).manifest.payload, []);
    assert.equal(await f.page.locator("#task-text").textContent(), "");
  });
});

test("invalid local title, basename or language cannot dispatch and correction retains other edits", { timeout: 25_000 }, async () => {
  for (const [selector, invalidValue] of [["#task-edit-title", " "], ["#task-edit-title", "é".repeat(129)],
    ["#task-edit-filename", "../outside.txt"], ["#task-edit-language", "Rust"]]) await run({ api: standard }, async f => {
    await open(f); await edit(f); await f.page.locator(selector).fill(invalidValue);
    await f.page.locator("#save-task-metadata").click(); await f.state("invalid-local");
    assert.equal(await f.page.locator("#save-metadata-dialog").evaluate(node => node.open), false);
    assert.equal(f.puts().length, 0);
    await f.page.locator(selector).fill(selector.endsWith("title") ? changedTitle : selector.endsWith("filename") ? "safe.txt" : "unknown-valid+hint");
    await f.state("dirty"); assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
    assert.equal(f.puts().length, 0);
  });
});

test("dirty navigation, paging, Session check, logout and discard all default to Cancel without I/O or lost edits", { timeout: 40_000 }, async () => {
  for (const action of ["load", "next", "open", "check", "logout", "discard"]) await run({ api: fullPage }, async f => {
    await open(f); await edit(f); const count = f.api().length;
    await leave(f, action);
    assert.equal(await f.page.evaluate(() => document.activeElement.id), "cancel-task-leave");
    assert.ok((await f.page.locator("#task-leave-target").textContent()).trim());
    assert.equal(await f.page.locator("#logout-dialog").evaluate(node => node.open), false, "dirty logout uses one confirmation only");
    await f.page.locator("#cancel-task-leave").click(); await f.state("dirty");
    assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
    assert.equal(f.api().length, count, `Cancel ${action} must not dispatch`);
  });
});

test("explicit leave confirmation discards local edits before executing only the selected action once", { timeout: 40_000 }, async () => {
  for (const action of ["load", "next", "open", "check", "logout", "discard"]) await run({ api: fullPage }, async f => {
    await open(f); await edit(f); const count = f.api().length;
    await leave(f, action); await f.page.locator("#confirm-task-leave").click();
    if (action === "discard") {
      assert.equal(f.api().length, count); assert.equal(await f.page.locator("#task-metadata-editor").isVisible(), false);
    } else {
      await waitFor(() => f.api().length === count + 1); await f.ready();
      assert.equal(f.api().length, count + 1);
      const request = f.api().at(-1);
      assert.equal(request.path, action === "logout" ? LOGOUT : action === "check" ? SESSION
        : action === "open" ? `${TASK}${id(11)}` : INDEX);
      assert.equal(request.method, action === "logout" ? "POST" : "GET");
      if (action === "logout") await f.session("unauthenticated");
    }
    assert.equal(f.puts().length, 0, "leaving cannot save or retry implicitly");
  });
});

test("recognized 409 conflicts retain the frozen edit and never rebase, replay or clear dirty automatically", { timeout: 20_000 }, async () => {
  for (const code of ["stale_revision", "conflict"]) await run({ api(record, f) {
    if (record.method === "PUT") return f.respond(record, 409, conflict.replace("stale_revision", code));
    return standard(record, f);
  } }, async f => {
    await open(f); await edit(f); await confirm(f); await f.state("conflict"); await f.ready();
    assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
    assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
    await f.page.locator("#save-task-metadata").evaluate(node => node.click());
    assert.equal(f.puts().length, 1); assert.equal(f.api(`${TASK}${id(10)}`).length, 1);
    await leave(f, "load"); await f.page.locator("#cancel-task-leave").click(); await f.state("conflict");
    assert.equal(f.puts().length, 1);
  });
});

test("bad 200 confirmations, malformed conflicts and server errors remain unknown with unsaved edits retained", { timeout: 40_000 }, async () => {
  const variants = [
    response => { response.task.revision++; },
    response => { response.task.owner.principal_id = id(999); },
    response => { response.task.created_at = "2026-10-25T01:02:03Z"; },
    response => { response.manifest.payload.reverse(); response.task.input_refs.reverse(); },
    response => { response.contents = []; },
    response => JSON.stringify(response).replace('"schema_version":1', '"schema_version":1,"schema_version":1'),
    () => [409, '{"schema_version":1,"error":{"code":"stale_revision","outcome":"unconfirmed"},"extra":true}'],
    () => [503, failed],
  ];
  for (const variant of variants) await run({ api(record, f) {
    if (record.method !== "PUT") return standard(record, f);
    const response = saved(record), result = variant(response);
    return f.respond(record, Array.isArray(result) ? result[0] : 200, Array.isArray(result) ? result[1] : result ?? response);
  } }, async f => {
    await open(f); await edit(f); await confirm(f); await f.state("unknown"); await f.ready();
    assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
    assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
    assert.equal(f.puts().length, 1); assert.equal(f.api(`${TASK}${id(10)}`).length, 1);
  });
});

test("browser waiting timeout retains the edit and a late HTTP success cannot turn unknown into saved", { timeout: 15_000 }, () => run({ fastTimeout: true, api(record, f) {
  if (record.method === "PUT") return;
  return standard(record, f);
} }, async f => {
  await open(f); await edit(f); await confirm(f); await waitFor(() => f.puts().length === 1);
  await f.state("unknown"); await f.ready();
  assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
  f.respond(f.puts()[0], 200, saved(f.puts()[0])); await f.page.waitForTimeout(80);
  assert.equal(await f.page.locator("#task-edit-state").getAttribute("data-state"), "unknown");
  assert.equal(f.puts().length, 1); assert.equal(f.api(`${TASK}${id(10)}`).length, 1);
}));

test("401/403 including bad bodies quarantine unsaved metadata and a later active Session cannot reveal or auto-restore it", { timeout: 35_000 }, async () => {
  const denied = '{"schema_version":1,"error":{"code":"access_denied","outcome":"unconfirmed"}}';
  for (const [status, raw, headers, session, held, restoreBeforeBody] of [[401, invalid, {}, "unauthenticated"],
    [401, "<html>denied</html>", { "content-type": "text/html" }, "unknown"],
    [403, Buffer.from([0xc3, 0x28]), {}, "unknown"],
    [401, invalid, {}, "unknown", true], [403, denied, {}, "unknown", true],
    [401, invalid, {}, "unknown", true, true]]) await run({ api(record, f) {
    if (record.method === "PUT") {
      if (held) return f.holdHeaders(record, status, headers);
      return f.respond(record, status, raw, headers);
    }
    return standard(record, f);
  } }, async f => {
    await open(f); await edit(f);
    const deniedHeaders = held ? f.page.waitForResponse(response => response.request().method() === "PUT"
      && response.url() === `${f.origin}${TASK}${id(10)}/content` && response.status() === status) : null;
    await confirm(f);
    if (held) {
      await waitFor(() => f.puts().length === 1);
      assert.equal((await deniedHeaders).status(), status, "Chrome observed actual denial headers while the body remains held");
      // Real HTTP denial headers are flushed; no body byte or EOF has been released.
      // The four-second UI wait must finish before the ten-second body deadline.
      await f.state("quarantined"); await f.session("unknown"); await cleared(f);
      for (const selector of ["#task-edit-title", "#task-edit-filename", "#task-edit-language"]) {
        assert.equal(await f.page.locator(selector).inputValue(), "");
      }
      assert.equal(await f.page.locator("#metadata-save-summary").textContent(), "");
      assert.equal(await f.page.locator("#metadata-save-target").textContent(), "");
      assert.equal(await f.page.locator("#task-leave-target").textContent(), "");
      assert.equal(await f.page.locator("#task-edit-item option").count(), 0);
      assert.ok(!(await f.page.locator("body").textContent()).includes(changedTitle));
      if (restoreBeforeBody) {
        await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
        await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
        await f.session(); await f.ready(); await f.state("quarantined");
        assert.equal(f.api(SESSION).length, 2, "restored page has its own new confirmed Session observation");
      }
      f.puts()[0].response.end(raw);
      await f.ready();
      if (restoreBeforeBody) await f.page.waitForTimeout(80);
      const observed = await f.page.locator("#status").getAttribute("data-state");
      assert.ok(restoreBeforeBody ? observed === "active"
        : status === 401 ? ["unknown", "unauthenticated"].includes(observed) : observed === "unknown");
    } else {
      await f.state("quarantined"); await f.session(session); await f.ready();
    }
    await cleared(f);
    assert.equal(await f.page.locator("#task-metadata-editor").isVisible(), false);
    for (const selector of ["#task-edit-title", "#task-edit-filename", "#task-edit-language"]) {
      assert.equal(await f.page.locator(selector).inputValue(), "");
    }
    assert.equal(await f.page.locator("#task-edit-item option").count(), 0);
    assert.ok(!(await f.page.locator("body").textContent()).includes(changedTitle));
    await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
    await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
    await f.session(); await f.ready(); await f.state("quarantined");
    assert.equal(await f.page.locator("#task-metadata-editor").isVisible(), false);
    await leave(f, "load"); await f.page.locator("#cancel-task-leave").click();
    assert.equal(f.api(INDEX).length, 1, "new active cannot bypass explicit discard of old context");
    await f.state("quarantined"); assert.equal(f.puts().length, 1);
  });
});

test("pagehide seals pending edits, best-effort unload stays guarded and a controlled late receipt cannot revive them", { timeout: 20_000 }, () => run({ api: standard, init: () => {
  const actual = window.fetch;
  window.__metadataHeldCount = 0;
  window.fetch = function(path, options) {
    if (options?.method === "PUT") {
      // Test-only transport deliberately ignores AbortSignal; no server dispatch is claimed.
      window.__metadataHeldCount++; window.__metadataHeldBody = options.body;
      return new Promise(resolve => { window.__metadataDeliverLate = raw => resolve(new Response(raw,
        { status: 200, headers: { "content-type": "application/json" } })); });
    }
    return actual.call(this, path, options);
  };
} }, async f => {
  await open(f); await edit(f);
  assert.equal(await f.page.evaluate(() => {
    const event = new Event("beforeunload", { cancelable: true }); window.dispatchEvent(event); return event.defaultPrevented;
  }), true, "best-effort handler only; this does not prove browser/OS will preserve memory");
  await confirm(f); await f.state("pending");
  await f.page.waitForFunction(() => typeof window.__metadataDeliverLate === "function");
  const request = { body: await f.page.evaluate(() => window.__metadataHeldBody) };
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  await f.state("quarantined"); await cleared(f);
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
  await f.session(); await f.ready();
  await f.page.evaluate(raw => window.__metadataDeliverLate(raw), JSON.stringify(saved(request)));
  await f.page.waitForTimeout(80); await f.state("quarantined"); await cleared(f);
  assert.equal(await f.page.evaluate(() => window.__metadataHeldCount), 1);
  assert.equal(await f.page.locator("#task-metadata-editor").isVisible(), false);
}));

test("five complete locales keep metadata fields and Cancel-focused confirmations usable at 360px", { timeout: 25_000 }, () => run({ api: standard,
  viewport: { width: 360, height: 940 }, touch: true }, async f => {
  await open(f); await edit(f);
  for (const language of ["en", "zh-CN", "es", "ja", "ko"]) {
    const requests = f.api().length; await f.page.locator("#locale").selectOption(language);
    const observed = await f.page.evaluate(async expected => {
      const { labels } = await import("/platform/ui/labels.js");
      return { locale: document.documentElement.lang, keys: Object.keys(labels[expected]).sort(),
        expected: Object.keys(labels.en).sort(), width: innerWidth, scroll: document.documentElement.scrollWidth,
        wrong: [...document.querySelectorAll("[data-i18n]")].filter(node => typeof labels[expected][node.dataset.i18n] !== "string"
          || !labels[expected][node.dataset.i18n].trim() || node.textContent !== labels[expected][node.dataset.i18n]).map(node => node.dataset.i18n) };
    }, language);
    assert.equal(observed.locale, language); assert.deepEqual(observed.keys, observed.expected);
    assert.deepEqual(observed.wrong, []); assert.ok(observed.scroll <= observed.width + 1);
    assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
    for (const selector of ["#task-edit-title", "#task-edit-item", "#task-edit-filename", "#task-edit-language", "#save-task-metadata"]) {
      await f.page.locator(selector).scrollIntoViewIfNeeded();
      const box = await f.page.locator(selector).boundingBox();
      assert.ok(box && box.width >= 44 && box.height >= 44 && box.x >= 0 && box.x + box.width <= 361);
    }
    await review(f);
    assert.equal(await f.page.evaluate(() => document.activeElement.id), "cancel-save-metadata");
    for (const selector of ["#cancel-save-metadata", "#confirm-save-metadata"]) {
      const box = await f.page.locator(selector).boundingBox();
      assert.ok(box && box.width >= 44 && box.height >= 44 && box.x >= 0 && box.x + box.width <= 361);
    }
    await f.page.locator("#cancel-save-metadata").tap(); await f.state("dirty");
    assert.equal(f.api().length, requests);
  }
  assert.equal(f.puts().length, 0);
}));
