import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const codeOnly = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// Only inspect the selected small method, so an old compatible method or a comment cannot
// satisfy a new boundary. This is a source guard, not a parser or runtime authorization proof.
function body(source, signature) {
  const match = signature.exec(source);
  assert.ok(match, `missing expected method: ${signature}`);
  const start = source.indexOf("{", match.index + match[0].length);
  assert.ok(start >= 0);
  let depth = 1;
  for (let index = start + 1; index < source.length; index++) {
    if (source[index] === "{") depth++;
    if (source[index] === "}" && --depth === 0) return source.slice(start + 1, index);
  }
  assert.fail(`unclosed method: ${signature}`);
}
const rustBody = (source, name) => body(source, new RegExp(`\\bfn\\s+${name}\\b`));

async function sourceFiles(directory, extension) {
  const result = [];
  for (const entry of await readdir(new URL(directory, root), { withFileTypes: true })) {
    const path = `${directory}/${entry.name}`;
    if (entry.isDirectory()) result.push(...await sourceFiles(path, extension));
    else if (extension.test(entry.name)) result.push([path, await read(path)]);
  }
  return result;
}

// These wiring checks complement the pure catalogue, Session SQL and real editor tests.
test("definition metadata stays neutral while the existing catalogue retains typed assessment", async () => {
  const metadata = codeOnly(await read("engine/src/services/task_catalog/definitions.rs"));
  assert.match(metadata, /struct\s+TaskDefinitionCatalog\s*\{/);
  assert.match(metadata, /Arc\s*<\s*\[\s*TaskDefinition\s*\]\s*>/);
  assert.match(rustBody(metadata, "new"), /validate_definition\s*\(/);
  assert.doesNotMatch(metadata, /\b(?:TaskProvider|AssessmentOutcome|TaskAssessment|Deserialize|Serialize|async|await)\b|\b(?:assess|execute|spawn|spawn_blocking)\s*\(|\b(?:tokio|sqlx|reqwest|domain_packs)\s*::|std\s*::\s*(?:fs|env|process|net)\b/);
  const catalog = codeOnly(await read("engine/src/services/task_catalog.rs"));
  assert.match(catalog, /TaskCatalog\s*<\s*P\s*:\s*TaskProvider\s*>/);
  assert.match(catalog, /evidence\s*:\s*P\s*::\s*Evidence\s*</);
  assert.match(rustBody(catalog, "new"), /TaskDefinitionCatalog\s*::\s*new\s*\(/);
  assert.match(rustBody(catalog, "get"), /self\s*\.\s*definitions\s*\.\s*get\s*\(\s*reference\s*\)/);
  // Inspect the typed implementation, not the body's absence in the TaskProvider trait.
  const implementationStart = catalog.search(/\bimpl\s*<\s*P\s*:\s*TaskProvider\s*>/);
  assert.ok(implementationStart >= 0);
  const implementation = catalog.slice(implementationStart);
  assert.match(rustBody(implementation, "assess"), /self\s*\.\s*provider\s*\.\s*assess\s*\(\s*definition\s*,\s*evidence\s*\)/);
});

test("old Task and Session catalogue constructors delegate without weakening exact definition admission", async () => {
  const draft = codeOnly(await read("engine/src/services/task_store/draft.rs"));
  assert.match(rustBody(draft, "from_catalog"), /Self\s*::\s*from_definitions\s*\(\s*catalog\s*\.\s*definition_catalog\s*\(/);
  const admitted = rustBody(draft, "from_definitions");
  assert.match(admitted, /\.\s*get\s*\(\s*reference\s*\)/);
  assert.match(admitted, /TaskStoreError\s*::\s*UnknownDefinition/);
  assert.match(admitted, /Self\s*::\s*from_definition\s*\(/);
  const session = codeOnly(await read("engine/src/services/auth_service/durable_source/task_commands/inputs/catalog.rs"));
  assert.match(rustBody(session, "new"), /Self\s*::\s*from_definitions\s*\(/);
  const composed = rustBody(session, "from_definitions");
  assert.match(composed, /SessionTaskInputsWorkspace\s*::\s*new\s*\(/);
  assert.match(composed, /catalog\s*\.\s*clone\s*\(/);
  assert.doesNotMatch(composed, /\b(?:TaskProvider|assess|validate_session|transaction|query)\b/);
  assert.match(rustBody(session, "definition"), /\.\s*get\s*\(\s*reference\s*\)/);
  assert.match(rustBody(session, "definition"), /TaskStoreError\s*::\s*UnknownDefinition/);
  const required = rustBody(session, "require_task");
  assert.match(required, /UnsupportedTask/);
  assert.match(required, /stored\s*!=\s*self\s*\.\s*definition\s*\(\s*&stored\s*\.\s*reference\s*\)/);
  assert.match(required, /DefinitionMismatch/);
});

test("portable hints stay independent from editor profiles and introduce no server or autosave connection", async () => {
  const draft = codeOnly(await read("frontend/src/features/tasks/taskDraft.ts"));
  const hint = codeOnly(await read("frontend/src/features/tasks/taskPayloadLanguage.ts"));
  assert.match(draft, /isTaskPayloadLanguage\s*\(\s*source\s*\.\s*language\s*\)/);
  for (const source of [draft, hint]) assert.doesNotMatch(source,
    /\b(?:BUILTIN_LANGUAGES|getEditorLanguage|resolveEditorLanguage|monaco)\b|(?:\.\.\/editor|features\/editor)/);
  const languages = codeOnly(await read("frontend/src/features/editor/languages.ts"));
  const resolve = body(languages, /function\s+resolveEditorLanguage\b/);
  assert.match(resolve, /getEditorLanguage\s*\(\s*hint\s*\)\s*\?\?\s*getEditorLanguage\s*\(\s*["']plaintext["']\s*\)/);
  const editor = codeOnly(await read("frontend/src/features/editor/useEditorSession.ts"));
  const synchronize = body(editor, /synchronize\s*\(\s*language\s*,\s*source\s*\)/);
  assert.match(synchronize, /resolveEditorLanguage\s*\(\s*language\s*\)/);
  assert.match(synchronize, /getLanguageId\s*\(\s*\)\s*===\s*profile\s*\.\s*id/);
  assert.doesNotMatch(synchronize, /getLanguageId\s*\(\s*\)\s*===\s*language\b/);
  for (const path of ["frontend/src/features/tasks/taskDraft.ts", "frontend/src/features/tasks/taskPayloadLanguage.ts",
    "frontend/src/features/tasks/TaskDraftWorkspace.tsx", "frontend/src/features/editor/useEditorSession.ts",
    "frontend/src/features/editor/EditorWorkspace.tsx"]) assert.doesNotMatch(codeOnly(await read(path)),
    /\b(?:fetch|WebSocket|EventSource|localStorage|sessionStorage|indexedDB)\b|from\s*["'][^"']*(?:sdk|\/api)[^"']*["']|\/platform\/v1\//, path);
  const publicFiles = await Promise.all(["engine/src/application.rs", "engine/src/state.rs", "engine/openapi/openapi.json"]
    .map(async path => [path, await read(path)]));
  publicFiles.push(...await sourceFiles("engine/src/routes", /\.rs$/), ...await sourceFiles("sdk-js/src", /\.ts$/));
  for (const [path, source] of publicFiles) assert.doesNotMatch(codeOnly(source),
    /\bTaskDefinitionCatalog\b|\/platform\/v1\/task-definitions/, path);
});
