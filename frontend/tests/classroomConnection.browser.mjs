import assert from "node:assert/strict";
import test from "node:test";

const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";
const id = "23d40d83-19de-43d3-85fe-506d86592a70";
const token = "a".repeat(64); // Synthetic, never an issued credential.
const joinUrl = `${baseUrl}/join#invite=${token}&classroom=${id}&protocol=1`;

async function setup(descriptor = {}) {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } });
  await context.addInitScript(() => {
    localStorage.setItem("cyanrex_locale", "zh-CN");
    window.joinSettled = 0;
    const originalFetch = window.fetch;
    window.fetch = async (...args) => {
      try { return await originalFetch(...args); }
      finally { if (String(args[0]).endsWith("/classroom/join")) window.joinSettled += 1; }
    };
  });
  const page = await context.newPage();
  const writes = [], errors = [], urls = [];
  const state = { joinFailure: null, hold: null };
  const invitation = { invite_id: id, username: "student-one", expires_at: "2099-09-09T12:10:00Z", join_url: joinUrl };
  let inventory = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => urls.push(request.url()));
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === new URL(baseUrl).origin) return route.continue();
    errors.push("Blocked an unmocked external request");
    return route.abort();
  });
  await context.route(`${engineUrl}/**`, async route => {
    const req = route.request();
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, POST, OPTIONS", "access-control-allow-headers": "content-type", "cache-control": "no-store" };
    const reply = (json, status = 200) => route.fulfill({ json, headers, status });
    if (req.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    const path = new URL(req.url()).pathname;
    if (req.method() === "POST") writes.push({ path, body: req.postDataJSON() });
    if (path === "/.well-known/cyanrex-classroom") return reply({ service: "cyanrex-classroom", classroom_id: id,
      display_name: "教师测试课堂", product_version: "0.3.99", protocol_min: 1, protocol_max: 1,
      join_url: `${baseUrl}/join`, capabilities: ["student-invite-v1"], ...descriptor });
    if (path === "/classroom/join") {
      if (state.hold) await state.hold;
      if (state.joinFailure === "network") return route.abort("failed");
      if (state.joinFailure === "malformed") return route.fulfill({ body: "invalid JSON", headers, status: 200 });
      if (state.joinFailure === "storage") return reply({ ok: false, message: "synthetic unconfirmed storage" }, 503);
      if (state.joinFailure === "incomplete") return reply({ ok: true, account_name: "student-one" }, 201);
      return reply({ ok: true, account_name: "student-one", secret: "JBSWY3DPEHPK3PXP", otpauth_uri: "otpauth://totp/Test:student-one?secret=JBSWY3DPEHPK3PXP&issuer=Test" }, 201);
    }
    if (path === "/classroom/invitations" && req.method() === "POST") { inventory = [invitation]; return reply(invitation, 201); }
    if (path === "/classroom/invitations") return reply({ invitations: inventory.map(({ join_url, ...safe }) => safe) });
    if (path === "/classroom/invitations/revoke") { inventory = []; return reply({ ok: true }); }
    if (path === "/auth/me") return reply({ authenticated: true, username: "teacher", role: "teacher" });
    if (path === "/events/unread-count") return reply({ unread: 0 });
    if (path === "/learning/teacher/overview") return reply({ active_students: 0, total_labs: 5, students: [] });
    return reply({});
  });
  return { browser, page, writes, errors, urls, state };
}

async function confirmJoin(f) {
  await f.page.goto(joinUrl);
  await f.page.getByTestId("classroom-identity").waitFor();
  await f.page.getByLabel("用户名", { exact: true }).fill("student-one");
  await f.page.getByLabel("密码", { exact: true }).fill("student-password-123");
  await f.page.getByLabel("确认密码", { exact: true }).fill("student-password-123");
  await f.page.getByRole("checkbox").check();
  await f.page.getByRole("button", { name: "加入教师课堂", exact: true }).click();
  await f.page.getByRole("dialog").getByRole("button", { name: "确认加入教师课堂", exact: true })
    .evaluate(button => { button.click(); button.click(); });
}

for (const failure of ["network", "malformed", "storage", "incomplete"]) {
  test(`uncertain classroom enrollment (${failure}) clears secrets and cannot replay the invitation`, { timeout: 30000 }, async () => {
    const f = await setup();
    try {
      f.state.joinFailure = failure;
      await confirmJoin(f);
      await f.page.getByRole("dialog").getByRole("alert").waitFor();
      assert.equal(f.writes.length, 1, "duplicate confirmation and errors never replay the request");
      assert.equal(await f.page.getByTestId("classroom-enrolled").count(), 0);
      assert.equal(await f.page.locator('input[type="password"]').count(), 0);
      await f.page.getByRole("dialog").getByRole("button", { name: "关闭", exact: true }).click();
      await f.page.getByText(/请向教师领取新的私密单次邀请链接/).waitFor();
      assert.equal(await f.page.getByRole("button", { name: "加入教师课堂", exact: true }).count(), 0);
      assert.equal(await f.page.evaluate(token => JSON.stringify([history.state, localStorage, sessionStorage]).includes(token), token), false);
      await f.page.reload();
      await f.page.getByText(/请向教师领取新的私密单次邀请链接/).waitFor();
      assert.equal(f.writes.length, 1);
      assert.equal(f.urls.some(url => url.includes(token)), false);
      assert.deepEqual(f.errors, []);
    } finally { await f.browser.close(); }
  });
}

test("leaving an in-flight classroom enrollment discards the late secret without replay", { timeout: 30000 }, async () => {
  const f = await setup();
  let release;
  try {
    f.state.hold = new Promise(resolve => { release = resolve; });
    await confirmJoin(f);
    await f.page.getByRole("dialog").getByRole("status").waitFor();
    await f.page.evaluate(() => { void window.next.router.push("/login"); });
    await f.page.waitForURL(`${baseUrl}/login`);
    release();
    await f.page.waitForFunction(() => window.joinSettled === 1);
    assert.equal(await f.page.getByTestId("classroom-enrolled").count(), 0);
    assert.equal(await f.page.evaluate(() => JSON.stringify([history.state, localStorage, sessionStorage, document.body.textContent]).includes("JBSWY3DPEHPK3PXP")), false);
    await f.page.goto(`${baseUrl}/join`);
    await f.page.getByText(/请向教师领取新的私密单次邀请链接/).waitFor();
    assert.equal(f.writes.length, 1);
    assert.deepEqual(f.errors, []);
  } finally { release?.(); await f.browser.close(); }
});

test("student confirms teacher identity before one-time join, keeps secrets out of history and storage", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(joinUrl);
    await f.page.getByTestId("classroom-identity").waitFor();
    if (process.env.CYANREX_UI_SCREENSHOT) await f.page.screenshot({ path: process.env.CYANREX_UI_SCREENSHOT, fullPage: true });
    await f.page.getByLabel("用户名", { exact: true }).fill("student-one");
    await f.page.getByLabel("密码", { exact: true }).fill("student-password-123");
    await f.page.getByLabel("确认密码", { exact: true }).fill("student-password-123");
    const join = f.page.getByRole("button", { name: "加入教师课堂", exact: true });
    assert.equal(await join.isDisabled(), true);
    assert.equal(f.writes.length, 0);
    assert.equal(f.page.url().includes(token), false);
    assert.equal(await f.page.evaluate(token => JSON.stringify(history.state).includes(token), token), false);
    await f.page.getByRole("checkbox").check();
    await join.click();
    await f.page.getByRole("dialog").getByRole("button", { name: "取消", exact: true }).click();
    assert.equal(f.writes.length, 0);
    await join.click();
    await f.page.getByRole("dialog").getByRole("button", { name: "确认加入教师课堂", exact: true }).click();
    await f.page.getByTestId("classroom-enrolled").waitFor();
    assert.equal(f.writes.length, 1);
    assert.equal(f.writes[0].path, "/classroom/join");
    assert.equal(f.writes[0].body.classroom_id, id);
    assert.equal(f.writes[0].body.protocol_version, 1);
    assert.equal(f.writes[0].body.invite_token, token);
    assert.equal(await f.page.evaluate(() => JSON.stringify([localStorage, sessionStorage]).includes("JBSWY3DPEHPK3PXP")), false);
    assert.equal(f.urls.some(url => url.includes(token)), false, "fragment never becomes a request URL");
    await f.page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await f.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await f.page.reload();
    await f.page.getByText(/请向教师领取新的私密单次邀请链接/).waitFor();
    assert.equal(await f.page.getByTestId("classroom-enrolled").count(), 0);
    assert.equal(f.writes.length, 1);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});

test("incompatible discovery never offers enrollment or follows another classroom", { timeout: 45000 }, async () => {
  for (const descriptor of [{ protocol_min: 2, protocol_max: 2 }, { classroom_id: "23d40d83-19de-43d3-85fe-506d86592a71" }, { join_url: "https://evil.example/join" }]) {
    const f = await setup(descriptor);
    try {
      await f.page.goto(joinUrl);
      await f.page.locator(".auth-card [role=alert]").first().waitFor();
      assert.equal(await f.page.getByRole("button", { name: "加入教师课堂", exact: true }).count(), 0);
      assert.equal(f.writes.length, 0);
      assert.equal(f.urls.some(url => url.includes("evil.example")), false);
      assert.deepEqual(f.errors, []);
    } finally { await f.browser.close(); }
  }
});

test("teacher confirms exact student invitation and revocation; folding hides the private link", { timeout: 45000 }, async () => {
  const f = await setup();
  try {
    await f.page.goto(`${baseUrl}/teaching`);
    await f.page.getByText("学生发现与邀请接入", { exact: true }).click();
    await f.page.getByLabel("指定学生用户名", { exact: true }).fill("student-one");
    await f.page.getByRole("button", { name: "创建单次邀请", exact: true }).click();
    await f.page.getByRole("dialog").getByRole("button", { name: "取消", exact: true }).click();
    assert.equal(f.writes.length, 0);
    await f.page.getByRole("button", { name: "创建单次邀请", exact: true }).click();
    await f.page.getByRole("dialog").getByRole("button", { name: "确认创建单次邀请", exact: true }).click();
    await f.page.getByLabel("私密邀请链接", { exact: true }).waitFor();
    assert.equal(f.writes.length, 1);
    assert.equal(await f.page.getByLabel("私密邀请链接", { exact: true }).inputValue(), joinUrl);
    await f.page.getByText("学生发现与邀请接入", { exact: true }).click();
    await f.page.getByLabel("私密邀请链接", { exact: true }).waitFor({ state: "detached" });
    assert.equal(await f.page.getByLabel("私密邀请链接", { exact: true }).count(), 0);
    await f.page.getByText("学生发现与邀请接入", { exact: true }).click();
    await f.page.getByRole("button", { name: "撤销邀请", exact: true }).click();
    await f.page.getByRole("dialog").getByRole("button", { name: "确认撤销邀请", exact: true }).click();
    await f.page.waitForFunction(() => ![...document.querySelectorAll("button")].some(button => button.textContent === "撤销邀请"));
    assert.deepEqual(f.writes.map(write => write.path), ["/classroom/invitations", "/classroom/invitations/revoke"]);
    assert.deepEqual(f.errors, []);
  } finally { await f.browser.close(); }
});
