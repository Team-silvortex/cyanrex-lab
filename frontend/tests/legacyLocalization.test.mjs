import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";
const root = new URL("../src/", import.meta.url).href;
const resolver = registerHooks({ resolve(s, c, next) {
  return c.parentURL?.startsWith(root) && s.startsWith(".") && !/\.[cm]?[jt]sx?$/.test(s)
    ? next(new URL(`${s}.ts`, c.parentURL).href, c) : next(s, c);
} });
let localizeLegacyText, localDiagnosticCode, runtimeTextRows, cIntelligenceRows, applyMarkers;
try {
  ({ localizeLegacyText, localDiagnosticCode } = await import("../src/i18n/legacyText.ts"));
  ({ runtimeTextRows } = await import("../src/i18n/locales/runtimeText.ts"));
  ({ cIntelligenceRows } = await import("../src/i18n/locales/cIntelligence.ts"));
  ({ applyMarkers } = await import("../src/features/ebpf/editorUtils.ts"));
}
finally { resolver.deregister(); }

const t = (key, vars = {}) => `${key}:${JSON.stringify(vars)}`;
test("known legacy diagnostics and feedback translate exact full messages and preserve parameters", () => {
  assert.equal(localizeLegacyText("Missing GPL license declaration", t), "runtimeText.missingGpl:{}");
  assert.equal(localizeLegacyText("Run did not complete successfully (stage: compile).", t),
    'runtimeText.runFailed:{"stage":"compile"}');
  assert.equal(localizeLegacyText("Required source evidence is missing: null guard for `counter`.", t),
    'runtimeText.requiredNullGuard:{"variable":"counter"}');
});
test("unknown external messages and user bodies are not guessed, normalized or interpreted", () => {
  for (const message of ["Custom compiler failure", "Missing GPL license declaration\n", "prefix: Missing GPL license declaration",
    "// Missing GPL license declaration", "someone exists", "<script>alert(1)</script>", "x".repeat(5000), ""])
    assert.equal(localizeLegacyText(message, t), message);
});
test("local diagnostic identity remains independent from the displayed language", () => {
  assert.equal(localDiagnosticCode("Missing #include <bpf/bpf_helpers.h>"), "cyanrex.include.bpf/bpf_helpers.h");
  assert.equal(localDiagnosticCode("Missing GPL license declaration"), "cyanrex.license.gpl");
  assert.equal(localDiagnosticCode("an external diagnostic"), undefined);
});

const locales = ["en", "zh-CN", "es", "ja"];
const rows = [...runtimeTextRows, ...cIntelligenceRows];
const format = (template, variables) => template.replace(/\{(\w+)\}/g, (_, name) => variables[name]);
const translator = locale => (key, variables = {}) => {
  const row = rows.find(row => `runtimeText.${row[0]}` === key);
  assert.ok(row, `no fallback for ${locale}: ${key}`);
  return format(row[locales.indexOf(locale) + 1], variables);
};
test("every known template resolves through the real adapter in all locales without losing literal parameters", () => {
  assert.equal(new Set(rows.map(row => row[0])).size, rows.length);
  assert.equal(new Set(rows.map(row => row[1])).size, rows.length);
  for (const row of rows) {
    const variables = Object.fromEntries([...row[1].matchAll(/\{(\w+)\}/g)].map(match =>
      [match[1], match[1] === "path" ? "/sys/fs/bpf" : `exact_☕_${match[1]}`]));
    const original = format(row[1], variables);
    for (const locale of locales) assert.equal(localizeLegacyText(original, translator(locale)),
      format(row[locales.indexOf(locale) + 1], variables), `${locale}: ${row[0]}`);
  }
});
test("actual marker projection localizes only display while preserving repair identity and source positions", () => {
  const model = { source: "not read or changed by marker projection" }, captures = [];
  const editor = { getModel: () => model };
  const monaco = { MarkerSeverity: { Error: 8, Warning: 4, Info: 2 },
    editor: { setModelMarkers: (owner, namespace, markers) => captures.push({ owner, namespace, markers }) } };
  const messages = ["Missing GPL license declaration", "Missing #include <bpf/bpf_helpers.h>",
    "Missing #include <linux/bpf.h> (or use #include <vmlinux.h> for CO-RE)", "external clang detail"];
  const diagnostics = messages.map((message, index) => ({ line: index + 1, column: 2, endColumn: 7, severity: "warning", message }));
  const before = JSON.stringify(diagnostics);
  for (const locale of locales) {
    applyMarkers(editor, monaco, diagnostics, translator(locale));
    const captured = captures.at(-1);
    assert.equal(captured.owner, model); assert.equal(captured.namespace, "cyanrex-c-analyzer");
    assert.deepEqual(captured.markers.map(marker => marker.code), ["cyanrex.license.gpl",
      "cyanrex.include.bpf/bpf_helpers.h", "cyanrex.include.linux/bpf.h", undefined]);
    assert.deepEqual(captured.markers.map(marker => marker.message), messages.map(message => localizeLegacyText(message, translator(locale))));
    assert.deepEqual(captured.markers.map(marker => [marker.startLineNumber, marker.startColumn, marker.endColumn, marker.severity]),
      messages.map((_, index) => [index + 1, 2, 7, 4]));
  }
  assert.equal(JSON.stringify(diagnostics), before);
  assert.equal(model.source, "not read or changed by marker projection");
});
