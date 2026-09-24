import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import SettingsPage from "../../pages/settings.tsx";
import RunnerAgentAdminPanel from "../../src/features/runner/RunnerAgentAdminPanel.tsx";
import { I18nProvider, useI18n } from "../../src/i18n/context.tsx";

function Page() {
  fixture.setLocale = useI18n().setLocale;
  return React.createElement(fixture.mode === "page" ? SettingsPage : RunnerAgentAdminPanel, { engineUrl: fixture.engineUrl });
}
const response = (data, status = 200) => ({ ok: status >= 200 && status < 300, status,
  headers: new Headers({ "content-type": "application/json" }), json: async () => structuredClone(data),
  body: { cancel: async () => undefined } });
const root = createRoot(document.querySelector("#root"));
Object.defineProperty(window, "localStorage", { value: { getItem: () => "en", setItem() {} } });
const fixture = window.fixture = {
  requests: [], saved: {}, mode: "panel", engineUrl: "https://engine-a.invalid",
  router: { pathname: "/settings", asPath: "/settings", replace() {}, push() {} },
  render(options = {}) {
    if (options.navigation) this.router = { ...this.router, asPath: options.navigation };
    if (options.engineUrl) this.engineUrl = options.engineUrl;
    if (options.mode) this.mode = options.mode;
    flushSync(() => root.render(options === false ? null : React.createElement(React.StrictMode, null,
      React.createElement(I18nProvider, null, React.createElement(Page)))));
  },
  respond(index, data, status = 200) { this.requests[index].resolve(response(data, status)); },
};
window.fetch = (url, init = {}) => {
  const path = new URL(url).pathname;
  // Deliberately ignore abort: obsolete headers/body must not regain ownership.
  if (path.startsWith("/runner/")) return new Promise((resolve, reject) => fixture.requests.push({ url, init, resolve, reject }));
  const body = init.body ? JSON.parse(init.body) : null;
  switch (path) {
    case "/auth/me": return Promise.resolve(response({ authenticated: true, username: "teacher", role: "teacher" }));
    case "/events/unread-count": return Promise.resolve(response({ unread: 0 }));
    case "/settings/performance": return Promise.resolve(response({}, 503));
    case "/settings/events": return Promise.resolve(response(body ? { ok: true, settings: body } : { max_records: 500, overflow_policy: "drop_oldest" }));
    case "/settings/compiler": return Promise.resolve(response(body
      ? { ok: true, settings: { ...body, strategy: body.resident ? "resident_cache" : "on_demand" } }
      : { resident: false, strategy: "on_demand" }));
    default: throw new Error("Unexpected fixture API " + path);
  }
};
