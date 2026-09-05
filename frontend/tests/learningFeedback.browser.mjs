import assert from "node:assert/strict";
import test from "node:test";

// Optional browser smoke: run against a local production frontend, with all Engine calls mocked.
const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";

test("teacher feedback editing, conflict recovery, and student history", { timeout: 60000 }, async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  try {
    const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } });
    await context.addInitScript(() => localStorage.setItem("cyanrex_locale", "zh-CN"));
    const page = await context.newPage();
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));
    let role = "teacher";
    let failNextSave = false;
    const writes = [];
    const attempts = Array.from({ length: 21 }, (_, index) => ({
      id: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
      username: "student", lab_id: "01-first-program", template_id: "xdp-pass",
      source: `// Submission ${index}\nSEC("xdp") int pass(void *ctx) { return XDP_PASS; }`,
      source_sha256: "a".repeat(64), run_success: true, stage: "run",
      attach_expected: false, attach_verified: false, completed: true,
      feedback: ["Automated acceptance passed"], teacher_feedback: null,
      created_at: new Date(Date.UTC(2026, 8, 5, 12, 0, 59 - index)).toISOString(),
    }));
    const current = attempts[0];
    const remoteFeedback = (comment, revision) => ({ reviewer: "another-teacher", comment, revision, updated_at: "2026-09-05T13:00:00Z" });
    await context.route(`${engineUrl}/**`, async (route) => {
      const request = route.request();
      const url = new URL(request.url());
      const path = url.pathname;
      const headers = {
        "access-control-allow-origin": new URL(baseUrl).origin,
        "access-control-allow-credentials": "true",
        "access-control-allow-methods": "GET, POST, OPTIONS",
        "access-control-allow-headers": "content-type",
      };
      const reply = (json, status = 200) => route.fulfill({ json, status, headers });
      if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
      if (path === "/auth/me") return reply({ authenticated: true, username: role, role });
      if (path === "/events/unread-count") return reply({ unread: 0 });
      if (path === "/learning/labs") return reply([]);
      if (path === "/learning/attempts") return reply(role === "student" ? attempts : []);
      if (path === "/learning/teacher/overview") return reply({
        generated_at: current.created_at, total_labs: 5, active_students: 1,
        students: [{ username: "student", completed_labs: 1, total_labs: 5, total_attempts: 21, last_activity_at: current.created_at, labs: [] }],
      });
      if (path === "/learning/teacher/attempts") {
        assert.equal(url.searchParams.get("username"), "student");
        return reply({ username: "student", attempts: attempts.slice(0, Number(url.searchParams.get("limit"))) });
      }
      if (path === "/learning/teacher/feedback" && request.method() === "POST") {
        const body = request.postDataJSON();
        writes.push(body);
        assert.equal(body.username, "student");
        assert.equal(body.attempt_id, current.id);
        if (failNextSave) { failNextSave = false; return reply({ ok: false, message: "storage unavailable" }, 500); }
        if (body.expected_revision !== (current.teacher_feedback?.revision ?? 0)) {
          return reply({ ok: false, message: "feedback changed; reload before editing" }, 409);
        }
        current.teacher_feedback = { ...remoteFeedback(body.comment, body.expected_revision + 1), reviewer: "teacher" };
        return reply(current.teacher_feedback);
      }
      throw new Error(`Unexpected mocked API request: ${request.method()} ${path}`);
    });

    await page.goto(`${baseUrl}/teaching`);
    await page.getByRole("button", { name: "查看尝试", exact: true }).click();
    const editor = page.getByLabel("编写评语", { exact: true }).first();
    const save = page.getByRole("button", { name: "保存评语", exact: true }).first();
    await editor.waitFor();
    assert.ok((await editor.boundingBox()).height <= 180, "feedback must not inherit the full code editor height");
    assert.equal(await save.isDisabled(), true);
    await editor.fill("😀".repeat(2001));
    assert.equal(await save.isDisabled(), true);
    await editor.fill("  检查指针边界。\nExplain the guard.  ");
    await save.click();
    await page.getByRole("status").filter({ hasText: "评语已保存" }).waitFor();
    assert.equal(current.teacher_feedback.comment, "检查指针边界。\nExplain the guard.");
    assert.equal(writes[0].expected_revision, 0);
    assert.equal(await save.isDisabled(), true);

    current.teacher_feedback = remoteFeedback("另一位教师已经更新", 2);
    await editor.fill("我的修改草稿");
    await save.click();
    await page.getByRole("alert").filter({ hasText: "评语已被修改" }).waitFor();
    assert.equal(await editor.inputValue(), "我的修改草稿");
    assert.equal(await save.isDisabled(), true);
    await page.getByRole("button", { name: "加载最新评语", exact: true }).click();
    await page.getByText("另一位教师已经更新", { exact: true }).waitFor();
    assert.equal(await editor.inputValue(), "我的修改草稿");
    await save.click();
    await page.getByRole("status").filter({ hasText: "评语已保存" }).waitFor();
    assert.equal(writes.at(-1).expected_revision, 2);
    assert.equal(current.teacher_feedback.revision, 3);

    failNextSave = true;
    const plainText = "<img src=x onerror=alert(1)>\n失败时保留的草稿";
    await editor.fill(plainText);
    await save.click();
    await page.getByRole("alert").filter({ hasText: "评语保存失败" }).waitFor();
    assert.equal(await editor.inputValue(), plainText);
    assert.equal(current.teacher_feedback.revision, 3);
    await save.click();
    await page.getByRole("status").filter({ hasText: "评语已保存" }).waitFor();
    assert.equal(current.teacher_feedback.revision, 4);
    if (process.env.CYANREX_UI_SCREENSHOT) {
      await editor.scrollIntoViewIfNeeded();
      await page.screenshot({ path: `${process.env.CYANREX_UI_SCREENSHOT}.teacher.png` });
    }

    role = "student";
    await page.goto(`${baseUrl}/learn`);
    await page.getByRole("heading", { name: "我的实验记录", exact: true }).waitFor();
    await page.getByText(plainText, { exact: true }).waitFor();
    assert.equal(await page.getByLabel("编写评语", { exact: true }).count(), 0);
    assert.equal(await page.getByRole("link", { name: "教学管理", exact: true }).count(), 0);
    assert.equal(await page.locator('img[src="x"]').count(), 0, "feedback must render as escaped text");
    assert.equal(await page.locator("article").count(), 20);
    await page.getByRole("button", { name: "加载更多记录", exact: true }).click();
    assert.equal(await page.locator("article").count(), 21);
    current.teacher_feedback = remoteFeedback("刷新后的教师评语", 5);
    await page.getByRole("button", { name: "刷新", exact: true }).click();
    await page.getByText("刷新后的教师评语", { exact: true }).waitFor();
    if (process.env.CYANREX_UI_SCREENSHOT) await page.screenshot({ path: process.env.CYANREX_UI_SCREENSHOT, fullPage: false });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.getByText("刷新后的教师评语", { exact: true }).scrollIntoViewIfNeeded();
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true);
    if (process.env.CYANREX_UI_SCREENSHOT) await page.screenshot({ path: `${process.env.CYANREX_UI_SCREENSHOT}.mobile.png` });
    assert.deepEqual(pageErrors, []);
  } finally { await browser.close(); }
});
