import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { baseUrl, engineUrl, setupTask } from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });

const environment = () => ({ overall_ok: true, generated_at: "2026-10-18T00:00:00Z",
  runtime_mode: "native-linux", runtime_guidance: "Native Linux backend; eBPF programs target the host kernel.",
  checks: [{ name: "kernel_btf", ok: true, detail: "/sys/kernel/btf/vmlinux exists" }] });
const runner = () => ({ mode: "local_process", isolation: "shared_kernel", instance_id: "fixture-helper-instance",
  max_concurrent: 2, max_per_user: 1, active_total: 0, active_for_current_user: 0,
  available_slots: 2, execution_timeout_seconds: 45 });
const runButton = page => page.getByRole("button", { name: "运行环境检查", exact: true });
const headers = { "access-control-allow-origin": new URL(baseUrl).origin,
  "access-control-allow-credentials": "true", "cache-control": "no-store" };
async function fixture(body) {
  const f = await setupTask(browser);
  let failure;
  try { await body(f); } catch (error) { failure = error; }
  finally {
    try { await f.assertLocalOnly(); }
    catch (error) { failure = failure ? new AggregateError([failure, error], "Helper behavior and request isolation failed") : error; }
    finally { await f.close(); }
  }
  if (failure) throw failure;
}
async function reads(f, handler) {
  const calls = [];
  for (const path of ["/helper/environment", "/runner/status"]) {
    await f.page.context().route(`${engineUrl}${path}`, async route => {
      const request = route.request();
      assert.equal(request.method(), "GET"); assert.equal(request.postData(), null);
      calls.push(path);
      await handler(route, path, calls);
    });
  }
  return calls;
}

test("a failed explicit recheck cannot leave the previous successful environment or Runner report displayed", { timeout: 45000 }, () => fixture(async f => {
  let failed = false;
  const calls = await reads(f, (route, path) => route.fulfill({ headers,
    status: failed ? 503 : 200, json: failed ? { message: "private synthetic failure must stay hidden" }
      : path === "/helper/environment" ? environment() : runner() }));
  await f.page.goto(`${baseUrl}/helper`);
  await runButton(f.page).click();
  await f.page.getByText("native-linux", { exact: true }).waitFor();
  await f.page.getByText("fixture-helper-instance", { exact: true }).waitFor();
  failed = true;
  await runButton(f.page).click();
  await runButton(f.page).waitFor();
  assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 0,
    "an old successful environment report is not current readiness after the recheck fails");
  assert.equal(await f.page.getByText("fixture-helper-instance", { exact: true }).count(), 0);
  assert.equal(calls.length, 4);
  assert.doesNotMatch(await f.page.locator("main").innerText(), /private synthetic failure/);
}));

test("helper ignores legacy stored reports and performs no reads on mount navigation or locale changes", { timeout: 45000 }, () => fixture(async f => {
  const calls = await reads(f, (route, path) => route.fulfill({ headers, json: path === "/helper/environment" ? environment() : runner() }));
  await f.page.evaluate(([report, status]) => {
    sessionStorage.setItem("helper_report_v1", JSON.stringify(report));
    sessionStorage.setItem("helper_runner_status_v1", JSON.stringify(status));
    sessionStorage.setItem("helper_error_v1", "old account error");
  }, [environment(), runner()]);
  await f.page.goto(`${baseUrl}/helper`);
  await runButton(f.page).waitFor();
  assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 0);
  assert.equal(await f.page.getByText("fixture-helper-instance", { exact: true }).count(), 0);
  for (const locale of ["en", "es", "ja", "ko", "zh-CN"]) {
    await f.page.locator(".sidebar-footer select").selectOption(locale);
    await f.page.waitForFunction(value => document.documentElement.lang === value, locale);
    assert.doesNotMatch(await f.page.locator("main").innerText(), /helper\.(?:explicitCheckHint|checkFailed)|old account error/);
  }
  assert.deepEqual(calls, []);
  await runButton(f.page).click(); await f.page.getByText("native-linux", { exact: true }).waitFor();
  await f.page.evaluate(() => window.next.router.push("/helper#same-page"));
  await f.page.waitForURL(`${baseUrl}/helper#same-page`); await runButton(f.page).waitFor();
  assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 0);
  assert.equal(calls.length, 2);
  assert.deepEqual(await f.page.evaluate(() => [JSON.parse(sessionStorage.getItem("helper_report_v1")),
    JSON.parse(sessionStorage.getItem("helper_runner_status_v1")), sessionStorage.getItem("helper_error_v1")]),
    [environment(), runner(), "old account error"], "neither reads nor overwrites former cross-owner diagnostic storage");
}));

test("a pending recheck clears the entire previous pair and does not show a successful half", { timeout: 45000 }, () => fixture(async f => {
  let hold = false, release, started;
  const pending = new Promise(resolve => { release = resolve; });
  const pendingStarted = new Promise(resolve => { started = resolve; });
  let waiting = 0;
  const calls = await reads(f, async (route, path) => {
    if (hold && path === "/runner/status") { waiting++; started(); await pending; }
    try { await route.fulfill({ headers, json: path === "/helper/environment" ? environment() : runner() }); }
    catch (error) { if (!hold) throw error; }
  });
  try {
    await f.page.goto(`${baseUrl}/helper`); await runButton(f.page).click();
    await f.page.getByText("fixture-helper-instance", { exact: true }).waitFor();
    hold = true; await runButton(f.page).click();
    await f.page.getByRole("button", { name: "检查中...", exact: true }).waitFor();
    assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 0);
    assert.equal(await f.page.getByText("fixture-helper-instance", { exact: true }).count(), 0);
    await pendingStarted;
    assert.equal(waiting, 1); assert.equal(calls.length, 4);
    release(); await f.page.getByText("fixture-helper-instance", { exact: true }).waitFor();
  } finally { release(); }
}));

for (const [label, report, status] of [
  ["malformed environment", { ...environment(), checks: null }, runner()],
  ["contradictory readiness", { ...environment(), overall_ok: false }, runner()],
  ["malformed Runner", environment(), { ...runner(), available_slots: -1 }],
]) {
  test(`${label} rejects the whole pair and leaves the rest of the helper usable`, { timeout: 45000 }, () => fixture(async f => {
    const calls = await reads(f, (route, path) => route.fulfill({ headers, json: path === "/helper/environment" ? report : status }));
    await f.page.goto(`${baseUrl}/helper`); await runButton(f.page).click();
    await f.page.getByRole("alert").waitFor();
    assert.equal(await runButton(f.page).isEnabled(), true);
    assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 0);
    assert.equal(await f.page.getByText("fixture-helper-instance", { exact: true }).count(), 0);
    assert.equal(calls.length, 2);
    assert.equal(await f.page.locator('nav a[href="/helper"]').count(), 1);
  }));
}

test("language changes translate a verified report without rereading or altering raw process output", { timeout: 45000 }, () => fixture(async f => {
  const report = environment(), raw = "clang helper fixture\nOriginal raw diagnostic: keep exact ☕";
  report.checks[0] = { name: "clang", ok: true, detail: raw };
  const calls = await reads(f, (route, path) => route.fulfill({ headers, json: path === "/helper/environment" ? report : runner() }));
  await f.page.goto(`${baseUrl}/helper`); await runButton(f.page).click();
  await f.page.getByText("native-linux", { exact: true }).waitFor();
  for (const locale of ["en", "es", "ja", "ko", "zh-CN"]) {
    await f.page.locator(".sidebar-footer select").selectOption(locale);
    await f.page.waitForFunction(value => document.documentElement.lang === value, locale);
    assert.equal(await f.page.getByText(raw, { exact: true }).textContent(), raw);
    assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 1);
  }
  assert.equal(calls.length, 2);
}));

test("unmounted and same-page old owners cannot publish late abort-ignoring responses into a fresh explicit check", { timeout: 45000 }, () => fixture(async f => {
  await f.page.goto(`${baseUrl}/helper`); await runButton(f.page).waitFor();
  // This one case deliberately ignores fetch cancellation to exercise the page-owner boundary.
  // The other cases use real same-origin routes; none proves production authentication.
  await f.page.evaluate(([report, status]) => {
    const original = window.fetch, state = { requests: [], original,
      release(index) { const request = this.requests[index]; request.resolve(Response.json(request.path === "/helper/environment" ? report : status)); } };
    window.helperHeld = state;
    window.fetch = (url, init) => {
      const path = new URL(String(url), location.href).pathname;
      return ["/helper/environment", "/runner/status"].includes(path)
        ? new Promise(resolve => state.requests.push({ path, signal: init.signal, resolve })) : original(url, init);
    };
  }, [environment(), runner()]);
  try {
    await runButton(f.page).click(); await f.page.waitForFunction(() => window.helperHeld.requests.length === 2);
    await f.page.evaluate(() => window.next.router.push("/account")); await f.page.waitForURL(`${baseUrl}/account`);
    assert.equal(await f.page.evaluate(() => window.helperHeld.requests.every(request => request.signal.aborted)), true);
    await f.page.locator('nav a[href="/helper"]').click(); await runButton(f.page).waitFor();
    assert.equal(await f.page.evaluate(() => window.helperHeld.requests.length), 2, "navigation does not run another check");
    await runButton(f.page).click(); await f.page.waitForFunction(() => window.helperHeld.requests.length === 4);
    await f.page.evaluate(() => { window.helperHeld.release(0); window.helperHeld.release(1); });
    assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 0);
    await f.page.evaluate(() => window.next.router.push("/helper#new-owner")); await f.page.waitForURL(`${baseUrl}/helper#new-owner`);
    await runButton(f.page).waitFor();
    assert.equal(await f.page.evaluate(() => window.helperHeld.requests.every(request => request.signal.aborted)), true);
    await f.page.evaluate(() => { window.helperHeld.release(2); window.helperHeld.release(3); });
    assert.equal(await f.page.getByText("native-linux", { exact: true }).count(), 0);
    await runButton(f.page).click(); await f.page.waitForFunction(() => window.helperHeld.requests.length === 6);
    await f.page.evaluate(() => { window.helperHeld.release(4); window.helperHeld.release(5); });
    await f.page.getByText("fixture-helper-instance", { exact: true }).waitFor();
  } finally {
    await f.page.evaluate(() => { window.fetch = window.helperHeld.original; delete window.helperHeld; });
  }
}));
