import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import SettingsPage from "../../pages/settings.tsx";
import { I18nProvider, useI18n } from "../../src/i18n/context.tsx";
import { usePerformanceMetrics } from "../../src/features/settings/usePerformanceMetrics.ts";
import PerformanceMetricsPanel from "../../src/features/settings/PerformanceMetricsPanel.tsx";
import { agentInventory, jobInventory, runnerJob, stamp } from "./runnerData.mjs";

function HookView() {
  const metrics = usePerformanceMetrics(fixture.engineUrl, fixture.router.asPath);
  fixture.metrics = metrics;
  return React.createElement(React.Fragment, null,
    React.createElement("output", { "data-testid": "hook-state" }, JSON.stringify({
      metrics: metrics.metrics, refreshing: metrics.refreshing, error: metrics.error, message: metrics.message, stale: metrics.stale,
    })),
    React.createElement(PerformanceMetricsPanel, { metrics: metrics.metrics, summary: metrics.hotspotSummary,
      refreshing: metrics.refreshing, error: metrics.error, message: metrics.message, stale: metrics.stale }));
}
function Page() {
  fixture.setLocale = useI18n().setLocale;
  return React.createElement(fixture.mode === "hook" ? HookView : SettingsPage);
}
const response = (data, status = 200) => ({ ok: status >= 200 && status < 300, status,
  headers: new Headers({ "content-type": "application/json" }), json: async () => structuredClone(data),
  body: { cancel: async () => undefined } });
const root = createRoot(document.querySelector("#root"));
Object.defineProperty(window, "localStorage", { value: { getItem: () => "en", setItem() {} } });
const fixture = window.fixture = {
  requests: [], saved: {}, writes: [], mode: "page", holdEvents: false,
  engineUrl: "https://engine-a.invalid",
  router: { pathname: "/settings", asPath: "/settings", replace() {}, push() {} },
  render(options = {}) {
    if (options.navigation) this.router = { ...this.router, asPath: options.navigation };
    if (options.engineUrl) this.engineUrl = options.engineUrl;
    if (options.mode) this.mode = options.mode;
    if (options.holdEvents !== undefined) this.holdEvents = options.holdEvents;
    flushSync(() => root.render(options === false ? null : React.createElement(React.StrictMode, null,
      React.createElement(I18nProvider, null, React.createElement(Page)))));
  },
  respond(index, data, status = 200) { this.requests[index].resolve(response(data, status)); },
};
window.fetch = (url, init = {}) => {
  const path = new URL(url).pathname;
  if (path === "/settings/performance" || (path === "/settings/events" && fixture.holdEvents)) {
    return new Promise((resolve, reject) => fixture.requests.push({ url, init, resolve, reject }));
  }
  if (init.method === "POST") fixture.writes.push({ path, body: JSON.parse(init.body) });
  const body = init.body ? JSON.parse(init.body) : null;
  switch (path) {
    case "/auth/me": return Promise.resolve(response({ authenticated: true, username: "teacher", role: "teacher" }));
    case "/events/unread-count": return Promise.resolve(response({ unread: 0 }));
    case "/settings/events": return Promise.resolve(response(body ? { ok: true, settings: body } : { max_records: 500, overflow_policy: "drop_oldest" }));
    case "/settings/compiler": return Promise.resolve(response(body
      ? { ok: true, settings: { ...body, strategy: body.resident ? "resident_cache" : "on_demand" } }
      : { resident: false, strategy: "on_demand" }));
    case "/runner/agents": return Promise.resolve(response(agentInventory([])));
    case "/runner/jobs": return Promise.resolve(response(jobInventory()));
    case "/runner/jobs/cancel": return Promise.resolve(response(runnerJob({ state: "cancelled", completed_at: stamp })));
    default: throw new Error("Unexpected fixture API " + path);
  }
};
