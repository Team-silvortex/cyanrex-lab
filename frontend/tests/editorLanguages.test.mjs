import assert from "node:assert/strict";
import test from "node:test";
import { BUILTIN_LANGUAGES, detectEditorLanguage, getEditorLanguage } from "../src/features/editor/languages.ts";
import { configureBuiltinLanguageServices, registerBasicLanguageIntelligence } from "../src/features/editor/languageServices.ts";

function defaults() {
  return {
    calls: [],
    setCompilerOptions(value) { this.compiler = value; this.calls.push("compiler"); },
    setDiagnosticsOptions(value) { this.diagnostics = value; this.calls.push("diagnostics"); },
    setModeConfiguration(value) { this.mode = value; this.calls.push("mode"); },
    setWorkerOptions(value) { this.worker = value; this.calls.push("worker"); },
    setEagerModelSync(value) { this.eager = value; this.calls.push("eager"); },
    setOptions(value) { this.options = value; this.calls.push("options"); },
  };
}
function monacoFixture() {
  const typescriptDefaults = defaults(), javascriptDefaults = defaults();
  const jsonDefaults = defaults(), htmlDefaults = defaults(), cssDefaults = defaults();
  const registrations = [];
  const monaco = {
    typescript: { typescriptDefaults, javascriptDefaults, ScriptTarget: { ESNext: 99 },
      ModuleKind: { ESNext: 99 }, ModuleResolutionKind: { NodeJs: 2 }, JsxEmit: { Preserve: 1 } },
    json: { jsonDefaults }, html: { htmlDefaults }, css: { cssDefaults },
    languages: {
    // Monaco 0.55.1 moved service namespaces to the top level; old aliases are not typed APIs.
    typescript: { deprecated: true }, json: { deprecated: true },
    html: { deprecated: true }, css: { deprecated: true },
    CompletionItemKind: { Snippet: 27 }, CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
    registerCompletionItemProvider(language, provider) {
      const entry = { language, provider, removed: 0 };
      registrations.push(entry);
      return { dispose() { entry.removed++; } };
    },
  } };
  return { monaco, registrations, typescriptDefaults, javascriptDefaults, jsonDefaults, htmlDefaults, cssDefaults };
}
function editorFixture(language = "rust") {
  const listeners = new Set();
  const model = { language, dead: false, isDisposed() { return this.dead; },
    getLanguageId() { return this.language; },
    getWordUntilPosition() { return { word: "fn", startColumn: 2, endColumn: 4 }; } };
  const editor = { model, getModel() { return this.model; },
    onDidDispose(callback) { listeners.add(callback); return { dispose: () => listeners.delete(callback) }; } };
  return { editor, model, listeners, token: { isCancellationRequested: false },
    destroy() { for (const listener of [...listeners]) listener(); } };
}
const request = (entry, model, token) => entry.provider.provideCompletionItems(
  model, { lineNumber: 3, column: 4 }, { triggerKind: 0 }, token,
);

test("registry is immutable, unique and contains every advertised built-in language", () => {
  const expected = ["typescript", "javascript", "json", "html", "css", "rust", "python", "c", "cpp", "markdown", "yaml", "sql", "shell", "plaintext"];
  assert.deepEqual(BUILTIN_LANGUAGES.map(item => item.id), expected);
  assert.equal(new Set(BUILTIN_LANGUAGES.map(item => item.id)).size, expected.length);
  assert.ok(Object.isFrozen(BUILTIN_LANGUAGES));
  for (const item of BUILTIN_LANGUAGES) {
    assert.equal(getEditorLanguage(item.id), item);
    assert.match(item.extension, /^\.[a-z]+$/);
    assert.ok(item.label.length > 0);
    assert.ok(Object.isFrozen(item));
    assert.ok(Object.isFrozen(item.capabilities));
    assert.equal(new Set(item.capabilities).size, item.capabilities.length);
    assert.equal(detectEditorLanguage(`draft${item.extension}`), item);
  }
});

test("unknown language IDs never silently select a different service", () => {
  for (const id of ["", "TypeScript", "js", "__proto__", "constructor", "rust-analyzer"]) {
    assert.equal(getEditorLanguage(id), undefined);
  }
});

test("filename detection uses the basename and explicit aliases with plaintext fallback", () => {
  for (const [filename, language] of [
    ["/work/component.tsx", "plaintext"], ["entry.mts", "typescript"], ["entry.cts", "typescript"],
    ["main.JSX", "plaintext"], ["main.mjs", "javascript"], ["main.cjs", "javascript"],
    ["C:\\work\\MAIN.PY", "python"], ["header.h", "c"], ["header.hpp", "cpp"], ["main.cc", "cpp"],
    ["README.MARKDOWN", "markdown"], ["config.yml", "yaml"], [".bashrc", "shell"], [".zshrc", "shell"],
    [".profile", "shell"], ["script.bash", "shell"], ["index.htm", "html"],
    ["", "plaintext"], ["README", "plaintext"], ["file.ts.bak", "plaintext"],
    ["/folder.rs/unknown", "plaintext"], ["document.jsonc", "plaintext"], ["file.unknown", "plaintext"],
  ]) assert.equal(detectEditorLanguage(filename).id, language, filename);
});

test("capabilities distinguish semantic workers, structural services and snippet-only languages", () => {
  const semantic = ["completion", "hover", "signature", "definition", "references", "rename", "symbols", "format", "diagnostics"];
  for (const id of ["typescript", "javascript"]) {
    assert.equal(getEditorLanguage(id).tier, "semantic");
    assert.deepEqual(getEditorLanguage(id).capabilities, semantic);
  }
  assert.deepEqual(getEditorLanguage("json").capabilities, ["completion", "hover", "symbols", "format", "diagnostics"]);
  assert.deepEqual(getEditorLanguage("html").capabilities, ["completion", "hover", "rename", "symbols", "format"]);
  assert.deepEqual(getEditorLanguage("css").capabilities, ["completion", "hover", "definition", "references", "rename", "symbols", "format", "diagnostics"]);
  for (const item of BUILTIN_LANGUAGES.filter(item => item.tier === "basic")) assert.deepEqual(item.capabilities, ["completion"]);
  assert.equal(getEditorLanguage("plaintext").tier, "text");
  assert.deepEqual(getEditorLanguage("plaintext").capabilities, []);
});

test("local service configuration is strict, isolated and applied once per Monaco instance", t => {
  t.mock.method(globalThis, "fetch", () => { throw new Error("configuration must not request network"); });
  const f = monacoFixture();
  configureBuiltinLanguageServices(f.monaco);
  for (const service of [f.typescriptDefaults, f.javascriptDefaults]) {
    assert.equal(service.compiler.strict, true);
    assert.equal(service.compiler.moduleDetection, 3);
    assert.equal(service.compiler.noEmit, true);
    assert.deepEqual(service.compiler.types, []);
    assert.deepEqual(service.compiler.typeRoots, []);
    assert.deepEqual(service.worker, {});
    assert.equal(service.eager, false);
    assert.equal(service.diagnostics.noSemanticValidation, false);
    assert.equal(service.diagnostics.noSyntaxValidation, false);
    for (const key of ["completionItems", "hovers", "signatureHelp", "definitions", "references", "rename", "documentSymbols", "documentRangeFormattingEdits", "diagnostics"]) assert.equal(service.mode[key], true, key);
  }
  assert.equal(f.javascriptDefaults.compiler.checkJs, true);
  assert.equal(f.javascriptDefaults.compiler.allowJs, true);
  assert.equal(f.jsonDefaults.diagnostics.enableSchemaRequest, false);
  assert.deepEqual(f.jsonDefaults.diagnostics.schemas, []);
  assert.equal(f.jsonDefaults.diagnostics.validate, true);
  assert.equal(f.jsonDefaults.mode.diagnostics, true);
  assert.equal(f.htmlDefaults.mode.links, false);
  assert.equal(f.htmlDefaults.mode.diagnostics, false);
  assert.equal(f.cssDefaults.options.validate, true);
  const counts = [f.typescriptDefaults, f.javascriptDefaults, f.jsonDefaults, f.htmlDefaults, f.cssDefaults].map(value => value.calls.length);
  configureBuiltinLanguageServices(f.monaco);
  assert.deepEqual([f.typescriptDefaults, f.javascriptDefaults, f.jsonDefaults, f.htmlDefaults, f.cssDefaults].map(value => value.calls.length), counts);
  const other = monacoFixture(); configureBuiltinLanguageServices(other.monaco);
  assert.ok(other.typescriptDefaults.calls.length > 0);
});

test("configured real TypeScript worker checks types without globals leakage or package acquisition", async t => {
  let requests = 0;
  t.mock.method(globalThis, "fetch", () => { requests++; throw new Error("package acquisition forbidden"); });
  const { TypeScriptWorker } = await import("../node_modules/monaco-editor/esm/vs/language/typescript/tsWorker.js");
  const f = monacoFixture(); configureBuiltinLanguageServices(f.monaco);
  const uri = name => ({ path: `/${name}`, toString: () => `file:///${name}` });
  const models = [
    { uri: uri("first.ts"), version: 1, getValue: () => "const shared = 1; const onlyFirst = 2;" },
    { uri: uri("second.ts"), version: 1, getValue: () => 'const shared: number = "wrong"; onlyFirst;' },
    { uri: uri("packages.ts"), version: 1, getValue: () => 'import { missing } from "not-installed-package"; missing();' },
  ];
  const worker = new TypeScriptWorker({ getMirrorModels: () => models }, { compilerOptions: f.typescriptDefaults.compiler, extraLibs: {}, inlayHintsOptions: {} });
  const diagnostics = await worker.getSemanticDiagnostics("file:///second.ts");
  assert.ok(diagnostics.some(item => item.code === 2322), "real type mismatch must be reported");
  assert.ok(diagnostics.some(item => item.code === 2304), "another model's local cannot become a global");
  assert.ok(!diagnostics.some(item => item.code === 2451), "same local name in separate models is legal");
  assert.ok((await worker.getSemanticDiagnostics("file:///packages.ts")).some(item => item.code === 2307));
  const js = [{ uri: uri("check.js"), version: 1, getValue: () => '/** @type {number} */\nconst amount = "wrong";' }];
  const jsWorker = new TypeScriptWorker({ getMirrorModels: () => js }, { compilerOptions: f.javascriptDefaults.compiler, extraLibs: {}, inlayHintsOptions: {} });
  assert.ok((await jsWorker.getSemanticDiagnostics("file:///check.js")).some(item => item.code === 2322));
  assert.equal(requests, 0);
});

test("configured real JSON worker never fetches a document's remote schema", async t => {
  let requests = 0;
  t.mock.method(globalThis, "fetch", () => { requests++; throw new Error("remote schema forbidden"); });
  const { JSONWorker } = await import("../node_modules/monaco-editor/esm/vs/language/json/jsonWorker.js");
  const f = monacoFixture(); configureBuiltinLanguageServices(f.monaco);
  let source = '{"$schema":"https://example.invalid/schema.json","value": }';
  const model = { uri: { toString: () => "file:///draft.json" }, version: 1,
    getValue: () => source };
  const worker = new JSONWorker({ getMirrorModels: () => [model] }, {
    languageId: "json", languageSettings: f.jsonDefaults.diagnostics,
    enableSchemaRequest: f.jsonDefaults.diagnostics.enableSchemaRequest,
  });
  assert.ok((await worker.doValidation("file:///draft.json")).length > 0, "syntax diagnostics remain enabled");
  await worker.doComplete("file:///draft.json", { line: 0, character: 1 });
  source = '{"$schema":"http://json-schema.org/draft-07/schema#","title":"Local schema","type":"object"}';
  model.version++;
  const hover = await worker.doHover("file:///draft.json", { line: 0, character: source.indexOf('"title"') + 2 });
  assert.ok(hover?.contents.join("").length > 0, "bundled schema metadata can provide a real offline hover");
  assert.ok((await worker.findDocumentSymbols("file:///draft.json")).length > 0);
  assert.ok((await worker.format("file:///draft.json", undefined, { tabSize: 2, insertSpaces: true })).length > 0);
  assert.equal(requests, 0);
});

test("every basic language gets bounded local snippets with snippet insertion ranges", t => {
  t.mock.method(globalThis, "fetch", () => { throw new Error("snippets must stay local"); });
  const f = monacoFixture(), e = editorFixture();
  const disposable = registerBasicLanguageIntelligence(f.monaco, e.editor);
  t.after(() => disposable.dispose());
  assert.deepEqual(f.registrations.map(item => item.language).sort(), BUILTIN_LANGUAGES.filter(item => item.tier === "basic").map(item => item.id).sort());
  for (const entry of f.registrations) {
    e.model.language = entry.language;
    const { suggestions } = request(entry, e.model, e.token);
    assert.ok(suggestions.length >= 2 && suggestions.length <= 8);
    for (const item of suggestions) {
      assert.equal(item.kind, 27); assert.equal(item.insertTextRules, 4);
      assert.ok(item.label && item.insertText && item.detail);
      assert.equal(item.command, undefined); assert.equal(item.additionalTextEdits, undefined);
      assert.deepEqual(item.range, { startLineNumber: 3, endLineNumber: 3, startColumn: 2, endColumn: 4 });
    }
  }
  e.model.language = "rust";
  const fn = request(f.registrations.find(item => item.language === "rust"), e.model, e.token).suggestions.find(item => item.label === "fn");
  assert.equal(fn.insertText, "fn ${1:name}(${2}) {\n\t${0}\n}");
});

test("providers ignore foreign, replaced, disposed and language-mismatched models", t => {
  const f = monacoFixture(), e = editorFixture();
  const disposable = registerBasicLanguageIntelligence(f.monaco, e.editor); t.after(() => disposable.dispose());
  const rust = f.registrations.find(item => item.language === "rust");
  const foreign = { ...e.model, getWordUntilPosition() { throw new Error("must not inspect foreign content"); } };
  assert.deepEqual(request(rust, foreign, e.token), { suggestions: [] });
  e.model.language = "python"; assert.deepEqual(request(rust, e.model, e.token), { suggestions: [] });
  e.model.language = "rust"; e.model.dead = true; assert.deepEqual(request(rust, e.model, e.token), { suggestions: [] });
  e.model.dead = false; e.editor.model = foreign; assert.deepEqual(request(rust, e.model, e.token), { suggestions: [] });
  e.editor.model = null; assert.deepEqual(request(rust, e.model, e.token), { suggestions: [] });
});

test("cancellation and disposal return empty results and unregister every provider exactly once", () => {
  const f = monacoFixture(), e = editorFixture();
  const disposable = registerBasicLanguageIntelligence(f.monaco, e.editor);
  const rust = f.registrations.find(item => item.language === "rust");
  e.token.isCancellationRequested = true;
  assert.deepEqual(request(rust, e.model, e.token), { suggestions: [] });
  e.token.isCancellationRequested = false;
  e.model.getWordUntilPosition = () => { e.token.isCancellationRequested = true; return { startColumn: 2, endColumn: 4 }; };
  assert.deepEqual(request(rust, e.model, e.token), { suggestions: [] });
  e.token.isCancellationRequested = false;
  disposable.dispose(); disposable.dispose();
  assert.deepEqual(request(rust, e.model, e.token), { suggestions: [] });
  assert.ok(f.registrations.every(item => item.removed === 1));
  assert.equal(e.listeners.size, 0);
});

test("editor destruction unregisters its providers without affecting another editor", t => {
  const f = monacoFixture(), first = editorFixture(), second = editorFixture("python");
  const firstHandle = registerBasicLanguageIntelligence(f.monaco, first.editor);
  const firstEntries = [...f.registrations];
  const secondHandle = registerBasicLanguageIntelligence(f.monaco, second.editor);
  t.after(() => { firstHandle.dispose(); secondHandle.dispose(); });
  first.destroy();
  assert.ok(firstEntries.every(item => item.removed === 1));
  const secondEntries = f.registrations.slice(firstEntries.length);
  assert.ok(secondEntries.every(item => item.removed === 0));
  assert.ok(request(secondEntries.find(item => item.language === "python"), second.model, second.token).suggestions.length > 0);
  assert.equal(first.listeners.size, 0); assert.equal(second.listeners.size, 1);
});
