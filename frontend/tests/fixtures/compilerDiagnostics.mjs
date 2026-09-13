import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { useCompilerDiagnostics } from "../../src/features/ebpf/useCompilerDiagnostics.ts";

// Real React lifecycle; synthetic fetch completions and a controllable browser clock.
// Ignoring abort simulates an application continuation that already has a response.
const root = createRoot(document.querySelector("#root"));
window.fixture = {
  requests: [], honorAbort: false,
  render(editors) {
    flushSync(() => root.render(React.createElement(React.StrictMode, null,
      editors.map((props, index) => React.createElement(Editor, { key: props.id || index, ...props })))));
  },
  respond(index, body, status = 200) {
    this.requests[index].resolve(Response.json(body, { status }));
  },
};

window.fetch = (url, init = {}) => new Promise((resolve, reject) => {
  window.fixture.requests.push({ url, init, resolve, reject });
  if (url.endsWith("/cancel")) { resolve(Response.json({ ok: true })); return; }
  const abort = () => { if (window.fixture.honorAbort) reject(init.signal.reason); };
  if (init.signal?.aborted) abort();
  else init.signal?.addEventListener("abort", abort, { once: true });
});

// Native AbortSignal.timeout uses a separate browser clock. Route it through the
// installed clock so 35-second deadlines can be tested without wall-clock sleeps.
AbortSignal.timeout = ms => {
  const controller = new AbortController();
  setTimeout(() => controller.abort(new DOMException("Timed out", "TimeoutError")), ms);
  return controller.signal;
};

function Editor({ id = "editor", code = "int draft;", engineUrl = "https://engine-a.invalid", headerContextKey = "", target = "local" }) {
  const result = useCompilerDiagnostics(code, engineUrl, headerContextKey, target);
  return React.createElement("pre", { "data-editor": id }, JSON.stringify(result));
}
