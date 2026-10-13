import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";

const sourceRoot = new URL("../src/", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, next) {
  return context.parentURL?.startsWith(sourceRoot) && specifier.startsWith(".")
    && !/\.[cm]?[jt]sx?$/.test(specifier)
    ? next(new URL(`${specifier}.ts`, context.parentURL).href, context) : next(specifier, context);
} });
let translations, registerBasicLanguageIntelligence, registerEbpfIntelligence;
let applyMarkers, localizeLegacyText, runtimeTextRows, cIntelligenceRows;
try {
  ({ translations } = await import("../src/i18n/translations.ts"));
  ({ registerBasicLanguageIntelligence } = await import("../src/features/editor/languageServices.ts"));
  ({ registerEbpfIntelligence } = await import("../src/utils/cEbpfIntelligence.ts"));
  ({ applyMarkers } = await import("../src/features/ebpf/editorUtils.ts"));
  ({ localizeLegacyText } = await import("../src/i18n/legacyText.ts"));
  ({ runtimeTextRows } = await import("../src/i18n/locales/runtimeText.ts"));
  ({ cIntelligenceRows } = await import("../src/i18n/locales/cIntelligence.ts"));
} finally { resolver.deregister(); }

function koreanValue(key) {
  let value = translations.ko;
  for (const part of key.split(".")) {
    assert.ok(value && Object.hasOwn(value, part), `merged Korean dictionary explicitly defines ${key}`);
    value = value[part];
  }
  assert.equal(typeof value, "string", key);
  assert.ok(value.trim(), key);
  return value;
}
const format = (template, variables = {}) => template.replace(/\{(\w+)\}/g, (_, name) => {
  assert.ok(Object.hasOwn(variables, name), `interpolation supplies ${name}`);
  return String(variables[name]);
});
const korean = (key, variables) => format(koreanValue(key), variables);
const localize = message => localizeLegacyText(message, korean);
const hangul = /[\uac00-\ud7af]/;
const snippetLabels = {
  rust: ["fn", "let", "match"], python: ["def", "class", "for"],
  c: ["main", "for", "include"], cpp: ["main", "class", "for"],
  markdown: ["heading", "link", "code"], yaml: ["mapping", "list"],
  sql: ["select", "create_table"], shell: ["if", "for", "function"],
};
function event() {
  const listeners = new Set();
  return {
    subscribe(callback) { listeners.add(callback); return { dispose: () => listeners.delete(callback) }; },
    fire(value) { for (const callback of [...listeners]) callback(value); },
  };
}
class Range {
  constructor(startLineNumber, startColumn, endLineNumber, endColumn) {
    Object.assign(this, { startLineNumber, startColumn, endLineNumber, endColumn });
  }
}
function fixture(language = "c") {
  const registrations = [], changed = event(), disposed = event(), content = event();
  const languageChanged = event(), removed = event(), cancellation = event();
  const model = {
    language, dead: false, source: "int original_source;", version: 7, word: "XDP_PASS",
    uri: { toString: () => "inmemory:///korean-owned.c" },
    isDisposed() { return this.dead; }, getLanguageId() { return this.language; },
    getWordUntilPosition: () => ({ word: "original_source", startColumn: 2, endColumn: 8 }),
    getWordAtPosition() { return { word: this.word, startColumn: 1, endColumn: this.word.length + 1 }; },
    getValue() { return this.source; }, getVersionId() { return this.version; },
    getLineContent(line) { return this.source.split("\n")[line - 1] || ""; },
    getLineCount() { return this.source.split("\n").length; },
    getPositionAt(offset) { return { lineNumber: 1, column: offset + 1 }; },
    onDidChangeLanguage: languageChanged.subscribe, onDidChangeContent: content.subscribe,
    onWillDispose: removed.subscribe,
  };
  const editor = {
    current: model, getModel() { return this.current; },
    onDidChangeModel: changed.subscribe, onDidDispose: disposed.subscribe,
  };
  const kinds = { Function: 1, Constant: 2, Snippet: 3, Struct: 4, Field: 5, Object: 6 };
  const monaco = {
    languages: {
      CompletionItemKind: kinds, CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
      CompletionTriggerKind: { Invoke: 0, TriggerCharacter: 1 }, SymbolKind: kinds,
    },
    MarkerSeverity: { Error: 8, Warning: 4, Info: 2 }, Range,
  };
  for (const kind of ["CompletionItem", "Hover", "SignatureHelp", "DocumentSymbol", "Definition", "CodeAction"]) {
    monaco.languages[`register${kind}Provider`] = (language, provider) => {
      const entry = { language, kind, provider, removed: 0 }; registrations.push(entry);
      return { dispose() { entry.removed++; } };
    };
  }
  const token = { isCancellationRequested: false, onCancellationRequested: cancellation.subscribe };
  return { monaco, model, editor, registrations, token,
    provider: (kind, language = "c") => registrations.find(entry => entry.kind === kind && entry.language === language).provider };
}
const position = { lineNumber: 1, column: 8 };
const request = (f, language, context = { triggerKind: 1, triggerCharacter: "_" }) =>
  f.provider("CompletionItem", language).provideCompletionItems(f.model, position, context, f.token);
const withoutDetail = items => items.map(({ detail, ...item }) => item);
const withoutDisplay = items => items.map(({ label, detail, ...item }) => item);
const rows = [...runtimeTextRows, ...cIntelligenceRows];
const keyFor = message => {
  const row = rows.find(row => row[1] === message);
  assert.ok(row, `known explanation exists: ${message}`);
  return `runtimeText.${row[0]}`;
};

test("merged Korean basic providers translate all 22 snippet details without changing code or identity", t => {
  const original = fixture("rust"), f = fixture("rust");
  const old = registerBasicLanguageIntelligence(original.monaco, original.editor);
  const current = registerBasicLanguageIntelligence(f.monaco, f.editor, korean);
  t.after(() => { old.dispose(); current.dispose(); });
  const entries = [...f.registrations], ownedModel = f.editor.current;
  assert.equal(entries.length, 8);
  let count = 0;
  for (const [language, labels] of Object.entries(snippetLabels)) {
    original.model.language = language; f.model.language = language;
    const english = request(original, language).suggestions;
    const localized = request(f, language).suggestions;
    assert.deepEqual(english.map(item => item.label), labels);
    assert.deepEqual(withoutDetail(localized), withoutDetail(english), language);
    assert.deepEqual(localized.map(item => item.detail), labels.map(label =>
      korean(`editorIntelligence.snippets.${language}.${label}`)));
    for (let index = 0; index < localized.length; index++) {
      assert.match(localized[index].detail, hangul);
      assert.notEqual(localized[index].detail, english[index].detail);
    }
    count += localized.length;
  }
  assert.equal(count, 22);
  assert.equal(f.editor.current, ownedModel);
  assert.equal(f.model.source, "int original_source;");
  assert.equal(f.model.version, 7); assert.equal(f.model.dead, false);
  assert.deepEqual(f.registrations, entries);
  assert.ok(entries.every(entry => entry.removed === 0));
});

test("Korean C hover and symbol explanations preserve all API titles and source identities", t => {
  const original = fixture(), f = fixture();
  const old = registerEbpfIntelligence(original.monaco, "https://engine.invalid", original.editor);
  const current = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", localize);
  t.after(() => { old.dispose(); current.dispose(); });
  const words = ["SEC", "xdp_md", "bpf_printk", "bpf_map_lookup_elem", "bpf_map_update_elem",
    "bpf_ringbuf_reserve", "bpf_ringbuf_submit", "bpf_ktime_get_ns", "bpf_get_smp_processor_id",
    "bpf_get_current_pid_tgid", "bpf_get_current_comm", "bpf_probe_read_kernel", "BPF_MAP_TYPE_HASH",
    "BPF_MAP_TYPE_RINGBUF", "trace_event_raw_sched_switch", "next_pid", "XDP_PASS", "XDP_DROP", "XDP_TX", "XDP_REDIRECT"];
  assert.equal(words.length, 20);
  const entries = [...f.registrations];
  for (const word of words) {
    original.model.word = word; f.model.word = word;
    const before = original.provider("Hover").provideHover(original.model, position);
    const after = f.provider("Hover").provideHover(f.model, position);
    assert.deepEqual(after.range, before.range, word);
    assert.deepEqual(after.contents[0], before.contents[0], "API titles remain literal");
    assert.equal(after.contents[1].value, korean(keyFor(before.contents[1].value)));
    assert.match(after.contents[1].value, hangul); assert.notEqual(after.contents[1].value, before.contents[1].value);
  }
  const source = 'SEC("xdp")\nint local_hook(struct xdp_md *ctx) { return XDP_PASS; }\nstruct {} local_map SEC(".maps");';
  original.model.source = source; f.model.source = source;
  const before = original.provider("DocumentSymbol").provideDocumentSymbols(original.model);
  const after = f.provider("DocumentSymbol").provideDocumentSymbols(f.model);
  assert.deepEqual(after.map(item => item.name), ["local_hook", "local_map"]);
  assert.deepEqual(withoutDetail(after), withoutDetail(before));
  assert.deepEqual(after.map(item => item.detail), [korean("runtimeText.cSymbolHook", { section: "xdp" }), korean("runtimeText.cSymbolMap")]);
  assert.equal(f.editor.current, f.model); assert.equal(f.model.source, source); assert.equal(f.model.version, 7);
  assert.deepEqual(f.registrations, entries); assert.ok(entries.every(entry => entry.removed === 0));
});

test("Korean C suggestions retain insertion contracts while raw semantic clang text remains untouched", async t => {
  t.mock.method(globalThis, "fetch", () => { throw new Error("static suggestions must not request a backend"); });
  const original = fixture(), f = fixture();
  const old = registerEbpfIntelligence(original.monaco, "https://engine.invalid", original.editor);
  const current = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", localize);
  t.after(() => { old.dispose(); current.dispose(); });
  const english = (await request(original, "c")).suggestions;
  const localized = (await request(f, "c")).suggestions;
  assert.equal(english.length, 20); assert.equal(localized.length, 20);
  assert.deepEqual(withoutDisplay(localized), withoutDisplay(english));
  const descriptive = new Set(["hash map declaration", "ring buffer declaration", "per-cpu array declaration", "GPL license"]);
  for (let index = 0; index < english.length; index++) {
    assert.equal(localized[index].label, descriptive.has(english[index].label)
      ? korean(keyFor(english[index].label)) : english[index].label);
    assert.equal(localized[index].detail, korean(keyFor(english[index].detail)));
    assert.match(localized[index].detail, hangul); assert.notEqual(localized[index].detail, english[index].detail);
  }
  const raw = "Raw clang detail · 원문 그대로: Missing GPL license declaration";
  t.mock.method(globalThis, "fetch", async () => Response.json({ ok: true,
    items: [{ label: "actual_symbol", insert_text: "actual_symbol", detail: raw, kind: "field" }] }));
  const semantic = (await request(f, "c", { triggerKind: 0 })).suggestions.find(item => item.label === "actual_symbol");
  assert.ok(semantic);
  assert.equal(semantic.detail, `clang · ${raw}`);
  assert.equal(semantic.insertText, "actual_symbol"); assert.equal(semantic.sortText, "0-actual_symbol");
  assert.equal(f.model.source, "int original_source;"); assert.equal(f.model.version, 7);
});

test("actual Korean marker projection and quick fixes use stable diagnostic codes rather than translated prose", t => {
  const f = fixture(), captures = [];
  f.monaco.editor = { setModelMarkers: (model, namespace, markers) => captures.push({ model, namespace, markers }) };
  const handle = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", localize);
  t.after(() => handle.dispose());
  const messages = ["Missing GPL license declaration", "Missing #include <bpf/bpf_helpers.h>",
    "Missing #include <linux/bpf.h> (or use #include <vmlinux.h> for CO-RE)", "교사의 원문 의견: keep this source"];
  const diagnostics = messages.map((message, index) => ({ message, line: index + 1, column: 2, endColumn: 7, severity: "warning" }));
  const unchanged = JSON.stringify(diagnostics);
  applyMarkers(f.editor, f.monaco, diagnostics, korean);
  const { model, namespace, markers } = captures.at(-1);
  assert.equal(model, f.model); assert.equal(namespace, "cyanrex-c-analyzer");
  assert.deepEqual(markers.map(marker => marker.code), ["cyanrex.license.gpl", "cyanrex.include.bpf/bpf_helpers.h", "cyanrex.include.linux/bpf.h", undefined]);
  assert.deepEqual(markers.map(marker => marker.message), [korean("runtimeText.missingGpl"), korean("runtimeText.missingHelpers"), korean("runtimeText.missingLinux"), messages[3]]);
  assert.deepEqual(markers.map(marker => [marker.startLineNumber, marker.startColumn, marker.endLineNumber, marker.endColumn, marker.severity]),
    messages.map((_, index) => [index + 1, 2, index + 1, 7, 4]));
  const actions = marker => f.provider("CodeAction").provideCodeActions(f.model, null, { markers: [marker] }).actions;
  const expected = [
    [korean("runtimeText.addGpl"), '\nchar _license[] SEC("license") = "GPL";\n'],
    [korean("runtimeText.addInclude", { header: "bpf/bpf_helpers.h" }), "#include <bpf/bpf_helpers.h>\n"],
    [korean("runtimeText.addInclude", { header: "linux/bpf.h" }), "#include <linux/bpf.h>\n"],
  ];
  for (let index = 0; index < expected.length; index++) {
    const result = actions(markers[index]); assert.equal(result.length, 1);
    assert.equal(result[0].title, expected[index][0]); assert.match(result[0].title, hangul);
    assert.equal(result[0].kind, "quickfix"); assert.equal(result[0].isPreferred, true);
    assert.equal(result[0].diagnostics[0], markers[index]);
    const edit = result[0].edit.edits[0];
    assert.equal(edit.textEdit.text, expected[index][1]); assert.equal(edit.versionId, 7); assert.equal(edit.resource, f.model.uri);
    assert.equal(edit.textEdit.range.startLineNumber, index === 0 ? 2 : 1);
  }
  const collision = actions({ ...markers[0], message: "Missing #include <wrong.h>" });
  assert.equal(collision[0].edit.edits[0].textEdit.text, expected[0][1], "a conflicting English message cannot redirect a stable code");
  const linked = actions({ ...markers[0], code: { value: "cyanrex.license.gpl", target: f.model.uri } });
  assert.equal(linked[0].edit.edits[0].textEdit.text, expected[0][1]);
  assert.equal(actions({ ...markers[0], code: undefined }).length, 0, "Korean prose cannot select a repair by itself");
  assert.equal(actions(markers[3]).length, 0);
  assert.equal(JSON.stringify(diagnostics), unchanged); assert.equal(f.model.source, "int original_source;");
});

test("every known legacy template uses the actual merged Korean adapter without changing literal parameters", () => {
  assert.equal(new Set(rows.map(row => row[0])).size, rows.length);
  assert.equal(new Set(rows.map(row => row[1])).size, rows.length);
  assert.equal(cIntelligenceRows.length, 46);
  assert.deepEqual(rows.map(row => row[0]).sort(), Object.keys(translations.ko.runtimeText).sort());
  assert.deepEqual(Object.keys(translations.ko.runtimeText).sort(), Object.keys(translations.en.runtimeText).sort());
  for (const [key, english] of rows) {
    const names = [...english.matchAll(/\{(\w+)\}/g)].map(match => match[1]);
    const variables = Object.fromEntries(names.map(name => [name,
      name === "path" ? "/sys/fs/bpf" : `exact_☕_${name}_<literal>{원문}`]));
    const calls = [];
    const actual = localizeLegacyText(format(english, variables), (key, captured) => {
      calls.push({ key, captured }); return korean(key, captured);
    });
    assert.equal(actual, format(koreanValue(`runtimeText.${key}`), variables), key);
    assert.match(actual, hangul, key);
    assert.notEqual(actual, format(english, variables), key);
    assert.deepEqual(calls, [{ key: `runtimeText.${key}`, captured: variables }]);
    for (const value of Object.values(variables)) assert.ok(actual.includes(value), `${key} retains ${value}`);
  }
  for (const path of ["/sys/fs/bpf", "/sys/kernel/btf/vmlinux"]) {
    assert.equal(localize(`${path} exists`), korean("runtimeText.pathExists", { path }));
    assert.equal(localize(`${path} missing`), korean("runtimeText.pathMissing", { path }));
  }
});

test("Korean legacy localization leaves unknown source teacher comments and non-exact templates unchanged", () => {
  const source = 'int main(void) {\n  // Missing GPL license declaration\n  return 0;\n}';
  const comment = "교사 의견: Missing GPL license declaration — 직접 검토하세요.";
  const originals = [source, comment, "Custom compiler failure", "Missing GPL license declaration\n",
    "prefix: Missing GPL license declaration", "// Missing GPL license declaration", "someone exists",
    "/private/student.c missing", "<script>alert(1)</script>", "x".repeat(4097), "",
    `Ringbuf reservation '${"x".repeat(513)}' must be checked for NULL before use`,
    "Run did not complete successfully (stage: compile\nload)."];
  for (const message of originals) {
    const result = localizeLegacyText(message, () => { throw new Error("unknown text must not request a display translation"); });
    assert.equal(result, message);
  }
});
