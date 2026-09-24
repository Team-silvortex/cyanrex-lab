import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const stub = fileURLToPath(new URL("./fixtures/eventPageStubs.mjs", import.meta.url));
  bundle = await buildFixture(new URL("./fixtures/eventPage.mjs", import.meta.url), {
    "next/router$": stub, "next/link$": stub, "next/head$": stub,
    [fileURLToPath(new URL("../src/utils/pageState.ts", import.meta.url))]: stub,
  });
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
const row = (id = "one", category = "kernel", timestamp = "2026-09-13T00:00:00Z") => ({ username: "student", timestamp,
  source: "fixture", event_type: "fixture." + id, category, severity: "success", color: "green", payload: {} });
async function run(callback) {
  const f = await setupFixture(browser, bundle);
  f.act = callback => f.page.evaluate(callback);
  const respond = f.respond;
  f.respond = async (...args) => { await respond(...args); await f.advance(1); };
  f.mount = async () => { await f.act(() => fixture.render()); await f.advance(1); };
  f.ids = async path => (await f.requests()).flatMap((request, index) => new URL(request.url).pathname === path ? [index] : []);
  f.latest = async path => (await f.ids(path)).at(-1);
  f.ready = async rows => { await f.mount(); await f.respond(await f.latest("/events"), rows ?? [row()]); };
  f.button = key => f.page.getByRole("button", { name: key, exact: true });
  f.approveDelete = async () => {
    await f.button("Delete Filtered").click(); await f.page.getByRole("dialog").getByRole("textbox").fill("DELETE");
    await f.page.getByRole("dialog").getByRole("button", { name: /Confirm/ }).click();
  };
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); } finally { await f.close(); }
}

test("filter changes never commit previous-scope rows under the new selection", () => run(async f => {
  await f.ready(); await f.act(() => { fixture.renders = []; });
  await f.page.locator("main.content select").first().selectOption("platform");
  const renders = await f.act(() => fixture.renders.filter(item => item.category === "platform"));
  assert.ok(renders.length); assert.ok(renders.every(item => item.rows.length === 0));
}));

test("invalid saved custom dates fail visibly instead of crashing the event page", () => run(async f => {
  await f.act(() => { fixture.saved = { events_range_v1: "custom", events_start_v1: "invalid-date" }; });
  await f.mount();
  assert.deepEqual(f.errors, []); assert.equal((await f.ids("/events")).length, 0);
  assert.equal(await f.button("Export Download").isDisabled(), true);
}));

test("rolling time windows retire expired visible rows even without new events", () => run(async f => {
  await f.act(() => { fixture.saved.events_range_v1 = "10m"; });
  await f.ready([row("old", "kernel", "2026-09-12T23:50:02Z")]);
  assert.equal(await f.page.locator("article").count(), 1);
  await f.advance(2200); assert.equal(await f.page.locator("article").count(), 0);
}));

test("export admission prevents same-turn duplicate downloads", () => run(async f => {
  await f.ready(); await f.act(() => {
    const button = [...document.querySelectorAll("button")].find(button => button.textContent === "Export Download");
    button.click(); button.click();
  });
  assert.equal((await f.ids("/events/export")).length, 1);
}));

test("changed export filters abort the download and ignore its late response", () => run(async f => {
  await f.ready(); await f.button("Export Download").click(); const index = await f.latest("/events/export");
  await f.page.locator("main.content select").first().selectOption("platform");
  await f.respond(index, [row()]);
  assert.equal((await f.requests())[index].aborted, true);
  assert.deepEqual(await f.act(() => fixture.downloads), []);
}));

test("leaving the page aborts export and cannot trigger a late local download", () => run(async f => {
  await f.ready(); await f.button("Export Download").click(); const index = await f.latest("/events/export");
  await f.act(() => fixture.render(false)); await f.respond(index, [row()]);
  assert.equal((await f.requests())[index].aborted, true); assert.deepEqual(await f.act(() => fixture.downloads), []);
}));

test("event exports reject unexpected response content instead of downloading an HTML error", () => run(async f => {
  await f.ready(); await f.button("Export Download").click(); const index = await f.latest("/events/export");
  await f.page.evaluate(index => fixture.response(index, "<html>login</html>", { "content-type": "text/html" }), index);
  await f.advance(1);
  await f.page.getByRole("alert").waitFor({ timeout: 2000 });
  assert.deepEqual(await f.act(() => fixture.downloads), []); assert.ok(await f.page.getByRole("alert").count());
}));

test("unconfirmed deletion bodies cannot clear visible history or restart its stream", () => run(async f => {
  await f.ready(); await f.approveDelete(); const index = await f.latest("/events/delete"), before = (await f.ids("/events")).length;
  await f.respond(index, { ok: "true", deleted: -1 });
  await f.page.getByRole("dialog").getByRole("alert").waitFor({ timeout: 2000 });
  assert.equal(await f.page.locator("article").count(), 1); assert.equal((await f.ids("/events")).length, before);
  assert.ok(await f.page.getByRole("dialog").getByRole("alert").count());
}));

test("navigation cancels deletion waiting and obsolete success cannot refresh a new view", () => run(async f => {
  await f.ready(); await f.approveDelete(); const index = await f.latest("/events/delete"), before = (await f.ids("/events")).length;
  await f.act(() => fixture.render({ navigation: "/events?view=other" }));
  await f.respond(index, { ok: true, deleted: 1 }); await f.advance(1);
  assert.equal((await f.requests())[index].aborted, true); assert.equal((await f.ids("/events")).length, before + 1);
}));

test("failed read acknowledgements remain visible and do not silently retry on live events", () => run(async f => {
  await f.ready(); await f.advance(1200); const index = await f.latest("/events/mark-read");
  await f.respond(index, { ok: false }, 503);
  assert.ok(await f.page.getByRole("alert").count());
  await f.page.evaluate(event => fixture.live(event), row("two")); await f.advance(1200);
  assert.equal((await f.ids("/events/mark-read")).length, 1);
}));

test("unread polling never overlaps a pending request and cancels it on page exit", () => run(async f => {
  await f.act(() => { fixture.holdUnread = true; }); await f.mount();
  await f.advance(9000); assert.equal((await f.ids("/events/unread-count")).length, 1);
  const index = await f.latest("/events/unread-count"); await f.act(() => fixture.render(false));
  assert.equal((await f.requests())[index].aborted, true);
}));

test("malformed unread counts show unavailable rather than a false zero or numeric badge", () => run(async f => {
  await f.act(() => { fixture.holdUnread = true; }); await f.mount();
  await f.respond(await f.latest("/events/unread-count"), { unread: "12" });
  assert.equal(await f.page.locator(".nav-badge").textContent(), "?");
}));

test("export and unread/mark-read requests are private and reject redirects", () => run(async f => {
  await f.act(() => { fixture.holdUnread = true; }); await f.ready(); await f.advance(1200);
  await f.button("Export Download").click();
  for (const request of await f.requests()) {
    assert.equal(request.credentials, "include"); assert.equal(request.cache, "no-store"); assert.equal(request.redirect, "error");
  }
}));

test("a successful JSON export keeps its exact filters, downloads once and releases its object URL", () => run(async f => {
  await f.act(() => { fixture.saved.events_category_v1 = "kernel"; fixture.saved.events_severity_v1 = "success"; });
  await f.ready(); await f.button("Export Download").click(); const index = await f.latest("/events/export");
  await f.respond(index, [row()]); await f.advance(1);
  await f.page.waitForFunction(() => fixture.downloads.length === 1);
  const request = (await f.requests())[index], params = new URL(request.url).searchParams;
  assert.equal(params.get("category"), "kernel"); assert.equal(params.get("severity"), "success");
  assert.equal(params.get("format"), "json"); assert.equal(params.has("limit"), false);
  assert.deepEqual(await f.act(() => fixture.downloads), [{ filename: "cyanrex-events.json", url: "blob:fixture-1" }]);
  assert.deepEqual(await f.act(async () => JSON.parse(await fixture.blobs[0].text())), [row()]);
  await f.advance(1); assert.deepEqual(await f.act(() => fixture.revoked), ["blob:fixture-1"]);
}));

test("CSV export has a safe format-bound filename and changing format cancels the previous export", () => run(async f => {
  await f.ready(); await f.button("Export Download").click(); const old = await f.latest("/events/export");
  await f.page.locator("select").filter({ has: f.page.locator('option[value="csv"]') }).selectOption("csv");
  await f.button("Export Download").click(); const index = await f.latest("/events/export");
  await f.respond(old, []);
  await f.page.evaluate(index => fixture.response(index, "timestamp,source\nfixture,value\n", {
    "content-type": "text/csv; charset=utf-8", "content-disposition": 'attachment; filename="../../unsafe.exe"',
  }), index);
  await f.page.waitForFunction(() => fixture.downloads.length === 1); await f.advance(1);
  assert.equal((await f.requests())[old].aborted, true);
  assert.equal((await f.act(() => fixture.downloads))[0].filename, "cyanrex-events.csv");
}));

test("an export deadline covers a stalled body and returns control without a download or automatic retry", () => run(async f => {
  await f.ready(); await f.button("Export Download").click(); const index = await f.latest("/events/export");
  await f.page.evaluate(index => {
    const { init, resolve } = fixture.requests[index];
    resolve({ ok: true, headers: new Headers({ "content-type": "application/json" }), blob: () => new Promise((_resolve, reject) => {
      init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true });
    }) });
  }, index);
  await f.advance(20000);
  assert.equal((await f.requests())[index].aborted, true); assert.equal(await f.button("Export Download").isDisabled(), false);
  assert.deepEqual(await f.act(() => fixture.downloads), []); assert.equal((await f.ids("/events/export")).length, 1);
}));

test("returning to an earlier filter cannot resurrect an aborted export's busy state", () => run(async f => {
  await f.ready(); await f.button("Export Download").click();
  await f.page.locator("main.content select").first().selectOption("platform");
  await f.page.locator("main.content select").first().selectOption("all");
  assert.equal(await f.button("Export Download").isDisabled(), false);
  await f.button("Export Download").click(); assert.equal((await f.ids("/events/export")).length, 2);
}));

test("deletion has a complete waiting deadline and its failure never retries the mutation", () => run(async f => {
  await f.act(() => { fixture.honorAbort = true; }); await f.ready(); await f.approveDelete();
  const index = await f.latest("/events/delete"); await f.advance(20000);
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.requests())[index].aborted, true); assert.equal(await f.page.locator("article").count(), 1);
  assert.equal((await f.ids("/events/delete")).length, 1);
  assert.equal(await f.page.getByRole("dialog").getByRole("button", { name: /Confirm/ }).count(), 0);
}));

test("explicit deletion authorization rejection is not mislabeled as transport uncertainty", () => run(async f => {
  await f.ready(); await f.approveDelete(); await f.respond(await f.latest("/events/delete"), { message: "fixture forbidden" }, 403);
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.match(await f.page.getByRole("dialog").textContent(), /operation was rejected/);
  assert.doesNotMatch(await f.page.getByRole("dialog").textContent(), /Deletion could not be confirmed/);
}));

test("confirmed deletion refreshes history and invalidates an older unread response", () => run(async f => {
  await f.act(() => { fixture.holdUnread = true; }); await f.ready();
  const old = await f.latest("/events/unread-count"); await f.approveDelete();
  await f.respond(await f.latest("/events/delete"), { ok: true, deleted: 1 });
  await f.page.getByRole("dialog").waitFor({ state: "detached" }); await f.advance(1);
  const fresh = await f.latest("/events/unread-count"); assert.notEqual(fresh, old);
  await f.respond(fresh, { unread: 0 }); await f.respond(old, { unread: 99 });
  await f.respond(await f.latest("/events"), []);
  assert.equal((await f.requests())[old].aborted, true); assert.equal(await f.page.locator(".nav-badge").count(), 0);
  assert.equal(await f.page.locator("article").count(), 0);
  const deletion = (await f.requests()).find(request => new URL(request.url).pathname === "/events/delete");
  assert.ok(new URL(deletion.url).searchParams.get("end")); assert.equal(new URL(deletion.url).searchParams.has("limit"), false);
}));

test("pending read acknowledgement is single-flight and later live arrivals request one follow-up", () => run(async f => {
  await f.ready(); await f.advance(1200); const index = await f.latest("/events/mark-read");
  await f.page.evaluate(event => fixture.live(event), row("two")); await f.advance(1200);
  assert.equal((await f.ids("/events/mark-read")).length, 1);
  await f.respond(index, { ok: true }); await f.advance(1200);
  assert.equal((await f.ids("/events/mark-read")).length, 2);
  const request = (await f.requests())[index]; assert.equal(new URL(request.url).search, ""); assert.equal(request.body, undefined);
}));

test("read acknowledgement failure requires a manual retry and an explicit true response", () => run(async f => {
  await f.ready(); await f.advance(1200); await f.respond(await f.latest("/events/mark-read"), { ok: "true" });
  await f.button("Retry read acknowledgement").waitFor(); await f.button("Retry read acknowledgement").click();
  await f.advance(1200); assert.equal((await f.ids("/events/mark-read")).length, 2);
  await f.act(() => { fixture.unread = 0; }); await f.respond(await f.latest("/events/mark-read"), { ok: true });
  assert.equal(await f.page.locator(".nav-badge").count(), 0); assert.equal(await f.button("Retry read acknowledgement").count(), 0);
}));

test("leaving before acknowledgement debounce sends no mark-read mutation", () => run(async f => {
  await f.ready(); await f.act(() => fixture.render(false)); await f.advance(1200);
  assert.equal((await f.ids("/events/mark-read")).length, 0);
}));

test("leaving with a pending acknowledgement aborts it and ignores a late success", () => run(async f => {
  await f.ready(); await f.advance(1200); const index = await f.latest("/events/mark-read");
  await f.act(() => fixture.render(false)); const count = (await f.requests()).length;
  await f.respond(index, { ok: true }); assert.equal((await f.requests())[index].aborted, true);
  assert.equal((await f.requests()).length, count);
}));

test("unread timeout is visible and a later nonoverlapping poll can recover", () => run(async f => {
  await f.act(() => { fixture.holdUnread = true; fixture.honorAbort = true; }); await f.mount();
  const index = await f.latest("/events/unread-count"); await f.advance(10000);
  assert.equal((await f.requests())[index].aborted, true); assert.equal(await f.page.locator(".nav-badge").textContent(), "?");
  await f.advance(4000); await f.respond(await f.latest("/events/unread-count"), { unread: 7 });
  assert.equal(await f.page.locator(".nav-badge").textContent(), "7");
}));

test("refreshing the same history preserves its gap notice while a new filter gets a new scope", () => run(async f => {
  await f.ready(); await f.act(() => fixture.sockets.filter(socket => !socket.closed).at(-1).onmessage({ data: "{broken" }));
  const gap = f.page.getByRole("status").filter({ hasText: /some events may be missing/ }); await gap.waitFor();
  await f.button("Refresh history").click(); await f.advance(1); await f.respond(await f.latest("/events"), [row()]);
  assert.equal(await gap.count(), 1);
  await f.page.locator("main.content select").first().selectOption("platform"); assert.equal(await gap.count(), 0);
}));

test("late snapshots and socket callbacks cannot replace a changed history filter", () => run(async f => {
  await f.mount(); const old = await f.latest("/events");
  await f.page.locator("main.content select").first().selectOption("platform"); await f.advance(1);
  await f.respond(await f.latest("/events"), [row("current", "platform")]); await f.respond(old, [row("obsolete")]);
  await f.page.evaluate(event => fixture.sockets[1].onmessage({ data: JSON.stringify(event) }), row("late"));
  assert.deepEqual(await f.page.locator("article strong").allTextContents(), ["fixture.current"]);
  assert.equal((await f.requests())[old].aborted, true);
}));

test("bounded history preserves repeated events and time ticks do not query or acknowledge repeatedly", () => run(async f => {
  await f.act(() => { fixture.saved.events_range_v1 = "10m"; }); await f.ready();
  await f.page.evaluate(event => { for (let index = 0; index < 205; index++) fixture.live(event); }, row("repeat"));
  assert.equal(await f.page.locator("article").count(), 200);
  await f.advance(3000); assert.equal((await f.ids("/events")).length, 1); assert.equal((await f.ids("/events/mark-read")).length, 1);
}));

test("changing filters cancels a pending read acknowledgement from the previous view", () => run(async f => {
  await f.ready(); await f.advance(1200); const index = await f.latest("/events/mark-read");
  await f.page.locator("main.content select").first().selectOption("platform");
  assert.equal((await f.requests())[index].aborted, true);
  await f.respond(index, { ok: true }); assert.equal(await f.page.locator(".nav-badge").textContent(), "3");
}));

test("changing filters before read debounce cannot acknowledge the obsolete snapshot", () => run(async f => {
  await f.ready(); await f.page.locator("main.content select").first().selectOption("platform");
  await f.advance(1200); assert.equal((await f.ids("/events/mark-read")).length, 0);
}));
