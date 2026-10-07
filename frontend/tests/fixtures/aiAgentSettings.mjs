import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import AiAgentSettingsPanel from "../../src/features/aiAgents/AiAgentSettingsPanel.tsx";
import SettingsPage from "../../pages/settings.tsx";
import { I18nProvider, useI18n } from "../../src/i18n/context.tsx";
import { agentInventory, jobInventory } from "./runnerData.mjs";

function Page() {
  fixture.setLocale = useI18n().setLocale;
  return React.createElement(fixture.mode === "page" ? SettingsPage : AiAgentSettingsPanel, { engineUrl: fixture.engineUrl });
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
  // Deliberately ignore AbortSignal, including late rejection/headers/body ownership.
  if (path === "/settings/ai-agents") return new Promise((resolve, reject) => fixture.requests.push({ url, init, resolve, reject }));
  switch (path) {
    case "/auth/me": return Promise.resolve(response({ authenticated: true, username: "teacher", role: "teacher" }));
    case "/events/unread-count": return Promise.resolve(response({ unread: 0 }));
    case "/settings/performance": return Promise.resolve(response({}, 503));
    case "/settings/events": return Promise.resolve(response({ max_records: 500, overflow_policy: "drop_oldest" }));
    case "/settings/compiler": return Promise.resolve(response({ resident: false, strategy: "on_demand" }));
    case "/runner/agents": return Promise.resolve(response(agentInventory([], { enabled: false })));
    case "/runner/jobs": return Promise.resolve(response(jobInventory([])));
    default: throw new Error("Unexpected fixture API " + path);
  }
};
