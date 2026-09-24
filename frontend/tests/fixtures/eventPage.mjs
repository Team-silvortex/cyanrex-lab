import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import EventsPage from "../../pages/events.tsx";
import { I18nProvider } from "../../src/i18n/context.tsx";

const root = createRoot(document.querySelector("#root"));
Object.defineProperty(window, "localStorage", { value: { getItem: () => "en", setItem() {} } });
const fixture = window.fixture = {
  requests: [], saved: {}, sockets: [], downloads: [], blobs: [], revoked: [], renders: [],
  holdUnread: false, unread: 3, honorAbort: false,
  router: { pathname: "/events", asPath: "/events", replace() {}, push() {} },
  render(options = {}) {
    if (options.navigation) this.router = { ...this.router, asPath: options.navigation };
    flushSync(() => root.render(options === false ? null : React.createElement(React.StrictMode, null,
      React.createElement(I18nProvider, null, React.createElement(React.Profiler, { id: "page", onRender() {
        fixture.renders.push({ category: document.querySelector("main.content select")?.value,
          rows: [...document.querySelectorAll("article strong")].map(element => element.textContent) });
      } }, React.createElement(EventsPage))))));
  },
  respond(index, data, status = 200) { this.requests[index].resolve(Response.json(data, { status })); },
  response(index, body, headers, status = 200) { this.requests[index].resolve(new Response(body, { headers, status })); },
  live(row) { const socket = this.sockets.filter(socket => !socket.closed).at(-1); socket.onmessage({ data: JSON.stringify(row) }); },
};
window.WebSocket = class {
  closed = false;
  constructor(url) { this.url = url; fixture.sockets.push(this); setTimeout(() => { if (!this.closed) this.onopen?.({}); }, 0); }
  close() { this.closed = true; this.onclose?.({ code: 1000 }); }
};
window.fetch = (url, init = {}) => {
  const path = new URL(url).pathname;
  if (path === "/auth/me") return Promise.resolve(Response.json({ authenticated: true, username: "student", role: "student" }));
  if (path === "/events/unread-count" && !fixture.holdUnread) return Promise.resolve(Response.json({ unread: fixture.unread }));
  if (!["/events", "/events/unread-count", "/events/mark-read", "/events/export", "/events/delete"].includes(path)) throw new Error("Unexpected fixture API " + path);
  return new Promise((resolve, reject) => {
    fixture.requests.push({ url, init, resolve, reject });
    const abort = () => { if (fixture.honorAbort) reject(init.signal.reason); };
    if (init.signal?.aborted) abort(); else init.signal?.addEventListener("abort", abort, { once: true });
  });
};
URL.createObjectURL = blob => { fixture.blobs.push(blob); return "blob:fixture-" + fixture.blobs.length; };
URL.revokeObjectURL = url => fixture.revoked.push(url);
HTMLAnchorElement.prototype.click = function () { fixture.downloads.push({ filename: this.download, url: this.href }); };
