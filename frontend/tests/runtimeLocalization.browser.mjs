import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { baseUrl, engineUrl, setupTask } from "./helpers/taskPayloadEditorBrowser.mjs";

let browser;
before(async () => {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
});
after(async () => { await browser?.close(); });
async function readRoute(f, pathname, json, calls) {
  await f.page.context().route(`${engineUrl}${pathname}`, route => {
    const request = route.request();
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin,
      "access-control-allow-credentials": "true", "cache-control": "no-store" };
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    assert.equal(request.method(), "GET"); assert.equal(request.postData(), null);
    calls.push(pathname);
    return route.fulfill({ headers, json });
  });
}
async function run(body) {
  const f = await setupTask(browser);
  let failure;
  try { await body(f); } catch (error) { failure = error; }
  finally {
    try { await f.assertLocalOnly(); }
    catch (error) { failure = failure ? new AggregateError([failure, error], "Behavior and isolation failed") : error; }
    finally { await f.close(); }
  }
  if (failure) throw failure;
}

test("cached environment explanations follow all four locales without changing raw process output or refetching", { timeout: 60000 }, () => run(async f => {
  const calls = [];
  const raw = "clang version fixture 1.0\nunaltered raw output";
  await readRoute(f, "/helper/environment", { overall_ok: true, generated_at: "2026-10-13T00:00:00Z",
    runtime_mode: "native-linux", runtime_guidance: "Native Linux backend; eBPF programs target the host kernel.",
    checks: [{ name: "kernel_btf", ok: true, detail: "/sys/kernel/btf/vmlinux exists" },
      { name: "clang", ok: true, detail: raw }] }, calls);
  await readRoute(f, "/runner/status", { mode: "local_process", isolation: "shared_kernel", instance_id: "fixture",
    max_concurrent: 2, max_per_user: 1, active_total: 0, active_for_current_user: 0,
    available_slots: 2, execution_timeout_seconds: 45 }, calls);
  await f.page.goto(`${baseUrl}/helper`);
  await f.page.getByRole("button", { name: "运行环境检查", exact: true }).click();
  for (const [locale, guidance, label] of [
    ["zh-CN", "原生 Linux 后端；eBPF 程序操作宿主机内核。", "内核 BTF"],
    ["en", "Native Linux backend; eBPF programs target the host kernel.", "kernel_btf"],
    ["es", "Backend Linux nativo; los programas eBPF usan el kernel del anfitrión.", "BTF del kernel"],
    ["ja", "ネイティブ Linux バックエンドです。eBPF プログラムはホストのカーネルを対象とします。", "カーネル BTF"],
  ]) {
    await f.page.locator(".sidebar-footer select").selectOption(locale);
    await f.page.getByText(guidance, { exact: true }).waitFor();
    await f.page.locator("main strong").filter({ hasText: label }).waitFor();
    assert.equal(await f.page.getByText(raw, { exact: true }).textContent(), raw);
    await f.page.getByText("native-linux", { exact: true }).waitFor();
  }
  assert.deepEqual(calls.sort(), ["/helper/environment", "/runner/status"].sort());
  assert.doesNotMatch(await f.page.locator("main").innerText(), /runtimeText\.[A-Za-z]/);
}));

test("teaching summaries and automatic feedback translate while teacher comments and submitted code stay exact", { timeout: 60000 }, () => run(async f => {
  const calls = [], comment = "Missing GPL license declaration", source = "// Private submission\nint main(void) {}";
  await readRoute(f, "/learning/labs", [{ lab: { id: "01-first-program", position: 1,
    title: "Understand the eBPF execution pipeline", doc_slug: "labs/01-first-program",
    summary: "Compile and load XDP Pass, then inspect its lifecycle." },
    status: "in_progress", attempts: 1, latest_stage: "compile",
    latest_feedback: ["Run did not complete successfully (stage: compile)."] }], calls);
  await readRoute(f, "/learning/attempts", [{ id: "00000000-0000-4000-8000-000000000001",
    lab_id: "01-first-program", username: "fixture", template_id: "xdp-pass", source,
    created_at: "2026-10-13T00:00:00Z", run_success: false, stage: "compile", completed: false,
    attach_expected: false, attach_verified: false, feedback: ["The expected kernel attachment was not verified."],
    teacher_feedback: { reviewer: "fixture-teacher", updated_at: "2026-10-13T00:00:00Z", revision: 1, comment } }], calls);
  await f.page.goto(`${baseUrl}/learn`);
  for (const [locale, summary, feedback] of [
    ["zh-CN", "编译并加载 XDP Pass，再检查其生命周期。", "未验证到预期的内核挂载。"],
    ["en", "Compile and load XDP Pass, then inspect its lifecycle.", "The expected kernel attachment was not verified."],
    ["es", "Compila y carga XDP Pass e inspecciona su ciclo de vida.", "No se verificó el enlace al kernel esperado."],
    ["ja", "XDP Pass をコンパイルしてロードし、そのライフサイクルを確認します。", "想定したカーネルへのアタッチが確認できませんでした。"],
  ]) {
    await f.page.locator(".sidebar-footer select").selectOption(locale);
    await f.page.getByText(summary, { exact: true }).waitFor();
    await f.page.getByText(feedback, { exact: true }).waitFor();
    assert.equal(await f.page.getByText(comment, { exact: true }).textContent(), comment,
      "a teacher's own words are not treated as an automatic diagnostic");
    assert.equal(await f.page.locator(".learning-history pre").textContent(), source);
  }
  assert.ok(calls.includes("/learning/labs") && calls.includes("/learning/attempts"));
  assert.doesNotMatch(await f.page.locator("main").innerText(), /runtimeText\.[A-Za-z]/);
}));
