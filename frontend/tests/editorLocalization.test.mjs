import assert from "node:assert/strict";
import test from "node:test";
import { registerBasicLanguageIntelligence } from "../src/features/editor/languageServices.ts";
import { editorIntelligenceMessages } from "../src/i18n/locales/editorIntelligence.ts";

const locales = ["en", "zh-CN", "es", "ja"];
const templateLabels = {
  rust: ["fn", "let", "match"], python: ["def", "class", "for"],
  c: ["main", "for", "include"], cpp: ["main", "class", "for"],
  markdown: ["heading", "link", "code"], yaml: ["mapping", "list"],
  sql: ["select", "create_table"], shell: ["if", "for", "function"],
};
const placeholders = text => [...text.matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort();
function leaves(value, prefix = "") {
  return Object.entries(value).flatMap(([key, entry]) => typeof entry === "string"
    ? [[`${prefix}${key}`, entry]] : leaves(entry, `${prefix}${key}.`));
}
function translate(locale, key) {
  assert.ok(key.startsWith("editorIntelligence."), "provider requests its own display namespace");
  const result = key.slice("editorIntelligence.".length).split(".")
    .reduce((value, part) => value?.[part], editorIntelligenceMessages[locale]);
  assert.equal(typeof result, "string", `${locale}: ${key} must be defined, not an English fallback`);
  return result;
}
function fixture(language = "rust") {
  const registrations = [], listeners = new Set();
  const model = { language, dead: false, text: "original local source", version: 7,
    isDisposed() { return this.dead; }, getLanguageId() { return this.language; },
    getWordUntilPosition() { return { word: "fn", startColumn: 2, endColumn: 4 }; } };
  const editor = { current: model, getModel() { return this.current; },
    onDidDispose(callback) { listeners.add(callback); return { dispose: () => listeners.delete(callback) }; } };
  const monaco = { languages: {
    CompletionItemKind: { Snippet: 27 }, CompletionItemInsertTextRule: { InsertAsSnippet: 4 },
    registerCompletionItemProvider(language, provider) {
      const entry = { language, provider, removed: 0 };
      registrations.push(entry);
      return { dispose() { entry.removed++; } };
    },
  } };
  const token = { isCancellationRequested: false };
  return { monaco, editor, model, token, registrations, listeners,
    provider: language => registrations.find(entry => entry.language === language),
    destroy() { for (const listener of [...listeners]) listener(); } };
}
const request = (entry, model, token) => entry.provider.provideCompletionItems(
  model, { lineNumber: 3, column: 4 }, { triggerKind: 0 }, token,
);
const codeShape = items => items.map(({ detail, ...item }) => item);

test("editor intelligence dictionaries cover four locales with matching keys and placeholders", () => {
  const english = leaves(editorIntelligenceMessages.en);
  assert.equal(english.length, 24);
  assert.deepEqual(Object.keys(editorIntelligenceMessages).sort(), [...locales].sort());
  for (const locale of locales) {
    const entries = leaves(editorIntelligenceMessages[locale]);
    assert.deepEqual(entries.map(([key]) => key).sort(), english.map(([key]) => key).sort());
    assert.deepEqual(Object.keys(editorIntelligenceMessages[locale].snippets), Object.keys(templateLabels));
    for (const [language, labels] of Object.entries(templateLabels)) {
      assert.deepEqual(Object.keys(editorIntelligenceMessages[locale].snippets[language]), labels);
    }
    for (const [key, value] of entries) {
      assert.ok(value.trim().length > 0, `${locale}: ${key}`);
      assert.deepEqual(placeholders(value), placeholders(english.find(([name]) => name === key)[1]));
      if (locale !== "en") assert.notEqual(value, translate("en", `editorIntelligence.${key}`), `${locale}: ${key} is actually translated`);
    }
  }
});

test("registered basic providers use current locale without rebuilding models or registrations", t => {
  const f = fixture(); let locale = "zh-CN";
  const handle = registerBasicLanguageIntelligence(f.monaco, f.editor, key => translate(locale, key));
  t.after(() => handle.dispose());
  const entries = [...f.registrations], original = f.editor.current;
  assert.equal(entries.length, 8);
  let originalCode;
  for (const next of locales) {
    locale = next;
    const suggestions = request(f.provider("rust"), original, f.token).suggestions;
    assert.equal(suggestions.length, 3);
    assert.equal(suggestions[0].detail, editorIntelligenceMessages[locale].snippets.rust.fn);
    if (!originalCode) originalCode = codeShape(suggestions);
    else assert.deepEqual(codeShape(suggestions), originalCode, "locale never changes inserted source or completion identity");
    assert.equal(f.editor.current, original);
    assert.equal(original.dead, false); assert.equal(original.text, "original local source");
    assert.equal(original.version, 7);
    assert.deepEqual(f.registrations, entries); assert.ok(entries.every(entry => entry.removed === 0));
    assert.equal(f.listeners.size, 1);
  }
});

test("every basic snippet localizes only its detail and preserves the default English insertion contract", t => {
  const original = fixture(), localized = fixture(); let locale = "en";
  const originalHandle = registerBasicLanguageIntelligence(original.monaco, original.editor);
  const localizedHandle = registerBasicLanguageIntelligence(localized.monaco, localized.editor, key => translate(locale, key));
  t.after(() => { originalHandle.dispose(); localizedHandle.dispose(); });
  for (const [language, labels] of Object.entries(templateLabels)) {
    original.model.language = language; localized.model.language = language;
    const english = request(original.provider(language), original.model, original.token).suggestions;
    assert.deepEqual(english.map(item => item.label), labels);
    assert.deepEqual(english.map(item => item.detail), labels.map(label => editorIntelligenceMessages.en.snippets[language][label]));
    for (const next of locales) {
      locale = next;
      const translated = request(localized.provider(language), localized.model, localized.token).suggestions;
      assert.deepEqual(codeShape(translated), codeShape(english));
      assert.deepEqual(translated.map(item => item.detail), labels.map(label => editorIntelligenceMessages[locale].snippets[language][label]));
    }
  }
  assert.equal(request(original.provider("rust"), { ...original.model, language: "rust" }, original.token).suggestions.length, 0);
});

test("localized providers never inspect or translate foreign replaced disposed cancelled or wrong-language models", t => {
  const f = fixture(); let translated = 0;
  const handle = registerBasicLanguageIntelligence(f.monaco, f.editor, key => { translated++; return translate("zh-CN", key); });
  t.after(() => handle.dispose());
  const rust = f.provider("rust");
  const foreign = { ...f.model, getWordUntilPosition() { throw new Error("foreign content must not be inspected"); } };
  assert.deepEqual(request(rust, foreign, f.token), { suggestions: [] });
  f.editor.current = foreign; assert.deepEqual(request(rust, f.model, f.token), { suggestions: [] });
  f.editor.current = null; assert.deepEqual(request(rust, f.model, f.token), { suggestions: [] });
  f.editor.current = f.model;
  f.model.dead = true; assert.deepEqual(request(rust, f.model, f.token), { suggestions: [] });
  f.model.dead = false; f.model.language = "python";
  assert.deepEqual(request(rust, f.model, f.token), { suggestions: [] });
  f.model.language = "rust"; f.token.isCancellationRequested = true;
  assert.deepEqual(request(rust, f.model, f.token), { suggestions: [] });
  assert.equal(translated, 0);
  f.token.isCancellationRequested = false;
  assert.equal(request(rust, f.model, f.token).suggestions.length, 3, "a healthy current request still produces suggestions");
  assert.equal(translated, 3);
});

test("locale lookup cannot bypass cancellation that occurs while suggestions are prepared", t => {
  const f = fixture(); let calls = 0;
  const handle = registerBasicLanguageIntelligence(f.monaco, f.editor, key => {
    calls++; f.token.isCancellationRequested = true; return translate("ja", key);
  });
  t.after(() => handle.dispose());
  assert.deepEqual(request(f.provider("rust"), f.model, f.token), { suggestions: [] });
  assert.ok(calls > 0, "the cancellation is triggered by the actual display lookup");
});

test("localized provider disposal is once-only and does not remove another editor's registrations", t => {
  const f = fixture(), other = fixture("python");
  const first = registerBasicLanguageIntelligence(f.monaco, f.editor, key => translate("es", key));
  const own = [...f.registrations];
  const second = registerBasicLanguageIntelligence(f.monaco, other.editor, key => translate("ja", key));
  const surviving = f.registrations.slice(own.length);
  t.after(() => { first.dispose(); second.dispose(); });
  f.destroy(); first.dispose();
  assert.ok(own.every(entry => entry.removed === 1));
  assert.deepEqual(request(own.find(entry => entry.language === "rust"), f.model, f.token), { suggestions: [] });
  assert.ok(surviving.every(entry => entry.removed === 0));
  const suggestions = request(surviving.find(entry => entry.language === "python"), other.model, other.token).suggestions;
  assert.equal(suggestions[0].detail, editorIntelligenceMessages.ja.snippets.python.def);
  assert.equal(f.listeners.size, 0); assert.equal(other.listeners.size, 1);
});
