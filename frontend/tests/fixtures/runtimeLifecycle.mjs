import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { useEbpfPageController } from "../../src/features/ebpf/useEbpfPageController.ts";

const root = createRoot(document.querySelector("#root"));
const fixture = window.fixture = {
  options: { lab: "01-fixture", navigation: "/ebpf?lab=01-fixture" }, controller: null,
  requests: [], calls: [], outcomes: {}, tokens: {}, honorAbort: false,
  holdAttachments: false, attachments: [],
  render(options = {}) {
    if (options && !Array.isArray(options)) Object.assign(this.options, options);
    flushSync(() => root.render(options === false ? null
      : React.createElement(React.StrictMode, null, React.createElement(Controller, this.options))));
  },
  respond(index, data, status = 200) { this.requests[index].resolve(Response.json(data, { status })); },
  changeCode(code) { flushSync(() => this.controller.setCode(code)); },
  invoke(name, id = name, argument) {
    const token = this.tokens[id] ||= new AbortController();
    const args = name === "detach" ? [argument, token.signal] : [token.signal];
    Promise.resolve(this.controller[name](...args)).then(
      () => { this.outcomes[id] = { resolved: true }; },
      error => { this.outcomes[id] = { rejected: true, message: String(error?.message || error) }; },
    );
  },
};
window.fetch = (url, init = {}) => {
  fixture.calls.push({ url, init });
  const path = new URL(url).pathname;
  if (path === "/modules/c-headers/selected-metadata") return Promise.resolve(Response.json({ selected_headers: [] }));
  if (path === "/ebpf/check/backends") return Promise.resolve(Response.json({ local_available: true, agents: [] }));
  if (path === "/ebpf/check") return Promise.resolve(Response.json({ ok: true, message: "checked", diagnostics: [], stdout: "", stderr: "" }));
  if (["/scripts", "/ebpf/templates", "/learning/labs"].includes(path)) return Promise.resolve(Response.json([]));
  if (path === "/ebpf/attachments/details" && !fixture.holdAttachments) {
    return Promise.resolve(Response.json({ attachments: fixture.attachments }));
  }
  if (!["/ebpf/run", "/ebpf/detach", "/ebpf/attachments/details"].includes(path)) throw new Error("Unexpected fixture API " + path);
  return new Promise((resolve, reject) => {
    fixture.requests.push({ url, init, resolve, reject });
    const abort = () => { if (fixture.honorAbort) reject(init.signal.reason); };
    if (init.signal?.aborted) abort(); else init.signal?.addEventListener("abort", abort, { once: true });
  });
};
const translate = (key, vars) => key + (vars ? " " + JSON.stringify(vars) : "");
function Controller({ lab, navigation, blocked = false }) {
  const value = useEbpfPageController(translate, lab, blocked, navigation); fixture.controller = value;
  return React.createElement("pre", { "data-editor": "editor" }, JSON.stringify({
    result: value.result, error: value.error, running: value.running, detaching: value.detaching || false,
    attachments: value.attachments, attachmentState: value.attachmentState || "",
    runtimeNotice: value.runtimeNotice || "", code: value.code,
  }));
}
