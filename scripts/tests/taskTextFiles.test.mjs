import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = path => readFile(new URL(`../../${path}`, import.meta.url), "utf8");
const code = value => value.replace(/\/\/[^\n]*/g, "");

test("text-file import is a local UI adapter without a new backend or editor-dependent Task contract", async () => {
  const helper = code(await read("frontend/src/features/tasks/textFileImport.ts"));
  assert.doesNotMatch(helper, /\bfetch\s*\(|WebSocket|EventSource|localStorage|sessionStorage|artifact_ref|owner_id|\/platform\/v1/);
  for (const path of ["taskDraft.ts", "taskPayloadLanguage.ts"]) {
    assert.doesNotMatch(code(await read(`frontend/src/features/tasks/${path}`)), /BUILTIN_LANGUAGES|detectEditorLanguage|\.\.\/editor/);
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs", "engine/openapi/openapi.json"]) assert.doesNotMatch(await read(path), /textFileImport|task-text-import|importTextFiles/);
});

test("selected text files use bounded preflight and one accepted whole-draft continuation", async () => {
  const helper = await read("frontend/src/features/tasks/textFileImport.ts");
  for (const phrase of ["MAX_TEXT_FILE_BYTES", "TEXT_FILE_IMPORT_TIMEOUT_MS", "preflightTextFiles", "importTextFiles", "serializeTaskDraft", "arrayBuffer", "AbortSignal", "checkReviewed"]) assert.ok(helper.includes(phrase), phrase);
  const workspace = await read("frontend/src/features/tasks/TaskDraftWorkspace.tsx");
  assert.match(workspace, /data-testid="task-text-import"[\s\S]*?\bmultiple\b/);
  assert.ok(workspace.includes('data-testid="task-text-import-open"'));
  const start = workspace.indexOf("const importFiles ="), end = workspace.indexOf("const exportDraft =", start);
  assert.ok(start >= 0 && end > start);
  const flow = workspace.slice(start, end);
  assert.ok(flow.indexOf("preflightTextFiles") < flow.indexOf("safety.request"));
  assert.ok(flow.indexOf("safety.request") < flow.indexOf("await importTextFiles"));
  assert.equal((flow.match(/commit\(next\)/g) || []).length, 1);
  assert.ok(flow.includes("checkReviewed(expected, signal)"));
  assert.ok(flow.includes("!selectedRef.current"));
  assert.doesNotMatch(flow, /commit\([^\n]*true|fetch\(|parseTaskDraft|setDraft\(/);
});

test("text-file import translations retain all four locales and explicit local-only outcomes", async () => {
  const messages = await read("frontend/src/i18n/locales/taskDraft.ts");
  for (const key of ["importFiles", "importFilesConfirm", "filesAdded", "selectedFiles", "importReadFailed"]) assert.equal((messages.match(new RegExp(`\\b${key}:`, "g")) || []).length, 4, key);
  const workspace = await read("frontend/src/features/tasks/TaskDraftWorkspace.tsx");
  for (const phrase of ["taskDraft.importFiles", "taskDraft.importFilesConfirm", "taskDraft.filesAdded", "dangerous: false"]) assert.ok(workspace.includes(phrase), phrase);
  assert.match(workspace, /notice === "taskDraft\.filesAdded"/);
});

test("text-file importer has actual Node and browser test entry points without CI expansion", async () => {
  const packageJson = JSON.parse(await read("frontend/package.json"));
  assert.ok(packageJson.scripts["test:editor-languages"].includes("tests/textFileImport.test.mjs"));
  assert.ok(packageJson.scripts["test:task-payload-browser"].includes("tests/taskTextFiles.browser.mjs"));
  assert.ok(packageJson.scripts["test:task-text-import-browser"].includes("tests/taskTextFiles.browser.mjs"));
  assert.match(await read("scripts/quality-gate.sh"), /run test:editor-languages/);
  assert.match(await read(".github/workflows/ci.yml"), /npm run test:editor-languages/);
  const browser = await read("frontend/tests/taskTextFiles.browser.mjs");
  for (const phrase of ["setupTask", "assertLocalOnly", "setInputFiles", "confirmation", "approve", "arrayBuffer"]) assert.ok(browser.includes(phrase), phrase);
});

test("text-file import has bilingual instructions distinct from server saving and recovery", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const source = await read(`docs/${lang}/local-text-file-import.md`);
    for (const phrase of ["0.5.8", "2026-10-13", "32", "256 KiB", "8 MiB", "UTF-8", "10", "textFileImport", "taskTextFiles", "JSON"]) assert.ok(source.includes(phrase), `${lang}: ${phrase}`);
    for (const path of ["README.md", "editor.md", "architecture-map.md", "testing-guide.md"]) assert.ok((await read(`docs/${lang}/${path}`)).includes("local-text-file-import.md"), `${lang}: ${path}`);
    assert.equal(await read(`frontend/public/course/${lang}/local-text-file-import.md`), source);
  }
});
