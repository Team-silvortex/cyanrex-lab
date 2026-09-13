import React from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { registerEbpfIntelligence } from "../../src/utils/cEbpfIntelligence.ts";
import { useEbpfPageController } from "../../src/features/ebpf/useEbpfPageController.ts";

function event() {
  const listeners = new Set();
  return { subscribe: callback => { listeners.add(callback); return { dispose: () => listeners.delete(callback) }; },
    fire: () => { for (const callback of listeners) callback(); } };
}
const registrations = [], markerCalls = [];
const kinds = { Function: 1, Constant: 2, Snippet: 3, Struct: 4, Field: 5 };
const monaco = {
  languages: { CompletionItemKind: kinds, CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
    CompletionTriggerKind: { Invoke: 0, TriggerCharacter: 1 }, SymbolKind: kinds },
  editor: { defineTheme() {}, setTheme() {}, getModels: () => [], MouseTargetType: { GUTTER_GLYPH_MARGIN: 1 },
    setModelMarkers: (model, owner, markers) => markerCalls.push({ id: model.id, owner, markers }) },
  MarkerSeverity: { Error: 8, Warning: 4, Info: 2 }, KeyCode: { F9: 1 },
  Range: class { constructor(startLineNumber, startColumn, endLineNumber, endColumn) { Object.assign(this, { startLineNumber, startColumn, endLineNumber, endColumn }); } },
};
for (const kind of ["CompletionItem", "Hover", "SignatureHelp", "DocumentSymbol", "Definition", "CodeAction"]) {
  monaco.languages[`register${kind}Provider`] = (_language, provider) => {
    const registration = { kind, provider, disposed: false }; registrations.push(registration);
    return { dispose: () => { registration.disposed = true; } };
  };
}
function model(id, source = "int fixture;") {
  const changed = event(), disposed = event();
  return { id, source, version: 1, dead: false, uri: { toString: () => `inmemory:///${id}` },
    getValue() { return this.source; }, getVersionId() { return this.version; }, isDisposed() { return this.dead; },
    getLineContent(line) { return this.source.split("\n")[line - 1] || ""; }, getLineCount() { return this.source.split("\n").length; },
    getWordUntilPosition: position => ({ startColumn: 1, endColumn: position.column, word: "fixture" }),
    getWordAtPosition: () => ({ startColumn: 1, endColumn: 11, word: "bpf_printk" }),
    getPositionAt: offset => ({ lineNumber: 1, column: offset + 1 }),
    onDidChangeContent: changed.subscribe, onWillDispose: disposed.subscribe,
    change(source) { this.source = source; this.version++; changed.fire(); },
    dispose() { disposed.fire(); this.dead = true; },
  };
}
function editor(current) {
  const changed = event(), disposed = event();
  return { current, getModel() { return this.current; },
    setModel(next) { this.current = next; changed.fire(); },
    onDidChangeModel: changed.subscribe, onDidDispose: disposed.subscribe,
    dispose() { disposed.fire(); }, updateOptions() {}, deltaDecorations: () => [],
    onMouseDown: () => ({ dispose() {} }), onKeyDown: () => ({ dispose() {} }),
  };
}
function token() {
  const cancelled = event();
  return { isCancellationRequested: false, onCancellationRequested: cancelled.subscribe,
    cancel() { this.isCancellationRequested = true; cancelled.fire(); } };
}
const root = createRoot(document.querySelector("#root"));
const fixture = window.fixture = {
  registrations, markerCalls, monaco, models: {}, editors: {}, handles: {}, providers: {}, tokens: {}, results: {},
  requests: [], honorAbort: false, holdMetadata: false, metadata: [], controller: null,
  render(show = true) { flushSync(() => root.render(show ? React.createElement(React.StrictMode, null, React.createElement(Controller)) : null)); },
  respond(index, data, status = 200) { this.requests[index].resolve(Response.json(data, { status })); },
  makeModel(id, source) { return this.models[id] = model(id, source); },
  register(id, headers = "") {
    const ownModel = this.models[id] || this.makeModel(id);
    const ownEditor = this.editors[id] || (this.editors[id] = editor(ownModel));
    this.handles[id] = registerEbpfIntelligence(monaco, "https://engine-a.invalid", ownEditor, headers);
    this.providers[id] = registrations.findLast(item => item.kind === "CompletionItem").provider;
  },
  complete(id, requestId = id, modelId = id, cancelled = false, triggerKind = 0) {
    const cancellation = this.tokens[requestId] = token();
    if (cancelled) cancellation.cancel();
    void this.providers[id].provideCompletionItems(this.models[modelId], { lineNumber: 1, column: 5 },
      { triggerKind, triggerCharacter: "b" }, cancellation).then(result => { this.results[requestId] = result; });
  },
  changeCode(code) { flushSync(() => this.controller.setCode(code)); },
  mountEditor() {
    this.makeModel("foreign", "int foreign;"); const own = this.makeModel("owned", this.controller.code);
    monaco.editor.getModels = () => [this.models.foreign, own];
    this.editors.owned = editor(own);
    flushSync(() => this.controller.onEditorMount(this.editors.owned, monaco));
  },
};

window.fetch = (url, init = {}) => {
  if (url.endsWith("/selected-metadata") && !fixture.holdMetadata) return Promise.resolve(Response.json({ selected_headers: fixture.metadata }));
  if (url.endsWith("/backends")) return Promise.resolve(Response.json({ local_available: true, agents: [] }));
  if (url.endsWith("/attachments/details")) return Promise.resolve(Response.json({ attachments: [] }));
  if (["/scripts", "/ebpf/templates", "/learning/labs"].some(path => url.endsWith(path))) return Promise.resolve(Response.json([]));
  return new Promise((resolve, reject) => {
    fixture.requests.push({ url, init, resolve, reject });
    const abort = () => { if (fixture.honorAbort) reject(init.signal.reason); };
    if (init.signal?.aborted) abort();
    else init.signal?.addEventListener("abort", abort, { once: true });
  });
};
const translate = key => key;
function Controller() {
  const value = useEbpfPageController(translate); fixture.controller = value;
  return React.createElement("pre", { "data-editor": "editor" }, JSON.stringify({
    check: value.headerInjectionCheck, metadata: value.injectedMetadata,
    metadataError: value.injectedMetadataError || "", metadataLoading: value.injectedMetadataLoading || false,
  }));
}
