import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import test from "node:test";

const root = new URL("../src/i18n/", import.meta.url).href;
const resolver = registerHooks({ resolve(s, c, next) {
  return c.parentURL?.startsWith(root) && s.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(s)
    ? next(new URL(`${s}.ts`, c.parentURL).href, c) : next(s, c);
} });
let translations, SUPPORTED_LOCALES, localizeLegacyText;
try {
  ({ translations, SUPPORTED_LOCALES } = await import("../src/i18n/translations.ts"));
  ({ localizeLegacyText } = await import("../src/i18n/legacyText.ts"));
} finally { resolver.deregister(); }
const flatten = (object, prefix = "", result = {}) => {
  for (const [key, value] of Object.entries(object)) {
    const full = prefix ? `${prefix}.${key}` : key;
    if (typeof value === "string") result[full] = value; else flatten(value, full, result);
  }
  return result;
};
const english = flatten(translations.en);
const parameters = value => [...value.matchAll(/\{(\w+)\}/g)].map(match => match[1]).sort();

test("Korean is an explicit selectable locale with a complete independent dictionary", () => {
  assert.deepEqual(SUPPORTED_LOCALES.find(locale => locale.code === "ko"), { code: "ko", label: "한국어" });
  assert.ok(translations.ko, "persisted Korean preferences must resolve their own dictionary");
  assert.notEqual(translations.ko, translations.en);
  const korean = flatten(translations.ko);
  assert.equal(Object.keys(english).length, 865);
  assert.deepEqual(Object.keys(korean).sort(), Object.keys(english).sort());
  for (const [key, value] of Object.entries(korean)) {
    assert.ok(value.trim(), key);
    assert.deepEqual(parameters(value), parameters(english[key]), key);
    assert.doesNotMatch(value, /[\u3040-\u30ff\u4e00-\u9fff]/u, `${key}: not copied Japanese/Chinese prose`);
  }
});

test("Korean translates critical identity safety editor and local-only notices rather than relying on English", () => {
  assert.ok(translations.ko);
  const korean = flatten(translations.ko);
  for (const key of ["layout.language", "layout.logoutFailed", "layout.roleTeacher", "auth.loginHintDefault",
    "classroom.discoveryHint", "classroom.joinImpact", "safety.title", "safety.running", "safety.failed", "safety.compilerTarget",
    "editor.localOnly", "editorIntelligence.ariaLabel", "editorIntelligence.plainText", "editorIntelligence.snippets.rust.fn",
    "taskDraft.importFilesConfirm", "taskDraft.filesAdded", "settings.verifyBeforeSave", "learn.documentFallback",
    "aiAgents.endpointHint", "aiAgents.credentialRef", "runtimeText.missingGpl", "runtimeText.cHoverPass"]) {
    assert.match(korean[key], /[\uac00-\ud7a3]/u, key);
    assert.notEqual(korean[key], english[key], key);
  }
});

test("Korean legacy presentation retains literal code protocol identities and teacher text boundaries", () => {
  assert.ok(translations.ko);
  const korean = flatten(translations.ko);
  const t = (key, variables = {}) => {
    assert.ok(key in korean, key);
    return korean[key].replace(/\{(\w+)\}/g, (_, name) => String(variables[name]));
  };
  const result = localizeLegacyText("Run did not complete successfully (stage: compile).", t);
  assert.match(result, /[\uac00-\ud7a3]/u); assert.ok(result.includes("compile"));
  assert.match(localizeLegacyText("Missing GPL license declaration", t), /[\uac00-\ud7a3]/u);
  assert.equal(korean["aiAgents.protocols.openai_responses"], english["aiAgents.protocols.openai_responses"]);
  assert.equal(korean["terminal.moduleNamePlaceholder"], english["terminal.moduleNamePlaceholder"]);
  for (const raw of ["clang raw diagnostic: unexpected token", "// Private Korean code: 한글", "teacher's original comment"])
    assert.equal(localizeLegacyText(raw, t), raw);
});
