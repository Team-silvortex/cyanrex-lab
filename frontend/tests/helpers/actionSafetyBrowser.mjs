export const baseUrl = process.env.CYANREX_UI_BASE_URL || "http://localhost:3217";
export const engineUrl = process.env.CYANREX_UI_ENGINE_URL || "http://localhost:8080";
export const draft = "// Unsaved safety draft\nint keep_draft(void) { return 1; }";
export const target = "/sys/fs/bpf/safety-fixture/program-a";

export async function setup(role = "admin") {
  const { chromium } = await import(process.env.CYANREX_PLAYWRIGHT_MODULE || "playwright");
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH });
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  await context.addInitScript(({ draft, engineUrl }) => {
    localStorage.setItem("cyanrex_locale", "zh-CN");
    sessionStorage.setItem("ebpf_code_v1", JSON.stringify(draft));
    window.fixtureSockets = [];
    const EngineSocket = class {
      closed = false;
      constructor(url) { this.url = url; window.fixtureSockets.push(this); setTimeout(() => { if (!this.closed) this.onopen?.({}); }, 0); }
      close() { this.closed = true; this.onclose?.({ code: 1000 }); }
    };
    window.WebSocket = new Proxy(window.WebSocket, { construct(Socket, args) {
      return new URL(args[0], location.href).host === new URL(engineUrl).host
        ? new EngineSocket(args[0]) : Reflect.construct(Socket, args);
    } });
  }, { draft, engineUrl });
  const page = await context.newPage(), writes = [], errors = [];
  const state = { fail: false, hold: null, headers: ["header-a", "header-b"].map(id => ({ id, name: id, description: "Fixture header", source_url: "fixture", downloaded: true, selected: true, local_path: `/fixture/${id}` })) };
  page.on("pageerror", error => errors.push(error.message));
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === new URL(baseUrl).origin) return route.continue();
    errors.push("Blocked an unmocked external request");
    return route.abort();
  });
  await context.route(`${engineUrl}/**`, async route => {
    const request = route.request(), url = new URL(request.url());
    const headers = { "access-control-allow-origin": new URL(baseUrl).origin, "access-control-allow-credentials": "true",
      "access-control-allow-methods": "GET, POST, OPTIONS", "access-control-allow-headers": "content-type" };
    const reply = (json, status = 200) => route.fulfill({ json, status, headers });
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers });
    if (request.method() === "POST" && !["/events/mark-read", "/ebpf/check", "/ebpf/complete"].includes(url.pathname)) {
      writes.push({ path: url.pathname, params: Object.fromEntries(url.searchParams), body: request.postData() ? request.postDataJSON() : null });
      if (state.hold) await state.hold();
      if (state.fail || writes.length === state.failAt) return reply({ ok: false, message: "fixture unavailable" }, 503);
    }
    switch (url.pathname) {
      case "/auth/me": return reply({ authenticated: true, username: role, role });
      case "/events/unread-count": return reply(state.unreadResult ?? { unread: 0 }, state.unreadStatus || 200);
      case "/events/mark-read": return reply(state.markReadResult ?? { ok: true }, state.markReadStatus || 200);
      case "/events/export":
        (state.exports ||= []).push(url.search);
        return route.fulfill({ body: JSON.stringify(state.events ?? []), headers: { ...headers,
          "content-type": "application/json", "content-disposition": 'attachment; filename="cyanrex-events-fixture.json"',
          "access-control-expose-headers": "content-disposition" } });
      case "/events":
        (state.eventReads ||= []).push(url.search);
        return reply(state.events ?? [{ username: role, timestamp: "2026-09-08T11:00:00Z", source: "fixture", event_type: "fixture.event", category: "kernel", severity: "error", color: "red", payload: {} }], state.eventsStatus || 200);
      case "/events/delete": return reply({ ok: true, deleted: 500 });
      case "/learning/labs": return reply(state.labs ?? []);
      case "/ebpf/templates": return reply([{ id: "template-a", name: "Template A", capability: "xdp", code: "// Template source" }]);
      case "/ebpf/attachments/details": return reply({ attachments: state.attachments ?? [{ pin_path: target, source: draft, program_name: "program-a" }] }, state.attachmentStatus || 200);
      case "/ebpf/detach": return reply(state.detachResult ?? { ok: true, message: "detached", detached: [target], clean: true }, state.detachStatus || 200);
      case "/modules/c-headers/selected-metadata": return reply({ selected_headers: state.selectedMetadata || [] }, state.metadataStatus || 200);
      case "/modules/c-headers/catalog": return reply({ headers: state.headers });
      case "/modules/c-headers/delete": return reply({ ok: true, message: "removed" });
      case "/modules/c-headers/select": return reply({ ok: true, message: "selected" });
      case "/ebpf/check/backends": return reply({ local_available: true, agents: [{ agent_id: "compiler-agent-a", isolation: "container", available_slots: 1, max_concurrent: 1 }] });
      case "/ebpf/check":
        (state.checks ||= []).push(request.postDataJSON());
        return reply({ ok: true, diagnostics: [], message: "checked", stdout: "", stderr: "" });
      case "/ebpf/check/remote": return reply({ job_id: "remote-check-a", state: state.remoteState || "succeeded", result: { ok: !state.remoteState, diagnostics: [], message: "remote checked", stdout: "", stderr: "" } });
      case "/ebpf/check/remote/cancel": return reply({ ok: true });
      case "/ebpf/complete":
        (state.completions ||= []).push(request.postDataJSON());
        return reply({ ok: true, items: state.completionItems || [], message: "" });
      case "/ebpf/run": return reply(state.runResult ?? { success: false, stage: "compile", message: "fixture compilation", compile_stdout: "", compile_stderr: "", load_stdout: "", load_stderr: "" }, state.runStatus || 200);
      case "/scripts": return reply([{ id: "script-a", title: "My saved script", script: "// Saved source", updated_at: "2026-09-08T12:00:00Z" }]);
      case "/scripts/delete": return reply({ ok: true, message: "deleted" });
      case "/modules": return reply([{ name: "module-a", status: "running", version: "0.3.5" }]);
      case "/command": return reply({ ok: true, commandType: request.postDataJSON().commandType, message: "done", module: { name: "module-a", status: "stopped" } });
      case "/auth/delete": return reply({ ok: true });
      case "/settings/performance": return reply({}, 503);
      case "/settings/events": return reply(request.method() === "POST" ? { ok: true, settings: request.postDataJSON() } : { max_records: 500, overflow_policy: "drop_oldest" });
      case "/settings/compiler": return reply(request.method() === "POST" ? { ok: true, settings: request.postDataJSON() } : { resident: false, strategy: "on_demand" });
      case "/runner/agents": return reply({ enabled: true, total_agents: 0, online_agents: 0, agents: [] });
      case "/runner/jobs": return reply({ total_jobs: 1, jobs: [{ job_id: "job-a-full-confirmation-id", kind: "compile_check", state: "queued", owner_username: "student", target_agent_id: "agent-a", created_at: "2026-09-08T12:00:00Z" }] });
      case "/runner/jobs/cancel": return reply({ ok: true });
      default: errors.push(`Unexpected API ${request.method()} ${url.pathname}`); return reply({}, 404);
    }
  });
  return { browser, page, writes, errors, state, code: () => page.evaluate(() => JSON.parse(sessionStorage.getItem("ebpf_code_v1"))) };
}
