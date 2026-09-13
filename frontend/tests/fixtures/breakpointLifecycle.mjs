import React, { useRef } from "react";
import { createRoot } from "react-dom/client";
import { flushSync } from "react-dom";
import { useBreakpointHitStream } from "../../src/features/ebpf/useBreakpointHitStream.ts";
import { useEbpfEditorBreakpoints } from "../../src/features/ebpf/useEditorBreakpoints.ts";

function listeners() {
  const active = new Set(), all = [];
  return { active, all, subscribe: callback => { active.add(callback); all.push(callback); return { dispose: () => active.delete(callback) }; },
    fire: value => { for (const callback of [...active]) callback(value); } };
}
function model(id, source) {
  const dispose = listeners(); let nextId = 0;
  return { id, source, dead: false, disposal: dispose, decorations: new Map(), getLineCount() { return this.source.split("\n").length; },
    getValue() { return this.source; }, isDisposed() { return this.dead; }, onWillDispose: dispose.subscribe,
    deltaDecorations(old, decorations) {
      old.forEach(id => this.decorations.delete(id));
      return decorations.map(item => { const key = `${id}-${++nextId}`; this.decorations.set(key, item); return key; });
    },
    dispose() { dispose.fire(); this.dead = true; this.decorations.clear(); },
  };
}
function editor(id, current) {
  const keys = listeners(), mouse = listeners(), changes = listeners(), dispose = listeners();
  return { id, current, keys, mouse, changes, disposal: dispose, line: 2, dead: false,
    getModel() { return this.current; }, getPosition() { return { lineNumber: this.line, column: 1 }; },
    updateOptions() {}, deltaDecorations(old, next) { return this.current.deltaDecorations(old, next); },
    onKeyDown: keys.subscribe, onMouseDown: mouse.subscribe, onDidChangeModel: changes.subscribe, onDidDispose: dispose.subscribe,
    setModel(next) { this.current = next; changes.fire(); },
    dispose() { this.dead = true; dispose.fire(); },
  };
}
const monaco = { KeyCode: { F9: 9 }, editor: { MouseTargetType: { GUTTER_GLYPH_MARGIN: 2 } },
  Range: class { constructor(startLineNumber, startColumn, endLineNumber, endColumn) { Object.assign(this, { startLineNumber, startColumn, endLineNumber, endColumn }); } } };
const root = createRoot(document.querySelector("#root"));
const fixture = window.fixture = {
  options: { mode: "stream", engine: "https://engine-a.invalid", session: "session-a", lines: [2, 3], code: "one\ntwo\nthree\nfour", hit: null },
  sockets: [], requests: [], history: [], editors: {}, models: {}, controller: null, honorAbort: false,
  render(options = {}) {
    if (options !== false) Object.assign(this.options, options);
    flushSync(() => root.render(options === false ? null : React.createElement(React.StrictMode, null,
      React.createElement(this.options.mode === "stream" ? Stream : Editor, this.options))));
  },
  respond(index, data, status = 200) { this.requests[index].resolve(Response.json(data, { status })); },
  open(index = this.sockets.length - 1) { this.sockets[index].onopen?.({}); },
  message(value, index = this.sockets.length - 1) { this.sockets[index].onmessage?.({ data: typeof value === "string" ? value : JSON.stringify(value) }); },
  disconnect(code = 1013, index = this.sockets.length - 1) { const socket = this.sockets[index]; socket.closed = true; socket.onclose?.({ code }); },
  attach(id) {
    const value = this.editors[id] = editor(id, this.models[id] = model(id, this.options.code));
    flushSync(() => { this.editorRef.current = value; this.monacoRef.current = monaco; this.controller.onEditorReadyForDebug(value, monaco); });
  },
  key(id, stale = false) {
    const value = { keyCode: monaco.KeyCode.F9, preventDefault() {} };
    flushSync(() => { if (stale) this.editors[id].keys.all[0](value); else this.editors[id].keys.fire(value); });
  },
  swapModel(id, source) {
    const next = this.models.replacement = model("replacement", source);
    flushSync(() => this.editors[id].setModel(next));
  },
  decorations(id) { return [...this.models[id].decorations.values()].map(item => ({ line: item.range.startLineNumber, className: item.options.className })); },
};
window.WebSocket = class {
  closed = false;
  constructor(url) { this.url = url; fixture.sockets.push(this); }
  close() { this.closed = true; this.onclose?.({ code: 1000 }); }
};
window.fetch = (url, init = {}) => new Promise((resolve, reject) => {
  fixture.requests.push({ url, init, resolve, reject });
  init.signal?.addEventListener("abort", () => { if (fixture.honorAbort) reject(init.signal.reason); }, { once: true });
});
function Stream({ engine, session, lines }) {
  const value = useBreakpointHitStream(engine, session, lines);
  fixture.history.push({ engine, session, ...value });
  return React.createElement("pre", { "data-editor": "editor" }, JSON.stringify(value));
}
function Editor({ code, hit }) {
  const editorRef = useRef(null), monacoRef = useRef(null);
  fixture.editorRef = editorRef; fixture.monacoRef = monacoRef;
  const value = useEbpfEditorBreakpoints({ code, hitLine: hit, editorRef, monacoRef }); fixture.controller = value;
  return React.createElement("pre", { "data-editor": "editor" }, JSON.stringify({ breakpoints: value.debugBreakpoints }));
}
