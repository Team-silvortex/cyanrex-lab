import assert from "node:assert/strict";
import { before, after, test } from "node:test";
import { setup, published, saved, waitFor, SESSION, LOGOUT, INDEX, TASK, active, invalid, conflict, failed } from "./platformTaskBody/browserFixture.mjs";
import { id, entry, page, content, copy, hash } from "./platformTasks/data.mjs";

// Real Chrome and production assets/CSP; ALL API responses are own loopback MOCKS.
// This does not establish production authorization, persisted bytes or a server COMMIT.
let browser;
before(async () => {
  assert.ok(process.env.CYANREX_PLAYWRIGHT_MODULE, "set explicit Playwright module path");
  assert.ok(process.env.CYANREX_CHROMIUM_PATH, "set explicit Chrome executable path");
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE);
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const firstText = "Unchanged original body\r\n한국어 설명 😀\t保留原样。\r\n";
const secondText = "Second original note\n";
const original = content(entry(), [firstText, secondText]);
const changedTitle = "Edited local Draft title";
const changedText = "New body text，讨论记录 😀\n<script>window.__bodyExecuted=1</script>\n";
const publication = `${TASK}${id(10)}/text-artifacts`, replacement = `${TASK}${id(10)}/content`;
async function run(options, callback) {
  const f = await setup(browser, { capability: "enabled", ...options });
  try { await callback(f); await f.safe(); } finally { await f.close(); }
}
function standard(record, f) {
  if (record.path === SESSION) return f.respond(record, 200, active);
  if (record.path === INDEX) return f.respond(record, 200, page());
  if (record.method === "POST" && record.path === publication) return f.respond(record, 201, published(record, original));
  if (record.method === "PUT" && record.path === replacement) return f.respond(record, 200, saved(record, original));
  if (record.path === LOGOUT) return f.respond(record, 200, '{"schema_version":1,"ok":true}');
  if (record.method === "GET" && record.path === `${TASK}${id(10)}`) return f.respond(record, 200, original);
  return f.respond(record, 404, failed);
}
async function open(f) {
  await f.page.goto(`${f.origin}/platform/`, { waitUntil: "domcontentloaded" });
  await f.session(); await f.ready();
  await f.page.locator("#load-tasks").click(); await f.readState("listed"); await f.ready();
  await f.page.locator(`#task-list button[data-task-id="${id(10)}"]`).click();
  await f.readState("opened"); await f.ready();
}
async function begin(f) {
  await f.page.locator("#edit-task").click(); await f.state("clean");
  assert.equal(await f.page.locator("#task-edit-text").count(), 1, "trusted capability requires the body textarea");
  assert.equal(await f.page.locator("#task-edit-text").isVisible(), true);
}
async function edit(f) {
  await begin(f);
  await f.page.locator("#task-edit-title").fill(changedTitle);
  await f.page.locator("#task-edit-item").selectOption({ index: 1 });
  await f.page.locator("#task-edit-text").fill(changedText); await f.state("dirty");
}
async function review(f) {
  await f.page.locator("#save-task-metadata").click();
  assert.equal(await f.page.locator("#save-metadata-dialog").evaluate(node => node.open), true);
  assert.equal(await f.page.evaluate(() => document.activeElement.id), "cancel-save-metadata");
}
async function confirm(f) { await review(f); await f.page.locator("#confirm-save-metadata").click(); }
async function progress(f, count, task = false) {
  assert.equal(await f.page.locator("#task-save-progress").getAttribute("data-confirmed-artifacts"), String(count));
  assert.equal(await f.page.locator("#task-save-progress").getAttribute("data-task-confirmed"), String(task));
}
async function cleared(f) {
  assert.equal(await f.page.locator("#task-list button[data-task-id]").count(), 0);
  for (const selector of ["#task-content-title", "#task-content-metadata", "#task-text", "#metadata-save-summary", "#metadata-save-target", "#task-leave-target"])
    assert.equal(await f.page.locator(selector).textContent(), "");
  for (const selector of ["#task-edit-title", "#task-edit-filename", "#task-edit-language", "#task-edit-text"])
    assert.equal(await f.page.locator(selector).inputValue(), "");
  assert.equal(await f.page.locator("#task-edit-item option").count(), 0);
  assert.equal(await f.page.locator("#task-payload-select option").count(), 0);
  assert.equal(await f.page.locator("#task-metadata-editor").isVisible(), false);
  assert.ok(!(await f.page.locator("body").textContent()).includes(changedText));
}
function assertPublication(record, expectedText) {
  assert.equal(record.method, "POST"); assert.equal(record.url, publication);
  const body = JSON.parse(record.body);
  assert.deepEqual(Object.keys(body).sort(), ["artifact_id", "expected", "revision_id", "schema_version", "text"]);
  assert.equal(body.schema_version, 1); assert.equal(body.text, expectedText);
  assert.deepEqual(body.expected, { task: original.task, manifest: original.manifest });
  for (const key of ["artifact_id", "revision_id"]) {
    assert.match(body[key], /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/);
    assert.ok(!original.task.input_refs.some(ref => ref[key] === body[key]));
  }
}
function assertHeaders(record, f) {
  assert.equal(record.headers.origin, f.origin); assert.equal(record.headers["content-type"], "application/json");
  assert.equal(record.headers["x-cyanrex-platform-request"], "1");
  assert.equal(record.headers["sec-fetch-mode"], "same-origin");
  assert.equal(record.headers["sec-fetch-site"], "same-origin"); assert.equal(record.headers.authorization, undefined);
}

test("absent, disabled or unrecognized host capability keeps metadata-only editing and never publishes bodies", { timeout: 25_000 }, async () => {
  for (const capability of [null, "disabled", "true", "ENABLED"]) await run({ capability, api: standard }, async f => {
    await open(f); await f.page.locator("#edit-task").click(); await f.state("clean");
    assert.equal(await f.page.locator("footer").getAttribute("data-i18n"), "footer");
    const body = f.page.locator("#task-edit-text");
    assert.ok(await body.count() === 0 || !await body.isVisible() || await body.isDisabled());
    await f.page.locator("#task-edit-title").fill(changedTitle); await f.state("dirty");
    await confirm(f); await f.state("saved"); await f.ready();
    assert.equal(f.posts().length, 0); assert.equal(f.puts().length, 1);
    const expected = copy(original.manifest); expected.title = changedTitle;
    assert.deepEqual(JSON.parse(f.puts()[0].body), { schema_version: 1, expected_revision: 5, manifest: expected });
    assert.equal(await f.page.locator("#task-text").textContent(), firstText);
  });
});

test("enabled opening and slot selection preserve raw CRLF; metadata-only save sends no publication", { timeout: 15_000 }, () => run({ api: standard }, async f => {
  await open(f); await begin(f);
  assert.equal(await f.page.locator("footer").getAttribute("data-i18n"), "bodyFooter");
  assert.equal(await f.page.locator("#task-edit-text").inputValue(), firstText.replaceAll("\r\n", "\n"));
  await f.page.locator("#task-edit-item").selectOption({ index: 1 });
  assert.equal(await f.page.locator("#task-edit-text").inputValue(), secondText);
  await f.page.locator("#task-edit-item").selectOption({ index: 0 }); await f.state("clean");
  assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
  assert.equal(f.writes().length, 0);
  await f.page.locator("#task-edit-title").fill(changedTitle); await f.state("dirty");
  await confirm(f); await f.state("saved"); await f.ready();
  assert.equal(f.posts().length, 0); assert.equal(f.puts().length, 1);
  assert.deepEqual(JSON.parse(f.puts()[0].body).manifest.payload, original.manifest.payload);
  assert.equal(await f.page.locator("#task-text").textContent(), firstText);
}));

test("body review and dirty navigation default to Cancel without writes or lost slot edits", { timeout: 20_000 }, () => run({ api: standard }, async f => {
  await open(f); await edit(f);
  await f.page.locator("#task-edit-filename").fill("changed-note.txt");
  await f.page.locator("#task-edit-language").fill("future-valid+body");
  await f.page.locator("#task-edit-item").selectOption({ index: 0 });
  await f.page.locator("#task-edit-item").selectOption({ index: 1 });
  assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
  await review(f);
  assert.ok((await f.page.locator("#metadata-save-target").textContent()).includes(id(10)));
  const summary = await f.page.locator("#metadata-save-summary").textContent();
  for (const value of [changedTitle, "changed-note.txt", "future-valid+body"]) assert.ok(summary.includes(value));
  await f.page.locator("#cancel-save-metadata").click(); await f.state("dirty");
  assert.equal(f.writes().length, 0);
  for (const selector of ["#load-tasks", "#check-session", "#logout", "#discard-task-edit"]) {
    const requests = f.api().length;
    await f.page.locator(selector).click();
    assert.equal(await f.page.locator("#task-leave-dialog").evaluate(node => node.open), true);
    assert.equal(await f.page.evaluate(() => document.activeElement.id), "cancel-task-leave");
    await f.page.locator("#cancel-task-leave").click(); await f.state("dirty");
    assert.equal(f.api().length, requests); assert.equal(f.writes().length, 0);
    assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
  }
}));

test("confirmed body save publishes each slot sequentially against the original full baseline before one content PUT", { timeout: 20_000 }, () => run({ api(record, f) {
  if (record.method !== "GET") return; // Hold every write to observe admission and ordering.
  return standard(record, f);
} }, async f => {
  await open(f); await edit(f);
  await f.page.locator("#task-edit-filename").fill("new-second.txt");
  await review(f); await f.page.locator("#confirm-save-metadata").evaluate(node => { node.click(); node.click(); });
  await waitFor(() => f.posts().length === 1); await f.state("pending");
  assert.equal(f.puts().length, 0); await progress(f, 0);
  for (const selector of ["#task-edit-title", "#task-edit-item", "#task-edit-filename", "#task-edit-language", "#task-edit-text", "#save-task-metadata"])
    assert.equal(await f.page.locator(selector).isDisabled(), true);
  assertPublication(f.posts()[0], firstText); assertHeaders(f.posts()[0], f);
  const first = published(f.posts()[0], original); f.respond(f.posts()[0], 201, first);
  await waitFor(() => f.posts().length === 2); await progress(f, 1);
  assert.equal(f.puts().length, 0); assertPublication(f.posts()[1], changedText); assertHeaders(f.posts()[1], f);
  const second = published(f.posts()[1], original); f.respond(f.posts()[1], 201, second);
  await waitFor(() => f.puts().length === 1); await progress(f, 2);
  assertHeaders(f.puts()[0], f); assert.equal(f.puts()[0].url, replacement);
  const expected = copy(original.manifest); expected.title = changedTitle;
  expected.payload[0].artifact = first.artifact.reference; expected.payload[1].artifact = second.artifact.reference;
  expected.payload[1].filename = "new-second.txt";
  assert.deepEqual(JSON.parse(f.puts()[0].body), { schema_version: 1, expected_revision: 5, manifest: expected });
  assert.notEqual(first.artifact.reference.artifact_id, second.artifact.reference.artifact_id);
  assert.notEqual(first.artifact.reference.revision_id, second.artifact.reference.revision_id);
  f.respond(f.puts()[0], 200, saved(f.puts()[0], original)); await f.state("saved"); await f.ready(); await progress(f, 2, true);
  assert.equal(await f.page.locator("#task-content-title").textContent(), changedTitle);
  await f.page.locator("#task-payload-select").selectOption({ index: 1 });
  assert.equal(await f.page.locator("#task-text").textContent(), changedText);
  assert.equal(await f.page.evaluate(() => window.__bodyExecuted), undefined);
  assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
  assert.deepEqual(f.writes().map(record => record.method), ["POST", "POST", "PUT"]);
  assert.equal(f.api(`${TASK}${id(10)}`).length, 1, "save cannot auto-reopen");
  for (const path of ["body-contract.js", "task-save/taskContentSave.js", "task-save/taskSaveContract.js", "task-save/taskDraft.js", "task-save/taskPayloadLanguage.js"])
    assert.ok(f.records.some(record => record.path === `/platform/ui/${path}`), "use actual production body save modules");
}));

test("invalid text is local only; correcting it retains other local metadata", { timeout: 20_000 }, async () => {
  for (const value of ["\u0000", "é".repeat(131073)]) await run({ api: standard }, async f => {
    await open(f); await edit(f); await f.page.locator("#task-edit-text").fill(value);
    await f.page.locator("#save-task-metadata").click(); await f.state("invalid-local");
    assert.equal(await f.page.locator("#save-metadata-dialog").evaluate(node => node.open), false);
    assert.equal(f.writes().length, 0);
    await f.page.locator("#task-edit-text").fill(changedText); await f.state("dirty");
    assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
  });
});

test("unconfirmed publication never dispatches the next slot or PUT, retains local edits and never retries", { timeout: 35_000 }, async () => {
  const cases = [
    [503, () => failed, "unknown"], [409, () => conflict, "conflict"],
    [200, record => published(record, original), "unknown"],
    [201, record => { const value = published(record, original); value.artifact.title = changedTitle; return value; }, "unknown"],
    [201, record => { const value = published(record, original); value.artifact.reference.sha256 = hash("different"); return value; }, "unknown"],
    [201, record => JSON.stringify(published(record, original)).replace('"schema_version":1', '"schema_version":1,"schema_version":1'), "unknown"],
  ];
  for (const [status, response, state] of cases) await run({ api(record, f) {
    if (record.method === "POST" && record.path === publication) return f.respond(record, status, response(record));
    return standard(record, f);
  } }, async f => {
    await open(f); await edit(f); await confirm(f); await f.state(state); await f.ready();
    assert.equal(f.posts().length, 1); assert.equal(f.puts().length, 0); await progress(f, 0);
    assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
    assert.equal(await f.page.locator("#task-edit-title").inputValue(), changedTitle);
    assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
    await f.page.locator("#save-task-metadata").evaluate(node => node.click());
    assert.equal(f.posts().length, 1); assert.equal(f.api(`${TASK}${id(10)}`).length, 1);
  });
});

test("one confirmed Artifact followed by failure keeps partial progress and does not attach or replay", { timeout: 15_000 }, () => run({ api(record, f) {
  if (record.method === "POST" && record.path === publication && f.posts().length === 2) return f.respond(record, 503, failed);
  return standard(record, f);
} }, async f => {
  await open(f); await edit(f); await confirm(f); await f.state("unknown"); await f.ready(); await progress(f, 1);
  assert.equal(f.posts().length, 2); assert.equal(f.puts().length, 0);
  assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
  assert.equal(await f.page.locator("#task-content-title").textContent(), original.task.title);
  assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
}));

test("PUT conflict or malformed success retains published progress and frozen edits without retry or rebase", { timeout: 20_000 }, async () => {
  for (const state of ["conflict", "unknown"]) await run({ api(record, f) {
    if (record.method !== "PUT") return standard(record, f);
    if (state === "conflict") return f.respond(record, 409, conflict);
    const value = saved(record, original); value.task.owner.principal_id = id(999);
    return f.respond(record, 200, value);
  } }, async f => {
    await open(f); await edit(f); await confirm(f); await f.state(state); await f.ready(); await progress(f, 2);
    assert.equal(f.posts().length, 2); assert.equal(f.puts().length, 1);
    assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
    assert.equal(await f.page.locator("#task-content-title").textContent(), original.task.title);
    await f.page.locator("#save-task-metadata").evaluate(node => node.click());
    assert.equal(f.writes().length, 3); assert.equal(f.api(`${TASK}${id(10)}`).length, 1);
  });
});

test("publication timeout fences a late 201 and cannot continue to another POST or PUT", { timeout: 15_000 }, () => run({ fastTimeout: true, api(record, f) {
  if (record.method === "POST" && record.path === publication) return;
  return standard(record, f);
} }, async f => {
  await open(f); await edit(f); await confirm(f); await waitFor(() => f.posts().length === 1);
  await f.state("unknown"); await f.ready();
  f.respond(f.posts()[0], 201, published(f.posts()[0], original)); await f.page.waitForTimeout(80);
  await f.state("unknown"); await progress(f, 0);
  assert.equal(f.posts().length, 1); assert.equal(f.puts().length, 0);
  assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
}));

test("final PUT timeout keeps two Artifact confirmations but a controlled late 200 cannot confirm the Task", { timeout: 15_000 }, () => run({
  fastTimeout: true, api: standard, init: () => {
    const actual = window.fetch;
    window.__bodyHeldCount = 0;
    window.fetch = function(path, options) {
      if (options?.method === "PUT" && String(path).endsWith("/content")) {
        // Test-only transport ignores cancellation; no server dispatch or rollback claim.
        window.__bodyHeldCount++; window.__bodyHeldRequest = options.body;
        return new Promise(resolve => { window.__bodyDeliverLate = raw => resolve(new Response(raw,
          { status: 200, headers: { "content-type": "application/json" } })); });
      }
      return actual.call(this, path, options);
    };
  },
}, async f => {
  await open(f); await edit(f); await confirm(f);
  await f.page.waitForFunction(() => typeof window.__bodyDeliverLate === "function");
  await f.state("unknown"); await f.ready(); await progress(f, 2);
  const record = { body: await f.page.evaluate(() => window.__bodyHeldRequest) };
  await f.page.evaluate(raw => window.__bodyDeliverLate(raw), JSON.stringify(saved(record, original)));
  await f.page.waitForTimeout(80); await f.state("unknown"); await progress(f, 2);
  assert.equal(await f.page.evaluate(() => window.__bodyHeldCount), 1);
  assert.equal(f.posts().length, 2); assert.equal(f.puts().length, 0, "controlled PUT was not sent to mock server");
  assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
  assert.equal(await f.page.locator("#task-content-title").textContent(), original.task.title);
  assert.equal(await f.page.locator("#save-task-metadata").isDisabled(), true);
  assert.equal(f.api(`${TASK}${id(10)}`).length, 1);
}));

test("POST and PUT denial headers quarantine body DOM before a held error body, including restored active Sessions", { timeout: 30_000 }, async () => {
  for (const [method, status] of [["POST", 401], ["POST", 403], ["PUT", 401], ["PUT", 403]]) await run({ api(record, f) {
    if (record.method === method) return f.holdHeaders(record, status);
    return standard(record, f);
  } }, async f => {
    await open(f); await edit(f);
    const denial = f.page.waitForResponse(response => response.request().method() === method && response.status() === status);
    await confirm(f); assert.equal((await denial).status(), status);
    await f.state("quarantined"); await f.session("unknown"); await cleared(f);
    await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
    await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
    await f.session(); await f.ready(); await f.state("quarantined"); await cleared(f);
    const held = f.writes().at(-1); held.response.end(status === 401 ? invalid : "<html>denied</html>");
    await f.page.waitForTimeout(80); await f.state("quarantined"); await cleared(f);
    assert.equal(f.posts().length, method === "POST" ? 1 : 2); assert.equal(f.puts().length, method === "POST" ? 0 : 1);
  });
});

test("pagehide seals a pending publication and a controlled late success cannot revive body work", { timeout: 20_000 }, () => run({ api: standard, init: () => {
  const actual = window.fetch;
  window.__bodyHeldCount = 0;
  window.fetch = function(path, options) {
    if (options?.method === "POST" && String(path).endsWith("/text-artifacts")) {
      // Controlled test transport ignores AbortSignal; this does not claim server dispatch.
      window.__bodyHeldCount++; window.__bodyHeldRequest = options.body;
      return new Promise(resolve => { window.__bodyDeliverLate = raw => resolve(new Response(raw,
        { status: 201, headers: { "content-type": "application/json" } })); });
    }
    return actual.call(this, path, options);
  };
} }, async f => {
  await open(f); await edit(f);
  assert.equal(await f.page.evaluate(() => {
    const event = new Event("beforeunload", { cancelable: true }); window.dispatchEvent(event); return event.defaultPrevented;
  }), true, "best-effort browser handler only");
  await confirm(f); await f.state("pending");
  await f.page.waitForFunction(() => typeof window.__bodyDeliverLate === "function");
  const request = { body: await f.page.evaluate(() => window.__bodyHeldRequest) };
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pagehide", { persisted: true })));
  await f.state("quarantined"); await cleared(f);
  await f.page.evaluate(() => window.dispatchEvent(new PageTransitionEvent("pageshow", { persisted: true })));
  await f.session(); await f.ready();
  await f.page.evaluate(raw => window.__bodyDeliverLate(raw), JSON.stringify(published(request, original)));
  await f.page.waitForTimeout(80); await f.state("quarantined"); await cleared(f);
  assert.equal(await f.page.evaluate(() => window.__bodyHeldCount), 1); assert.equal(f.writes().length, 0);
}));

test("five locales retain body edits and Cancel-focused confirmations at 360px", { timeout: 25_000 }, () => run({ api: standard,
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
    assert.equal(await f.page.locator("#task-edit-text").inputValue(), changedText);
    await f.page.locator("#task-edit-text").scrollIntoViewIfNeeded();
    const box = await f.page.locator("#task-edit-text").boundingBox();
    assert.ok(box && box.width >= 44 && box.height >= 44 && box.x >= 0 && box.x + box.width <= 361);
    await review(f);
    for (const selector of ["#cancel-save-metadata", "#confirm-save-metadata"]) {
      const button = await f.page.locator(selector).boundingBox();
      assert.ok(button && button.width >= 44 && button.height >= 44 && button.x >= 0 && button.x + button.width <= 361);
    }
    await f.page.locator("#cancel-save-metadata").tap(); await f.state("dirty"); assert.equal(f.api().length, requests);
  }
  assert.equal(f.writes().length, 0);
}));
