import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import ts from "typescript";

const root = new URL("../src/i18n/", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, next) {
  if (context.parentURL?.startsWith(root) && specifier.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(specifier)) {
    return next(new URL(`${specifier}.ts`, context.parentURL).href, context);
  }
  return next(specifier, context);
} });
let translations;
try { ({ translations } = await import("../src/i18n/translations.ts")); }
finally { resolver.deregister(); }
function flatten(dict, prefix = "", result = {}) {
  for (const [key, value] of Object.entries(dict)) {
    const name = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") result[name] = value;
    else flatten(value, name, result);
  }
  return result;
}
const dictionaries = Object.fromEntries(Object.entries(translations).map(([locale, dict]) => [locale, flatten(dict)]));
const placeholders = text => [...text.matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort();

test("every supported locale explicitly covers the complete English dictionary with matching interpolation", () => {
  const expected = Object.keys(dictionaries.en).sort();
  for (const [locale, dict] of Object.entries(dictionaries)) {
    assert.deepEqual(Object.keys(dict).sort(), expected, locale);
    for (const [key, value] of Object.entries(dict)) {
      assert.ok(value.trim(), `${locale}.${key} is not blank`);
      assert.deepEqual(placeholders(value), placeholders(dictionaries.en[key]), `${locale}.${key}`);
    }
  }
});

test("Chinese human-facing labels do not remain English while protocol names and examples stay literal", () => {
  for (const key of ["auth.secretLabel", "ebpf.detachedCount", "ebpf.detachState", "ebpf.resultPinPath"]) {
    assert.notEqual(dictionaries["zh-CN"][key], dictionaries.en[key], key);
  }
  for (const key of ["aiAgents.protocols.openai_responses", "aiAgents.protocols.anthropic_messages",
    "aiAgents.protocols.gemini_generate_content", "terminal.moduleNamePlaceholder"]) {
    assert.equal(dictionaries["zh-CN"][key], dictionaries.en[key], key);
  }
});

test("literal interface translation calls resolve without English fallback or raw keys", () => {
  const missing = [];
  let calls = 0;
  function visitDirectory(directory) {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const filename = path.join(directory, entry.name);
      if (entry.isDirectory()) visitDirectory(filename);
      else if (/\.[jt]sx?$/.test(filename)) {
        const source = ts.createSourceFile(filename, readFileSync(filename, "utf8"), ts.ScriptTarget.Latest, true);
        function visit(node) {
          if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "t") {
            const first = node.arguments[0];
            if (first && (ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))) {
              calls++;
              for (const [locale, dict] of Object.entries(dictionaries)) {
                if (!(first.text in dict)) missing.push(`${locale}: ${filename}: ${first.text}`);
              }
            }
          }
          ts.forEachChild(node, visit);
        }
        visit(source);
      }
    }
  }
  visitDirectory(new URL("../src", import.meta.url).pathname);
  visitDirectory(new URL("../pages", import.meta.url).pathname);
  assert.ok(calls > 700, "check real application sources, not only a small fixture");
  assert.deepEqual(missing, []);
});
