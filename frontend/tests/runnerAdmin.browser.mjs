import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { buildFixture, setupFixture } from "./helpers/compilerDiagnosticsBrowser.mjs";
import { agentInventory, jobInventory, runnerAgent, runnerJob } from "./fixtures/runnerData.mjs";

let browser, bundle;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const stub = fileURLToPath(new URL("./fixtures/eventPageStubs.mjs", import.meta.url));
  bundle = await buildFixture(new URL("./fixtures/runnerAdmin.mjs", import.meta.url), {
    "next/router$": stub, "next/link$": stub, "next/head$": stub,
    [fileURLToPath(new URL("../src/utils/pageState.ts", import.meta.url))]: stub,
  });
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

async function run(callback) {
  const f = await setupFixture(browser, bundle); f.page.setDefaultTimeout(2500);
  f.mount = async (options = {}) => { await f.page.evaluate(options => fixture.render(options), options); await f.advance(1); };
  f.ids = async (path, method = "GET") => (await f.requests()).flatMap((r, i) => new URL(r.url).pathname === path && r.method === method ? [i] : []);
  f.latest = async (path, method) => (await f.ids(path, method)).at(-1);
  f.refresh = () => f.page.getByRole("button", { name: "Refresh Agents", exact: true });
  f.probe = () => f.page.getByRole("button", { name: "Send health probe", exact: true });
  f.cancel = () => f.page.getByRole("button", { name: "Cancel", exact: true });
  f.confirm = () => f.page.getByRole("dialog").getByRole("button", { name: /^Confirm/ });
  f.closeDialog = () => f.page.getByRole("dialog").getByRole("button", { name: "Close", exact: true }).click();
  f.waitIdle = () => f.page.waitForFunction(() => [...document.querySelectorAll("button")].some(button => button.textContent === "Refresh Agents" && !button.disabled));
  f.inventory = async (agents = agentInventory(), jobs = jobInventory()) => {
    await f.respond(await f.latest("/runner/agents"), agents);
    await f.respond(await f.latest("/runner/jobs"), jobs); await f.advance(1);
  };
  f.ready = async () => { await f.mount(); await f.inventory(); await f.cancel().waitFor(); };
  f.postCancel = async () => { await f.cancel().click(); await f.confirm().click(); await f.advance(1); return f.latest("/runner/jobs/cancel", "POST"); };
  try { await callback(f); await f.advance(1); assert.deepEqual(f.errors, []); } finally { await f.close(); }
}

test("malformed Runner inventory cannot crash neighboring settings controls", () => run(async f => {
  await f.mount({ mode: "page" });
  await f.inventory(agentInventory([runnerAgent({ labels: null })]));
  assert.equal(await f.page.getByRole("button", { name: "Save Settings", exact: true }).count(), 1);
  assert.equal(await f.page.getByRole("heading", { name: "Runner Agent Operations", exact: true }).count(), 1);
}));

test("stalled inventory reads time out and release refresh", () => run(async f => {
  await f.mount(); const old = await f.latest("/runner/agents"); await f.advance(10010);
  assert.equal((await f.requests())[old].aborted, true);
  assert.equal(await f.refresh().isDisabled(), false);
}));

test("navigation aborts obsolete inventories and starts a new owner", () => run(async f => {
  await f.mount(); const old = await f.latest("/runner/agents");
  await f.mount({ navigation: "/settings?scope=next" });
  assert.equal((await f.requests())[old].aborted, true);
  assert.notEqual(await f.latest("/runner/agents"), old);
}));

test("failed refresh keeps the previous inventory read-only", () => run(async f => {
  await f.ready(); await f.refresh().click();
  await f.respond(await f.latest("/runner/agents"), {}, 503); await f.advance(1);
  assert.equal(await f.cancel().isDisabled(), true);
  assert.equal(await f.probe().isDisabled(), true);
}));

test("a 2xx response without a matching cancellation acknowledgement is not success", () => run(async f => {
  await f.ready(); const post = await f.postCancel(); await f.respond(post, { ok: true }); await f.advance(1);
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal(await f.page.getByRole("dialog").count(), 1);
  assert.equal(await f.page.getByText("Cancellation requested for fixture-job.", { exact: true }).count(), 0);
}));

test("probe submission requires a reviewed confirmation", () => run(async f => {
  await f.ready(); await f.probe().click();
  assert.equal((await f.ids("/runner/jobs/probe", "POST")).length, 0);
  assert.equal(await f.page.getByRole("dialog").count(), 1);
}));

test("leaving the page aborts a pending cancellation without a follow-up refresh", () => run(async f => {
  await f.ready(); const post = await f.postCancel(); await f.mount(false);
  assert.equal((await f.requests())[post].aborted, true);
  const count = (await f.requests()).length;
  await f.respond(post, runnerJob({ state: "cancelled", completed_at: "2026-09-13T00:00:02Z" }));
  await f.advance(20010); assert.equal((await f.requests()).length, count);
}));

test("Strict Mode and late obsolete reads cannot overwrite the current atomic inventory", () => run(async f => {
  await f.mount(); const agents = await f.ids("/runner/agents"), jobs = await f.ids("/runner/jobs");
  assert.equal(agents.length, 2); assert.equal(jobs.length, 2);
  assert.equal((await f.requests())[agents[0]].aborted, true);
  await f.inventory(); await f.waitIdle();
  await f.respond(agents[0], agentInventory([], { enabled: false }));
  await f.respond(jobs[0], jobInventory([])); await f.advance(1);
  assert.equal(await f.cancel().count(), 1);
  for (const r of await f.requests()) {
    assert.equal(r.method, "GET"); assert.equal(r.cache, "no-store");
    assert.equal(r.redirect, "error"); assert.equal(r.credentials, "include"); assert.equal(r.body, undefined);
  }
}));

test("a failed half of the inventory pair aborts its sibling and cannot publish a partial update", () => run(async f => {
  await f.ready(); await f.refresh().click(); const old = await f.latest("/runner/agents");
  await f.respond(await f.latest("/runner/jobs"), { message: "private fixture detail" }, 500); await f.waitIdle();
  assert.equal((await f.requests())[old].aborted, true);
  await f.respond(old, agentInventory([], { enabled: false })); await f.advance(1);
  assert.equal(await f.cancel().isDisabled(), true);
  assert.doesNotMatch(await f.page.locator("body").textContent(), /private fixture detail/);
  await f.refresh().click(); await f.inventory(); await f.waitIdle(); assert.equal(await f.cancel().isDisabled(), false);
}));

test("a stalled inventory body cannot publish after a newer verified read", () => run(async f => {
  await f.mount(); const old = await f.latest("/runner/agents");
  await f.page.evaluate(index => fixture.requests[index].resolve({ ok: true,
    headers: new Headers({ "content-type": "application/json" }), json: () => new Promise(resolve => { fixture.releaseBody = resolve; }) }), old);
  await f.respond(await f.latest("/runner/jobs"), jobInventory()); await f.advance(10010); await f.waitIdle();
  assert.equal((await f.requests())[old].aborted, true);
  await f.refresh().click(); await f.inventory(); await f.waitIdle();
  await f.page.evaluate(value => fixture.releaseBody(value), agentInventory([], { enabled: false })); await f.advance(1);
  assert.equal(await f.cancel().isDisabled(), false);
}));

test("probe approval submits exactly once and validates the returned job before success", () => run(async f => {
  await f.ready(); await f.probe().click(); await f.confirm().dblclick(); await f.advance(1);
  const ids = await f.ids("/runner/jobs/probe", "POST"); assert.equal(ids.length, 1);
  const request = (await f.requests())[ids[0]];
  assert.deepEqual(request.body, { agent_id: "fixture-agent", message: "teacher health probe", timeout_seconds: 30 });
  assert.equal(request.cache, "no-store"); assert.equal(request.redirect, "error"); assert.equal(request.credentials, "include");
  await f.respond(ids[0], runnerJob({ job_id: "probe-fixture-job", kind: "control_probe", message: "teacher health probe",
    timeout_seconds: 30, owner_username: null, source_bytes: null, program_name: null }), 201);
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  await f.page.getByText("Health probe submitted to fixture-agent.", { exact: true }).waitFor();
  assert.equal(await f.probe().isDisabled(), true);
  await f.inventory(); await f.waitIdle(); assert.equal(await f.probe().isDisabled(), false);
}));

test("a cancellation acknowledgement remains confirmed if the follow-up inventory fails", () => run(async f => {
  await f.ready(); const post = await f.postCancel();
  await f.respond(post, runnerJob({ state: "cancel_requested" }));
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  await f.respond(await f.latest("/runner/jobs"), {}, 503); await f.waitIdle();
  assert.equal(await f.cancel().isDisabled(), true);
  await f.page.getByText("Cancellation requested for fixture-job.", { exact: true }).waitFor();
  assert.doesNotMatch(await f.page.locator("body").textContent(), /may already have taken effect/);
  assert.equal((await f.ids("/runner/jobs/cancel", "POST")).length, 1);
}));

test("timed-out cancellation stays uncertain through polling until explicit refresh", () => run(async f => {
  await f.ready(); const post = await f.postCancel(); await f.advance(20010);
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.requests())[post].aborted, true); await f.closeDialog();
  assert.equal(await f.cancel().isDisabled(), true);
  await f.advance(10010); await f.inventory(); await f.waitIdle();
  assert.equal(await f.cancel().isDisabled(), true, "background polling must not clear uncertainty");
  await f.respond(post, runnerJob({ state: "cancelled" })); await f.advance(1);
  assert.equal(await f.page.getByText("Cancellation requested for fixture-job.", { exact: true }).count(), 0);
  assert.equal((await f.ids("/runner/jobs/cancel", "POST")).length, 1);
  await f.refresh().click(); await f.inventory(); await f.waitIdle(); assert.equal(await f.cancel().isDisabled(), false);
}));

test("a stalled cancellation body has the same deadline and never retries the write", () => run(async f => {
  await f.ready(); const post = await f.postCancel();
  await f.page.evaluate(index => fixture.requests[index].resolve({ ok: true,
    headers: new Headers({ "content-type": "application/json" }), json: () => new Promise(resolve => { fixture.releaseBody = resolve; }) }), post);
  await f.advance(20010); await f.page.getByRole("dialog").getByRole("alert").waitFor(); await f.closeDialog();
  await f.page.evaluate(value => fixture.releaseBody(value), runnerJob({ state: "cancelled" })); await f.advance(1);
  assert.equal(await f.cancel().isDisabled(), true);
  assert.equal((await f.ids("/runner/jobs/cancel", "POST")).length, 1);
}));

test("background state changes invalidate a reviewed cancellation target", () => run(async f => {
  await f.ready(); await f.cancel().click(); await f.advance(10010);
  await f.inventory(agentInventory(), jobInventory([runnerJob({ state: "claimed", assigned_agent_id: "agent-other" })]));
  await f.confirm().click(); await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.ids("/runner/jobs/cancel", "POST")).length, 0);
  await f.closeDialog(); assert.equal(await f.cancel().isDisabled(), false, "a pre-dispatch rejection is not an uncertain write");
}));

test("a confirmation cannot dispatch while inventory refresh is still in flight", () => run(async f => {
  await f.ready(); await f.cancel().click(); await f.advance(10010); await f.confirm().click();
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.ids("/runner/jobs/cancel", "POST")).length, 0);
  await f.closeDialog(); await f.inventory(); await f.waitIdle();
  assert.equal(await f.cancel().isDisabled(), false);
}));

test("probe review rechecks Agent expiry using Engine-relative time", () => run(async f => {
  await f.mount(); await f.inventory(agentInventory([runnerAgent({ expires_at: "2026-09-13T00:00:03Z" })]));
  await f.probe().click(); await f.advance(3000); await f.confirm().click();
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.ids("/runner/jobs/probe", "POST")).length, 0);
}));

test("Engine changes close the old confirmation and abort its pending POST", () => run(async f => {
  await f.ready(); const post = await f.postCancel(); await f.mount({ engineUrl: "https://engine-b.invalid" });
  assert.equal((await f.requests())[post].aborted, true);
  assert.equal(await f.page.getByRole("dialog").count(), 0);
  await f.inventory(); await f.waitIdle(); const count = (await f.requests()).length;
  await f.respond(post, runnerJob({ state: "cancelled" })); await f.advance(1);
  assert.equal((await f.requests()).length, count);
  assert.equal(await f.page.getByText("Cancellation requested for fixture-job.", { exact: true }).count(), 0);
}));

test("navigation aborts a pending write and its late response cannot touch the next page", () => run(async f => {
  await f.ready(); const post = await f.postCancel(); await f.mount({ navigation: "/settings?scope=next" });
  assert.equal((await f.requests())[post].aborted, true);
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  assert.equal(await f.page.getByRole("dialog").count(), 0);
  await f.inventory(); await f.waitIdle(); const count = (await f.requests()).length;
  await f.respond(post, runnerJob({ state: "cancelled" })); await f.advance(1);
  assert.equal((await f.requests()).length, count);
}));

test("read polling waits after completion and is paused during writes", () => run(async f => {
  await f.mount(); const initial = (await f.ids("/runner/agents")).length;
  await f.advance(7000); assert.equal((await f.ids("/runner/agents")).length, initial);
  await f.inventory(); await f.waitIdle(); await f.advance(9000);
  assert.equal((await f.ids("/runner/agents")).length, initial);
  const post = await f.postCancel(); await f.advance(15000);
  assert.equal((await f.ids("/runner/agents")).length, initial, "polling cannot race an unacknowledged write");
  await f.respond(post, runnerJob({ state: "cancelled" }));
  await f.page.getByRole("dialog").waitFor({ state: "detached" });
  assert.equal((await f.ids("/runner/agents")).length, initial + 1);
}));

test("unmount aborts both reads, drops late data and stops timers", () => run(async f => {
  await f.mount(); const old = await f.latest("/runner/agents"), jobs = await f.latest("/runner/jobs");
  const count = (await f.requests()).length; await f.mount(false);
  assert.equal((await f.requests())[old].aborted, true); assert.equal((await f.requests())[jobs].aborted, true);
  await f.respond(old, agentInventory()); await f.respond(jobs, jobInventory()); await f.advance(40000);
  assert.equal((await f.requests()).length, count);
}));

test("HTTP failures and incorrect media types stay generic and cannot acknowledge an action", () => run(async f => {
  await f.ready(); const post = await f.postCancel();
  await f.page.evaluate(index => fixture.requests[index].resolve({ ok: true,
    headers: new Headers({ "content-type": "text/html" }), json: async () => { throw new Error("private fixture detail"); } }), post);
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.doesNotMatch(await f.page.locator("body").textContent(), /private fixture detail/); await f.closeDialog();
  await f.refresh().click(); await f.inventory(); await f.waitIdle();
  const retry = await f.postCancel(); await f.respond(retry, { message: "private fixture detail" }, 403);
  await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.doesNotMatch(await f.page.locator("body").textContent(), /private fixture detail/);
  assert.equal((await f.ids("/runner/jobs/cancel", "POST")).length, 2, "only explicitly reviewed requests are sent");
}));

test("probe confirmation can be dismissed safely and rejects a re-registered Agent", () => run(async f => {
  await f.ready(); await f.probe().click(); await f.page.keyboard.press("Escape");
  assert.equal((await f.ids("/runner/jobs/probe", "POST")).length, 0);
  await f.probe().click(); await f.advance(10010);
  await f.inventory(agentInventory([runnerAgent({ registered_at: "2026-09-13T00:00:11Z" })]));
  await f.confirm().click(); await f.page.getByRole("dialog").getByRole("alert").waitFor();
  assert.equal((await f.ids("/runner/jobs/probe", "POST")).length, 0);
}));

test("locale changes translate feedback without extra Runner reads or settings draft loss", () => run(async f => {
  await f.mount({ mode: "page" }); await f.inventory(); await f.waitIdle();
  await f.page.getByRole("spinbutton").fill("717"); await f.refresh().click();
  await f.respond(await f.latest("/runner/agents"), {}, 503); await f.waitIdle(); const count = (await f.requests()).length;
  for (const [locale, text] of [["zh-CN", "无法核实 Runner 清单"], ["es", "No se pudo verificar"], ["ja", "Runner の一覧を確認できませんでした"], ["en", "Runner inventory could not be verified"]]) {
    await f.page.evaluate(locale => fixture.setLocale(locale), locale); await f.advance(1);
    assert.ok((await f.page.locator("body").textContent()).includes(text));
    assert.equal(await f.page.getByRole("spinbutton").inputValue(), "717");
  }
  assert.equal((await f.requests()).length, count);
  assert.equal(await f.page.getByRole("button", { name: "Save Settings", exact: true }).isDisabled(), false);
  assert.equal(await f.page.getByRole("button", { name: "Refresh metrics", exact: true }).isDisabled(), false);
}));
