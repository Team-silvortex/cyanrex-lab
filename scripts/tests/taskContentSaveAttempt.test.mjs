// Inventories supplement the executable supplied-value/state-machine tests, not real saving.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";

const read = path => readFile(new URL(`../../${path}`, import.meta.url), "utf8");
const code = value => value.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

test("local content-save preparation adds no transport storage Cookie or teaching dependency", async () => {
  for (const name of ["taskContentSave", "taskSaveContract"]) {
    const source = code(await read(`frontend/src/features/tasks/${name}.ts`));
    assert.doesNotMatch(source, /\b(?:fetch|WebSocket|EventSource|XMLHttpRequest|localStorage|sessionStorage|document|window|process|console)\b/);
    assert.doesNotMatch(source, /getEngineUrl|requestPrivate|AuthService|BUILTIN_LANGUAGES|\.\.\/editor|\.\.\/auth|setTimeout|setInterval/);
  }
  for (const path of ["frontend/src/features/tasks/TaskDraftWorkspace.tsx", "frontend/src/features/tasks/useTaskDraftController.ts",
    "engine/src/main.rs", "engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(code(await read(path)), /prepareTaskContentSave|taskContentSave|taskSaveContract/, path);
});

test("content-save snapshots all inputs before local hashing and exposes no checkpoint restoration", async () => {
  const source = code(await read("frontend/src/features/tasks/taskContentSave.ts"));
  const hash = source.indexOf('await crypto.subtle.digest("SHA-256", bytes)');
  assert.ok(hash > 0);
  for (const value of ["serializeTaskDraft(draft)", "contract.baseline(baseline)", "contract.guard(guard)",
    "contract.list(allocations).map(contract.allocation)"])
    assert.ok(source.indexOf(value) >= 0 && source.indexOf(value) < hash, value);
  assert.match(source, /if \(this\.#state === "pending"\) throw new Error\("taskSave\.busy"\)/);
  assert.match(source, /step !== this\.#step/);
  assert.match(source, /this\.#state = "pending";\s*return this\.#step/);
  assert.match(source, /this\.#state = "stopped"; this\.#reason = "invalid_confirmation"/);
  assert.deepEqual([...source.matchAll(/export (?:async )?function (\w+)\(/g)].map(match => match[1]), ["prepareTaskContentSave"]);
  assert.doesNotMatch(source, /export class|restore|resume|retry|adopt|clearDirty|setDirty/);
  const contract = code(await read("frontend/src/features/tasks/taskSaveContract.ts"));
  for (const value of ["Object.getOwnPropertyDescriptors", "Number.isSafeInteger", 'input.definition !== null',
    'input.status !== "draft"', 'artifact.sequence !== 1', 'artifact.parent !== null', 'artifact.kind !== "cyanrex.task-text"',
    'artifact.media_type !== "text/plain"', "result.task.revision !== original.revision + 1"])
    assert.ok(contract.includes(value), value);
});

test("executable content-save matrix is selected by the frontend gate and CI", async () => {
  const script = JSON.parse(await read("frontend/package.json")).scripts["test:task-content-save"];
  assert.equal(script, "node --experimental-strip-types --test tests/taskContentSave.test.mjs");
  assert.match(await read("scripts/quality-gate.sh"), /npm --prefix "\$PROJECT_ROOT\/frontend" run test:task-content-save/);
  assert.match(await read(".github/workflows/ci.yml"), /npm run test:editor-languages && npm run test:task-content-save/);
  const tests = await read("frontend/tests/taskContentSave.test.mjs");
  for (const value of ["named-empty.json", "multilingual.json", "future-language.json", "contextEpoch", "cancelled",
    "invalid_confirmation", "localStorage", "taskResponse", "artifactResponse"])
    assert.ok(tests.includes(value), value);
});

test("bilingual content-save boundaries stay reachable and do not claim a connected browser", async () => {
  for (const lang of ["en", "zh-CN"]) {
    const path = `docs/${lang}/task-draft-publication.md`, guide = await read(path);
    for (const value of ["prepareTaskContentSave", "taskContentSave.ts", "taskSaveContract.ts", "generation", "contextEpoch",
      "expected_revision", "SHA-256", "test:task-content-save"])
      assert.ok(guide.includes(value), `${lang}: ${value}`);
    assert.equal(guide, await read(`frontend/public/course/${lang}/task-draft-publication.md`));
    await assertDocumentationReference(`docs/${lang}/README.md`, path);
    await assertDocumentationReference(`docs/${lang}/project-status.md`, path);
  }
});
