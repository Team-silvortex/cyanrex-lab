import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = path => readFile(new URL(`../../${path}`, import.meta.url), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => source.replace(/\s+/g, "");
const viewPath = "frontend/src/features/tasks/TaskDraftWorkspace.tsx";
const controllerPath = "frontend/src/features/tasks/useTaskDraftController.ts";
const operations = ["changeTitle", "addText", "changeItem", "removeText", "reset", "importDraft", "importFiles", "exportDraft", "selectItem"];

function section(source, start, end) {
  const from = source.indexOf(start), to = source.indexOf(end, from + start.length);
  assert.ok(from >= 0 && to > from, `missing bounded source section: ${start} → ${end}`);
  return source.slice(from, to);
}

// Structural ownership checks complement the existing actual browser regressions. They
// do not establish persistence, server authorization, or runtime isolation.
test("Task draft view owns only rendering and native file input handles, not mutable draft authority", async () => {
  const view = codeOnly(await read(viewPath));
  assert.match(view, /import\s*\{\s*useTaskDraftController\s*\}\s*from\s*["']\.\/useTaskDraftController["']/);
  assert.match(view, /useTaskDraftController\s*\(\s*t\s*\)/);
  assert.equal([...view.matchAll(/\buseRef\s*</g)].length, 2);
  for (const name of ["upload", "textFiles"]) assert.match(view, new RegExp(`const\\s+${name}\\s*=\\s*useRef<HTMLInputElement>\\(null\\)`));
  assert.doesNotMatch(view, /\b(?:useState|useEffect|useConfirmedAction|useDraftWarning|draftRef|revision|generationRef|selectedRef|mounted|safety|choose|commit|checkReviewed|setDraft|setDirty|setNotice|setSelectedId|setGeneration)\b/);
  assert.doesNotMatch(view, /\b(?:updateTaskTitle|createTaskDraft|createTextPayloadItem|addPayloadItem|updatePayloadItem|removePayloadItem|parseTaskDraft|serializeTaskDraft|preflightTextFiles|importTextFiles)\b/);
  assert.match(view, /MAX_TASK_PAYLOAD_ITEMS[\s\S]*?from\s*["']\.\/taskDraft["']/);
  assert.match(view, /onClick=\{\(\)\s*=>\s*selectItem\(item\.id\)\}/);
  assert.match(view, /onChange=\{\(expected,\s*patch\)\s*=>\s*changeItem\(selected\.id,\s*generation,\s*expected,\s*patch\)\}/);
  assert.ok(view.includes("{dialog}") && view.includes("disabled={busy}"));
});

test("private controller owns revisions and replacement generations with refs updated before renders", async () => {
  const controller = codeOnly(await read(controllerPath));
  assert.match(controller, /export\s+function\s+useTaskDraftController\s*\(/);
  assert.match(controller, /vars\?\s*:\s*Record\s*<\s*string\s*,\s*string\s*\|\s*number\s*>/);
  for (const name of ["draftRef", "revision", "generationRef", "selectedRef", "mounted"]) assert.match(controller, new RegExp(`const\\s+${name}\\s*=\\s*useRef\\b`));
  assert.match(controller, /useDraftWarning\(dirty,\s*t\("taskDraft\.leaveConfirm"\)\)/);
  assert.ok(compact(controller).includes("return()=>{mounted.current=false;revision.current++;}"));
  const commit = compact(section(controller, "const commit =", "const checkReviewed ="));
  const draft = commit.indexOf("draftRef.current=next;"), revision = commit.indexOf("revision.current++;");
  const generation = commit.indexOf("generationRef.current++;");
  const rendered = commit.indexOf("setDraft(next)");
  assert.ok(draft >= 0 && draft < revision && revision < generation && generation < rendered);
  assert.ok(commit.includes("setGeneration(generationRef.current)"));
  assert.ok(commit.includes("choose(next.payload[0]?.id??null)"));
  assert.ok(commit.includes("setDirty(!empty)") && commit.includes('setNotice("")'));
  const reviewed = section(controller, "const checkReviewed =", "const changeTitle =");
  for (const check of ["!mounted.current", "signal.aborted", "revision.current !== expected"]) assert.ok(reviewed.includes(check), check);
  const changed = section(controller, "const changeItem =", "const removeText =");
  for (const check of ["!mounted.current", "safety.isBusy()", "generationRef.current !== ownerGeneration", "selectedRef.current !== id", "updatePayloadItem(draftRef.current, id, expectedRevision, patch)", "throw new Error(key)"]) assert.ok(changed.includes(check), check);
});

test("controller keeps reviewed confirmations and atomic async imports without giving the view continuation ownership", async () => {
  const controller = codeOnly(await read(controllerPath));
  for (const [name, next] of [["removeText", "reset"], ["reset", "importDraft"], ["importDraft", "importFiles"], ["importFiles", "exportDraft"]]) {
    const flow = section(controller, `const ${name} =`, `const ${next} =`);
    assert.ok(flow.includes("!mounted.current") && flow.includes("safety.isBusy()"), name);
    const revision = flow.indexOf("revision.current"), request = flow.indexOf("safety.request");
    const reviewed = flow.indexOf("checkReviewed(expected, signal)");
    assert.ok(revision >= 0 && revision < request && request < reviewed, name);
  }
  const draft = section(controller, "const importDraft =", "const importFiles =");
  assert.equal([...draft.matchAll(/checkReviewed\(expected, signal\)/g)].length, 3);
  assert.ok(draft.indexOf("safety.request") < draft.indexOf("await file.arrayBuffer()"));
  assert.ok(draft.indexOf("parseTaskDraft(bytes)") < draft.lastIndexOf("checkReviewed(expected, signal)"));
  assert.ok(draft.lastIndexOf("checkReviewed(expected, signal)") < draft.indexOf("commit(next, true)"));
  const files = section(controller, "const importFiles =", "const exportDraft =");
  assert.ok(files.includes("Object.freeze([...selection])"));
  assert.ok(files.indexOf("preflightTextFiles") < files.indexOf("safety.request"));
  assert.ok(files.indexOf("safety.request") < files.indexOf("await importTextFiles"));
  assert.equal([...files.matchAll(/commit\(next\)/g)].length, 1);
  assert.ok(files.lastIndexOf("checkReviewed(expected, signal)") < files.indexOf("commit(next)"));
  assert.doesNotMatch(files, /commit\([^\n]*true|setDraft\(/);
});

test("controller exposes only render data and bounded operations, with selection guarded inside its owner", async () => {
  const controller = codeOnly(await read(controllerPath));
  const returned = controller.match(/return\s*\{([^{}]*)\}\s*;?\s*\}\s*$/)?.[1];
  assert.ok(returned, "one explicit final controller return surface");
  const entries = returned.split(",").map(entry => entry.trim()).filter(Boolean);
  const names = entries.map(entry => entry.split(":")[0].trim()).sort();
  assert.deepEqual(names, ["draft", "generation", "selectedId", "selected", "dirty", "notice", "busy", "dialog", ...operations].sort());
  assert.ok(entries.includes("busy: safety.busy") && entries.includes("dialog: safety.dialog"));
  assert.doesNotMatch(returned, /\.\.\.|\b(?:draftRef|revision|generationRef|selectedRef|mounted|safety)\s*(?:,|$)|\b(?:set\w+|choose|commit|checkReviewed)\b/);
  const selected = controller.match(/const\s+selectItem\s*=\s*\([^)]*\)\s*=>\s*\{([^{}]*)\}/)?.[1] ?? "";
  assert.ok(selected.includes("mounted.current") && selected.includes("safety.isBusy()"));
  assert.equal([...selected.matchAll(/choose\(id\)/g)].length, 1);
  assert.match(compact(selected), /(?:if\(!mounted\.current\|\|safety\.isBusy\(\)\)return;choose\(id\);|if\(mounted\.current&&!safety\.isBusy\(\)\)choose\(id\);)/);
  assert.ok(compact(controller).includes("selectedRef.current=id;setSelectedId(id)"));
});

test("view-controller extraction remains local and preserves actual browser test entry points", async () => {
  for (const path of [viewPath, controllerPath]) assert.doesNotMatch(codeOnly(await read(path)),
    /\b(?:fetch|WebSocket|EventSource|localStorage|sessionStorage|indexedDB)\b|from\s*["'][^"']*(?:sdk|\/api)[^"']*["']|\/platform\/v1\//, path);
  const controller = codeOnly(await read(controllerPath));
  assert.doesNotMatch(controller, /useI18n|HTMLInputElement|FileList|<TextPayloadEditor|data-testid/);
  const packageJson = JSON.parse(await read("frontend/package.json"));
  for (const path of ["taskPayloadEditor.browser.mjs", "taskTextFiles.browser.mjs"])
    assert.ok(packageJson.scripts["test:task-payload-browser"].includes(`tests/${path}`), path);
  assert.ok(packageJson.scripts["test:task-text-import-browser"].includes("tests/taskTextFiles.browser.mjs"));
  const browser = await read("frontend/tests/taskPayloadEditor.browser.mjs");
  for (const phrase of ["confirmation", "approve", "setupTask", "assertLocalOnly"]) assert.ok(browser.includes(phrase), phrase);
});
