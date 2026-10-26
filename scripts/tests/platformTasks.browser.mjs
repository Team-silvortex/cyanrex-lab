import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { setup, waitFor, SESSION, LOGOUT, INDEX, TASK, active, invalid, unavailable, failed } from "./platformTasks/browserFixture.mjs";
import { id, workspace, entry, page, content, text, copy } from "./platformTasks/data.mjs";

// Real Chrome, the six production static assets and actual CSP. All HTTP API replies are MOCKED.
// This is not real Session/SQL authorization, TLS acceptance, teaching execution or saved provenance.
let browser;
before(async () => {
  assert.ok(process.env.CYANREX_PLAYWRIGHT_MODULE, "set an explicit absolute Playwright module path");
  assert.ok(process.env.CYANREX_CHROMIUM_PATH, "set an explicit Chrome executable path");
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE);
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
async function run(options, callback) {
  const f = await setup(browser, options);
  try { await callback(f); await f.safe(); } finally { await f.close(); }
}
async function open(f, session = "active") {
  await f.page.goto(`${f.origin}/platform/`, { waitUntil: "domcontentloaded" });
  await f.session(session); await f.ready();
  assert.equal(await f.page.locator("#load-tasks").count(), 1, "new explicit Task load control must exist");
}
async function load(f) { await f.page.locator("#load-tasks").click(); await f.state("listed"); await f.ready(); }
async function select(f, number = 10, expected = "opened") {
  await f.page.locator(`#task-list button[data-task-id="${id(number)}"]`).click();
  await f.state(expected); await f.ready();
}
function standard(record, f) {
  if (record.path === SESSION) return f.respond(record, 200, active);
  if (record.path === INDEX) return f.respond(record, 200, page());
  if (record.path.startsWith(TASK)) return f.respond(record, 200, content());
  if (record.path === LOGOUT) return f.respond(record, 200, '{"schema_version":1,"ok":true}');
  return f.respond(record, 404, unavailable);
}

test("active Session exposes explicit Task load without any automatic list or item request", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === INDEX && f.api(INDEX).length > 1) return f.respond(record, 200, page([]));
  return standard(record, f);
} }, async f => {
  await open(f); await f.state("idle");
  assert.equal(f.tasks().length, 0);
  assert.equal(await f.page.locator("#next-tasks").isDisabled(), true);
  await load(f);
  assert.equal(f.api(INDEX).length, 1);
  assert.equal(f.api(INDEX)[0].url, `${INDEX}?limit=20`);
  const headers = f.api(INDEX)[0].headers;
  assert.equal(headers["x-cyanrex-platform-request"], "1");
  assert.equal(headers["sec-fetch-site"], "same-origin");
  assert.equal(headers["sec-fetch-mode"], "same-origin");
  assert.equal(headers.authorization, undefined);
  assert.equal(f.tasks().length, 1, "listing cannot automatically reopen an item");
  for (const path of ["/platform/", "/platform/ui/app.js", "/platform/ui/labels.js",
    "/platform/ui/style.css", "/platform/ui/tasks.js", "/platform/ui/task-contract.js"]) {
    assert.ok(f.records.some(record => record.path === path), `actual fixed production asset requested: ${path}`);
  }
  await f.page.locator("#load-tasks").click(); await f.state("empty"); await f.ready(); await f.clear();
  assert.equal(f.api(INDEX).length, 2, "an explicit empty-page refresh must not silently fetch more");
}));

test("unconfirmed Session disables task loading and cannot dispatch a list request", { timeout: 15_000 }, () => run({ api(record, f) {
  return f.respond(record, 401, invalid);
} }, async f => {
  await open(f, "unauthenticated");
  assert.equal(await f.page.locator("#load-tasks").isDisabled(), true);
  await f.page.locator("#load-tasks").evaluate(node => node.click());
  assert.equal(f.tasks().length, 0); await f.clear();
}));

test("pagination is explicit, fixed at 20 and replaces rather than accumulates the previous page", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === SESSION) return f.respond(record, 200, active);
  const first = Array.from({ length: 20 }, (_, index) => entry(10 + index));
  return f.respond(record, 200, f.api(INDEX).length === 1 ? page(first, first[19].reference) : page([entry(30)]));
} }, async f => {
  await open(f); await load(f);
  assert.equal(await f.page.locator("#task-list button[data-task-id]").count(), 20);
  assert.equal(f.api(INDEX).length, 1);
  await f.page.locator("#next-tasks").click(); await f.state("listed"); await f.ready();
  const query = new URL(f.api(INDEX)[1].url, f.origin).searchParams;
  assert.deepEqual([...query.keys()].sort(), ["after", "limit"]);
  assert.equal(query.get("limit"), "20"); assert.equal(query.get("after"), id(29));
  assert.equal(await f.page.locator("#task-list button[data-task-id]").count(), 1);
  assert.equal(await f.page.locator("#next-tasks").isDisabled(), true);
  assert.equal(f.api(INDEX).length, 2);
}));

test("selected text is inert, payload selection is local and an empty Task reopens without an editor or Run", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === SESSION) return f.respond(record, 200, active);
  if (record.path === INDEX) return f.respond(record, 200, page([entry(), entry(11, { payload_items: 0 })]));
  return f.respond(record, 200, record.path.endsWith(id(11)) ? content(entry(11, { payload_items: 0 }), []) : content());
} }, async f => {
  await open(f); await load(f); await select(f);
  assert.equal(await f.page.locator("#task-text").textContent(), text);
  assert.equal(await f.page.evaluate(() => window.__taskExecuted), undefined);
  assert.equal(await f.page.locator("#task-text script, #task-text iframe").count(), 0);
  assert.equal(await f.page.locator("#task-payload-select option").count(), 2);
  await f.page.locator("#task-payload-select").selectOption({ index: 1 });
  assert.equal(await f.page.locator("#task-text").textContent(), "Second plain note\n");
  assert.equal(f.tasks().length, 2, "selecting an already returned payload must not fetch or execute");
  await select(f, 11, "empty");
  assert.equal(await f.page.locator("#task-text").textContent(), "");
  assert.equal(await f.page.locator("#task-payload-select option").count(), 0);
  assert.equal(await f.page.locator("#task-content-title").textContent(), "Discussion 11");
}));

test("a fresh load clears old details immediately and any failed read clears both list and details", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === INDEX && f.api(INDEX).length === 2) return;
  return standard(record, f);
} }, async f => {
  await open(f); await load(f); await select(f);
  await f.page.locator("#load-tasks").click(); await f.state("loading");
  assert.equal(await f.page.locator("#task-text").textContent(), "");
  assert.equal(await f.page.locator("#task-content-title").textContent(), "");
  await waitFor(() => f.api(INDEX).length === 2);
  f.respond(f.api(INDEX)[1], 503, failed); await f.state("unknown"); await f.ready(); await f.clear();
  assert.equal(f.api(INDEX).length, 2);
}));

test("missing/foreign item is unavailable without keeping previous private content", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path.startsWith(TASK) && f.api(record.path).length === 2) return f.respond(record, 404, unavailable);
  return standard(record, f);
} }, async f => {
  await open(f); await load(f); await select(f);
  await f.page.locator(`#task-list button[data-task-id="${id(10)}"]`).click();
  await f.state("unavailable"); await f.ready(); await f.clear();
  assert.equal(f.api(`${TASK}${id(10)}`).length, 2);
}));

test("Task 401 invalid_session clears private data and the active Session observation", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.path === INDEX && f.api(INDEX).length === 2) return f.respond(record, 401, invalid);
  return standard(record, f);
} }, async f => {
  await open(f); await load(f); await select(f);
  await f.page.locator("#load-tasks").click();
  await f.session("unauthenticated"); await f.ready(); await f.clear();
  assert.equal(await f.page.locator("#load-tasks").isDisabled(), true);
  assert.equal(f.api(INDEX).length, 2);
}));

test("index raw duplicates, wrong scope/cursor, repeated entries and oversized pages fail closed", { timeout: 35_000 }, async () => {
  const badCursor = copy(workspace); badCursor.workspace_id = id(999);
  const full = Array.from({ length: 20 }, (_, index) => entry(10 + index));
  const variants = [
    [200, '{"schema_version":1,"schema_version":1,"entries":[],"next_after":null}'],
    [200, page([entry(), entry()])],
    [200, page(Array.from({ length: 21 }, (_, index) => entry(10 + index)))],
    [200, page(full, { workspace: badCursor, task_id: id(29) })],
    [200, page(full, { workspace: copy(workspace), task_id: id(999) })],
    [200, page([entry(), entry(11, { reference: { workspace: badCursor, task_id: id(11) } })])],
    [200, { ...page(), owner: { authority_id: id(1), principal_id: id(9) } }],
    [401, { schema_version: 1, error: { code: "invalid_session", outcome: "unconfirmed" }, extra: true }],
    [401, "<html>Denied, not a JSON response</html>", { "content-type": "text/html" }],
    [403, Buffer.from([0xc3, 0x28])], // Invalid UTF-8 must not hide already received denial status.
  ];
  for (const [status, bad, headers] of variants) await run({ api(record, f) {
    return f.respond(record, record.path === SESSION ? 200 : status, record.path === SESSION ? active : bad,
      record.path === SESSION ? {} : headers);
  } }, async f => {
    await open(f); await f.page.locator("#load-tasks").click(); await f.state("unknown"); await f.ready(); await f.clear();
    assert.equal(f.api(INDEX).length, 1);
    if (status === 401 || status === 403) assert.equal(await f.page.locator("#status").getAttribute("data-state"),
      "unknown", `HTTP ${status} denial must invalidate the stale active Session even when its body is unreadable`);
  });
});

test("content revision pins, exact binding order, digest, decoded bounds and raw duplicates are checked before display", { timeout: 40_000 }, async () => {
  const changedTitle = content(); changedTitle.task.title = changedTitle.manifest.title = "Unexpected same-revision title";
  const changedScope = content(); changedScope.task.reference.workspace.workspace_id = id(999);
  const wrongHash = content(); wrongHash.contents[0].text += "tampered";
  const wrongOrder = content(); wrongOrder.contents.reverse();
  const huge = content(entry(), ["x".repeat(256 * 1024 + 1), "Second plain note\n"]);
  const variants = [changedTitle, changedScope, wrongHash, wrongOrder, huge,
    JSON.stringify(content()).replace('"schema_version":1', '"schema_version":1,"schema_version":1'),
    { ...content(), unexpected: true }];
  for (const bad of variants) await run({ api(record, f) {
    if (record.path === SESSION) return f.respond(record, 200, active);
    return f.respond(record, 200, record.path === INDEX ? page() : bad);
  } }, async f => {
    await open(f); await load(f);
    await f.page.locator(`#task-list button[data-task-id="${id(10)}"]`).click();
    await f.state("unknown"); await f.ready(); await f.clear();
    assert.equal(f.api(`${TASK}${id(10)}`).length, 1);
  });
});

test("manual Session check, confirmed logout dispatch and pagehide clear old list/detail before awaiting replies", { timeout: 30_000 }, async () => {
  for (const action of ["check", "logout", "pagehide"]) await run({ init: () => {
    const actual = window.fetch;
    window.__sessionClearAtHandoff = [];
    window.fetch = function(path, options) {
      if (path === "/platform/v1/session" || path === "/platform/v1/session/logout") {
        // Observe the DOM at handoff, then retain actual owned HTTP transport unchanged.
        window.__sessionClearAtHandoff.push({ path,
          list: document.querySelectorAll("#task-list button[data-task-id]").length,
          title: document.querySelector("#task-content-title")?.textContent || "",
          text: document.querySelector("#task-text")?.textContent || "" });
      }
      return actual.call(this, path, options);
    };
  }, api(record, f) {
    if (record.path === SESSION && f.api(SESSION).length > 1) return;
    if (record.path === LOGOUT) return;
    return standard(record, f);
  } }, async f => {
    await open(f); await load(f); await select(f);
    assert.equal(await f.page.locator("#task-text").textContent(), text);
    if (action === "check") {
      await f.page.locator("#check-session").click(); await waitFor(() => f.api(SESSION).length === 2);
    } else if (action === "logout") {
      await f.page.locator("#logout").click(); await f.page.locator("#confirm-logout").click();
      await waitFor(() => f.api(LOGOUT).length === 1);
    } else await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
    await f.clear();
    if (action !== "pagehide") assert.deepEqual(await f.page.evaluate(() => window.__sessionClearAtHandoff.at(-1)),
      { path: action === "check" ? SESSION : LOGOUT, list: 0, title: "", text: "" });
    assert.equal(f.tasks().length, 2, "invalidating Session observation cannot auto-refresh tasks");
  });
});

test("controlled fetch late success from an aborted epoch cannot revive cleared Task data", { timeout: 15_000 }, () => run({ api: standard, init: () => {
  const actual = window.fetch;
  window.__taskLateDelivered = false;
  window.fetch = function(path, options) {
    if (String(path).startsWith("/platform/v1/tasks/")) {
      // Explicit test-only transport ignores AbortSignal so production epoch fencing is exercised.
      return new Promise(resolve => { window.__deliverTaskLate = raw => {
        window.__taskLateDelivered = true;
        resolve(new Response(raw, { status: 200, headers: { "content-type": "application/json" } }));
      }; });
    }
    return actual.call(this, path, options);
  };
} }, async f => {
  await open(f); await load(f);
  await f.page.locator(`#task-list button[data-task-id="${id(10)}"]`).click();
  await f.page.waitForFunction(() => typeof window.__deliverTaskLate === "function");
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  await f.clear();
  await f.page.evaluate(raw => window.__deliverTaskLate(raw), JSON.stringify(content()));
  await f.page.waitForTimeout(80); await f.clear();
  assert.equal(await f.page.evaluate(() => window.__taskLateDelivered), true);
}));

test("all five languages keep explicit task controls and inert content usable at 360px", { timeout: 30_000 }, () => run({ api: standard,
  viewport: { width: 360, height: 940 }, touch: true }, async f => {
  await open(f); await load(f); await select(f);
  for (const language of ["en", "zh-CN", "es", "ja", "ko"]) {
    const requests = f.api().length;
    await f.page.locator("#locale").selectOption(language);
    const result = await f.page.evaluate(async expected => {
      const { labels } = await import("/platform/ui/labels.js");
      return { locale: document.documentElement.lang, keys: Object.keys(labels[expected]).sort(),
        expected: Object.keys(labels.en).sort(), width: innerWidth, scroll: document.documentElement.scrollWidth,
        wrong: [...document.querySelectorAll("[data-i18n]")].filter(node =>
          typeof labels[expected][node.dataset.i18n] !== "string" || !labels[expected][node.dataset.i18n].trim()
          || node.textContent !== labels[expected][node.dataset.i18n]).map(node => node.dataset.i18n) };
    }, language);
    assert.equal(result.locale, language); assert.deepEqual(result.keys, result.expected);
    assert.deepEqual(result.wrong, []); assert.ok(result.scroll <= result.width + 1);
    for (const selector of ["#load-tasks", "#task-payload-select", `#task-list button[data-task-id="${id(10)}"]`]) {
      await f.page.locator(selector).scrollIntoViewIfNeeded();
      const box = await f.page.locator(selector).boundingBox();
      assert.ok(box && box.width >= 44 && box.height >= 44 && box.x >= 0 && box.x + box.width <= 361);
    }
    assert.equal(f.api().length, requests, "language changes must not dispatch API calls");
    assert.equal(await f.page.locator("#task-text").textContent(), text);
  }
}));
