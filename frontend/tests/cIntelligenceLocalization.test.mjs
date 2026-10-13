import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import ts from "typescript";
import { registerHooks } from "node:module";
import { cIntelligenceRows } from "../src/i18n/locales/cIntelligence.ts";
const localeRoot = new URL("../src/i18n/locales/", import.meta.url).href;
const resolver = registerHooks({ resolve(s, c, next) {
  return c.parentURL?.startsWith(localeRoot) && s.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(s)
    ? next(new URL(`${s}.ts`, c.parentURL).href, c) : next(s, c);
} });
let runtimeTextRows;
try { ({ runtimeTextRows } = await import("../src/i18n/locales/runtimeText.ts")); }
finally { resolver.deregister(); }

// Load the actual TypeScript provider; only resolve its existing extensionless dependency.
// This does not copy a provider or load the UI's extensionless translation aggregate.
const source = await readFile(new URL("../src/utils/cEbpfIntelligence.ts", import.meta.url), "utf8");
const dependency = new URL("../src/utils/semanticCompletion.ts", import.meta.url).href;
const output = ts.transpileModule(source.replace('from "./semanticCompletion"', `from "${dependency}"`), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
}).outputText;
const { registerEbpfIntelligence } = await import(`data:text/javascript;base64,${Buffer.from(output).toString("base64")}`);
const locales = ["en", "zh-CN", "es", "ja"];
const escape = text => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
function localize(locale, message) {
  for (const row of [...cIntelligenceRows, ...runtimeTextRows]) {
    const variables = [], parts = []; let offset = 0;
    for (const match of row[1].matchAll(/\{(\w+)\}/g)) {
      parts.push(escape(row[1].slice(offset, match.index)), "([^\\r\\n]+)");
      variables.push(match[1]); offset = match.index + match[0].length;
    }
    parts.push(escape(row[1].slice(offset)));
    const match = new RegExp(`^${parts.join("")}$`).exec(message);
    if (match) return row[locales.indexOf(locale) + 1]
      .replace(/\{(\w+)\}/g, (_, name) => match[variables.indexOf(name) + 1]);
  }
  return message;
}
function event() {
  const listeners = new Set();
  return { subscribe(callback) { listeners.add(callback); return { dispose: () => listeners.delete(callback) }; },
    fire(value) { for (const callback of [...listeners]) callback(value); } };
}
function fixture() {
  const registrations = [], changed = event(), disposed = event(), language = event(), content = event(), removed = event();
  const model = { dead: false, language: "c", word: "XDP_PASS", source: "int fixture;", version: 7,
    uri: { toString: () => "inmemory:///localization-owned.c" },
    isDisposed() { return this.dead; }, getLanguageId() { return this.language; },
    getWordUntilPosition: () => ({ word: "fixture", startColumn: 1, endColumn: 8 }),
    getWordAtPosition() { return { word: this.word, startColumn: 1, endColumn: this.word.length + 1 }; },
    getValue() { return this.source; }, getVersionId() { return this.version; },
    getLineContent(line) { return this.source.split("\n")[line - 1] || ""; },
    getLineCount() { return this.source.split("\n").length; },
    getPositionAt(offset) { return { lineNumber: 1, column: offset + 1 }; },
    onDidChangeLanguage: language.subscribe, onDidChangeContent: content.subscribe, onWillDispose: removed.subscribe };
  const editor = { current: model, getModel() { return this.current; },
    onDidChangeModel: changed.subscribe, onDidDispose: disposed.subscribe };
  const kinds = { Function: 1, Constant: 2, Snippet: 3, Struct: 4, Field: 5, Object: 6 };
  const monaco = { languages: { CompletionItemKind: kinds, CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
    CompletionTriggerKind: { Invoke: 0, TriggerCharacter: 1 }, SymbolKind: kinds },
    Range: class { constructor(startLineNumber, startColumn, endLineNumber, endColumn) {
      Object.assign(this, { startLineNumber, startColumn, endLineNumber, endColumn });
    } } };
  for (const kind of ["CompletionItem", "Hover", "SignatureHelp", "DocumentSymbol", "Definition", "CodeAction"]) {
    monaco.languages[`register${kind}Provider`] = (_language, provider) => {
      const entry = { kind, provider, removed: 0 }; registrations.push(entry);
      return { dispose() { entry.removed++; } };
    };
  }
  const cancellation = event(), token = { isCancellationRequested: false, onCancellationRequested: cancellation.subscribe };
  return { monaco, editor, model, token, registrations,
    provider: kind => registrations.find(entry => entry.kind === kind).provider };
}
const position = { lineNumber: 1, column: 8 };
const complete = (f, context = { triggerKind: 1, triggerCharacter: "_" }) =>
  f.provider("CompletionItem").provideCompletionItems(f.model, position, context, f.token);
const codeShape = items => items.map(({ label, detail, ...item }) => item);
const marker = (message, code) => ({ message, code, startLineNumber: 1, startColumn: 1,
  endLineNumber: 1, endColumn: 2, severity: 4 });
const actions = (f, markers) => f.provider("CodeAction").provideCodeActions(f.model, null, { markers }).actions;

test("localized C hover descriptions update without changing API titles or provider ownership", t => {
  const f = fixture(); let locale = "zh-CN";
  const handle = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", message => localize(locale, message));
  t.after(() => handle.dispose());
  const entries = [...f.registrations], original = f.editor.current;
  for (const next of locales) {
    locale = next;
    const hover = f.provider("Hover").provideHover(original, position);
    assert.equal(hover.contents[0].value, "**XDP_PASS**");
    assert.equal(hover.contents[1].value, localize(locale, "Allow packet to continue through network stack."));
    assert.equal(f.editor.current, original); assert.equal(original.version, 7);
    assert.equal(original.source, "int fixture;"); assert.equal(original.dead, false);
    assert.deepEqual(f.registrations, entries); assert.ok(entries.every(entry => entry.removed === 0));
  }
});

test("all local C explanation templates have exact four-locale rows and matching placeholders", () => {
  assert.equal(cIntelligenceRows.length, 46);
  assert.equal(new Set(cIntelligenceRows.map(row => row[0])).size, 46);
  assert.equal(new Set(cIntelligenceRows.map(row => row[1])).size, 46);
  for (const row of cIntelligenceRows) {
    assert.equal(row.length, 5);
    const expected = [...row[1].matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort();
    for (const value of row.slice(1)) {
      assert.ok(value.trim().length > 0);
      assert.deepEqual([...value.matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort(), expected);
    }
    for (const value of row.slice(2)) assert.notEqual(value, row[1], `${row[0]} really has translated prose`);
  }
  const available = new Set(cIntelligenceRows.map(row => row[1]));
  for (const match of source.matchAll(/detail: "([^"]+)"/g)) assert.ok(available.has(match[1]), match[1]);
  for (const label of ["hash map declaration", "ring buffer declaration", "per-cpu array declaration", "GPL license"]) {
    assert.ok(available.has(label));
  }
});

test("C snippet display changes but source API labels ranges and insertion bytes retain their default contract", async t => {
  t.mock.method(globalThis, "fetch", () => { throw new Error("static completions must not request a backend"); });
  const original = fixture(), f = fixture(); let locale = "en";
  const old = registerEbpfIntelligence(original.monaco, "https://engine.invalid", original.editor);
  const current = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", message => localize(locale, message));
  t.after(() => { old.dispose(); current.dispose(); });
  const english = (await complete(original)).suggestions;
  assert.equal(english.length, 20);
  for (const next of locales) {
    locale = next;
    const translated = (await complete(f)).suggestions;
    assert.deepEqual(codeShape(translated), codeShape(english));
    assert.deepEqual(translated.map(item => item.label), english.map(item => localize(locale, item.label)));
    assert.deepEqual(translated.map(item => item.detail), english.map(item => localize(locale, item.detail)));
    for (const item of translated.filter(item => /^(SEC |#include |bpf_|XDP_)/.test(item.label))) {
      assert.ok(english.some(original => original.label === item.label), "code and API labels stay literal");
    }
  }
});

test("translated quick fixes choose stable diagnostic codes before prose and preserve legacy English fallback", t => {
  const f = fixture();
  const handle = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", message => localize("zh-CN", message));
  t.after(() => handle.dispose());
  const headers = ["linux/bpf.h", "bpf/bpf_helpers.h"];
  for (const header of headers) {
    const result = actions(f, [marker("原始诊断说明已翻译", `cyanrex.include.${header}`)]);
    assert.equal(result.length, 1);
    assert.equal(result[0].title, localize("zh-CN", `Add #include <${header}>`));
    assert.equal(result[0].edit.edits[0].textEdit.text, `#include <${header}>\n`);
    assert.equal(result[0].edit.edits[0].versionId, 7);
    assert.equal(result[0].edit.edits[0].resource, f.model.uri);
  }
  const gpl = actions(f, [marker("Missing #include <wrong.h>", "cyanrex.license.gpl")]);
  assert.equal(gpl.length, 1, "known GPL code cannot be redirected by conflicting English text");
  assert.equal(gpl[0].title, localize("zh-CN", "Add GPL license declaration"));
  assert.equal(gpl[0].edit.edits[0].textEdit.text, '\nchar _license[] SEC("license") = "GPL";\n');
  const include = actions(f, [marker("Missing GPL license declaration", "cyanrex.include.linux/bpf.h")]);
  assert.equal(include.length, 1); assert.equal(include[0].edit.edits[0].textEdit.text, "#include <linux/bpf.h>\n");
  const linked = actions(f, [marker("翻译后的诊断", { value: "cyanrex.license.gpl", target: f.model.uri })]);
  assert.equal(linked.length, 1); assert.equal(linked[0].edit.edits[0].textEdit.text, gpl[0].edit.edits[0].textEdit.text);
  assert.equal(actions(f, [marker("缺少 GPL 许可证声明")]).length, 0, "translated prose never selects a repair");
  assert.equal(actions(f, [marker("Missing GPL license declaration")]).length, 1);
  assert.equal(actions(f, [marker("Missing #include <vmlinux.h>")])[0].edit.edits[0].textEdit.text, "#include <vmlinux.h>\n");
});

test("local symbol explanations translate without changing source symbols section values or locations", t => {
  const f = fixture();
  f.model.source = 'SEC("xdp")\nint local_hook(struct xdp_md *ctx) { return XDP_PASS; }\nstruct {} local_map SEC(".maps");';
  const old = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor);
  const before = f.provider("DocumentSymbol").provideDocumentSymbols(f.model);
  old.dispose(); f.registrations.length = 0;
  const handle = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", message => localize("ja", message));
  t.after(() => handle.dispose());
  const after = f.provider("DocumentSymbol").provideDocumentSymbols(f.model);
  assert.equal(before.length, 2);
  assert.deepEqual(after.map(({ detail, ...value }) => value), before.map(({ detail, ...value }) => value));
  assert.deepEqual(after.map(item => item.detail), before.map(item => localize("ja", item.detail)));
  assert.deepEqual(after.map(item => item.name), ["local_hook", "local_map"]);
});

test("semantic clang descriptions remain raw external text rather than local display templates", async t => {
  const f = fixture(), raw = "Raw external clang detail: keep exact 中文";
  t.mock.method(globalThis, "fetch", async () => Response.json({ ok: true,
    items: [{ label: "actual_symbol", insert_text: "actual_symbol", detail: raw, kind: "field" }] }));
  const handle = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", message => {
    assert.notEqual(message, raw); assert.notEqual(message, `clang · ${raw}`);
    return localize("es", message);
  });
  t.after(() => handle.dispose());
  const result = await complete(f, { triggerKind: 0 });
  const item = result.suggestions.find(item => item.label === "actual_symbol");
  assert.equal(item.detail, `clang · ${raw}`); assert.equal(item.insertText, "actual_symbol");
});

test("C localization retains foreign-model wrong-language disposal and cancellation rejection", async t => {
  const f = fixture(); let lookups = 0;
  const handle = registerEbpfIntelligence(f.monaco, "https://engine.invalid", f.editor, "", message => {
    lookups++; return localize("ja", message);
  });
  const foreign = { ...f.model, getWordAtPosition() { throw new Error("do not inspect another model"); } };
  assert.equal(f.provider("Hover").provideHover(foreign, position), null);
  assert.equal(f.provider("CodeAction").provideCodeActions(foreign, null, { markers: [marker("Missing GPL license declaration")] }).actions.length, 0);
  f.model.language = "rust";
  assert.equal(f.provider("Hover").provideHover(f.model, position), null);
  f.model.language = "c"; f.token.isCancellationRequested = true;
  assert.deepEqual(await complete(f), { suggestions: [] });
  f.token.isCancellationRequested = false; f.model.dead = true;
  assert.equal(f.provider("Hover").provideHover(f.model, position), null);
  assert.equal(lookups, 0);
  f.model.dead = false;
  for (const word of ["constructor", "__proto__", "toString"]) {
    f.model.word = word;
    assert.equal(f.provider("Hover").provideHover(f.model, position), null, "only own documented APIs may supply display prose");
  }
  assert.equal(lookups, 0); f.model.word = "XDP_PASS";
  assert.equal(f.provider("Hover").provideHover(f.model, position).contents[1].value,
    localize("ja", "Allow packet to continue through network stack."));
  handle.dispose();
  assert.equal(f.provider("Hover").provideHover(f.model, position), null);
  assert.ok(f.registrations.every(entry => entry.removed === 1));
});
