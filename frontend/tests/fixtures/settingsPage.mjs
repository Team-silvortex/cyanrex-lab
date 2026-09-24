import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import SettingsPage from "../../pages/settings.tsx";
import { I18nProvider, useI18n } from "../../src/i18n/context.tsx";
import { agentInventory, jobInventory } from "./runnerData.mjs";

function Page() {
  fixture.setLocale = useI18n().setLocale;
  return React.createElement(SettingsPage);
}

const root = createRoot(document.querySelector("#root"));
Object.defineProperty(window, "localStorage", { value: { getItem: () => "en", setItem() {} } });
const fixture = window.fixture = {
  requests: [], saved: {}, honorAbort: false, unreadChanges: 0,
  router: { pathname: "/settings", asPath: "/settings", replace() {}, push() {} },
  render(options = {}) {
    if (options.navigation) this.router = { ...this.router, asPath: options.navigation };
    flushSync(() => root.render(options === false ? null : React.createElement(React.StrictMode, null,
      React.createElement(I18nProvider, null, React.createElement(Page)))));
  },
  respond(index, data, status = 200) {
    // Controlled headers/body promises keep UI assertions independent of Web Streams scheduling.
    this.requests[index].resolve({ ok: status >= 200 && status < 300, status,
      headers: new Headers({ "content-type": "application/json" }),
      json: async () => structuredClone(data), body: { cancel: async () => undefined } });
  },
};
window.addEventListener("cyanrex-events-unread-changed", () => fixture.unreadChanges++);
window.fetch = (url, init = {}) => {
  const path = new URL(url).pathname;
  if (path === "/auth/me") return Promise.resolve(Response.json({ authenticated: true, username: "teacher", role: "teacher" }));
  if (path === "/events/unread-count") return Promise.resolve(Response.json({ unread: 0 }));
  if (path === "/settings/performance") return Promise.resolve(Response.json({}, { status: 503 }));
  if (path === "/runner/agents") return Promise.resolve(Response.json(agentInventory([], { enabled: false })));
  if (path === "/runner/jobs") return Promise.resolve(Response.json(jobInventory([])));
  if (!["/settings/events", "/settings/compiler"].includes(path)) throw new Error("Unexpected fixture API " + path);
  return new Promise((resolve, reject) => {
    fixture.requests.push({ url, init, resolve, reject });
    const abort = () => { if (fixture.honorAbort) reject(init.signal.reason); };
    if (init.signal?.aborted) abort(); else init.signal?.addEventListener("abort", abort, { once: true });
  });
};
