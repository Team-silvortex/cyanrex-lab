// Pure supplied-value planning only; these tests establish no HTTP dispatch or commit provenance.
import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";
import { readFile } from "node:fs/promises";
import {
  allocations, artifactResponse, assertFrozen, assertProgress, baseline, clone, draft, guard,
  item, prepare, reference, taskResponse, updated, uuid,
} from "./helpers/taskContentSaveFixtures.mjs";

const resolver = registerHooks({ resolve(specifier, context, nextResolve) {
  if (context.parentURL?.includes("/src/features/tasks/")
    && specifier.startsWith("./") && !/\.[a-z]+$/i.test(specifier)) {
    return nextResolve(new URL(`${specifier}.ts`, context.parentURL).href, context);
  }
  return nextResolve(specifier, context);
} });
let api;
try { api = await import("../src/features/tasks/taskContentSave.ts"); }
finally { resolver.deregister(); }
const { prepareTaskBoundContentSave, prepareTaskContentSave } = api;
const create = options => prepare(prepareTaskBoundContentSave, options);
function receipt(step) {
  const result = artifactResponse({ body: { ...step.body, title: step.body.expected.task.title } });
  result.artifact.owner = clone(step.body.expected.task.owner);
  result.artifact.reference.workspace = clone(step.body.expected.task.reference.workspace);
  return result;
}
const pendingPublish = (f, index = 0) => ({ kind: "publish", index, ...f.allocations[index] });

test("Task-bound publication is an explicit factory and does not silently change the old unbound factory", async () => {
  assert.equal(typeof prepareTaskBoundContentSave, "function");
  const old = await prepare(prepareTaskContentSave), legacy = old.attempt.next(old.guard);
  assert.deepEqual(legacy, { kind: "publish", index: 0,
    body: { schema_version: 1, ...old.allocations[0], title: old.draft.title, text: old.draft.payload[0].text } });
  assert.ok(!Object.hasOwn(legacy, "task_id")); assert.ok(!Object.hasOwn(legacy.body, "expected"));
  const f = await create(), step = f.attempt.next(f.guard);
  assert.deepEqual(step, { kind: "publish", index: 0, task_id: f.baseline.task.reference.task_id,
    body: { schema_version: 1, expected: f.baseline, ...f.allocations[0], text: f.draft.payload[0].text } });
  assert.deepEqual(Object.keys(step.body).sort(), ["artifact_id", "expected", "revision_id", "schema_version", "text"]);
  assert.ok(!Object.hasOwn(step.body, "title")); assert.ok(!Object.hasOwn(step.body, "task_id"));
  assertFrozen(step); assertProgress(f.attempt, { state: "pending", pending: pendingPublish(f) });
});

test("renaming a Task publishes under the original title and changes the title only in the separate PUT", async () => {
  const f = await create(), published = f.attempt.next(f.guard);
  assert.notEqual(f.draft.title, f.baseline.task.title);
  const acknowledged = receipt(published);
  assert.equal(acknowledged.artifact.title, f.baseline.task.title);
  assert.equal(f.attempt.confirm(published, acknowledged, f.guard), true);
  assertProgress(f.attempt, { confirmedArtifacts: 1 });
  const replacement = f.attempt.next(f.guard);
  assert.deepEqual(replacement, { kind: "replace", task_id: f.baseline.task.reference.task_id,
    body: { schema_version: 1, expected_revision: f.baseline.task.revision,
      manifest: { schema_version: 1, title: f.draft.title, payload: [{ kind: "text",
        filename: f.draft.payload[0].filename, language: f.draft.payload[0].language,
        artifact: acknowledged.artifact.reference }] } } });
  assert.equal(f.attempt.confirm(replacement, taskResponse(replacement, f.baseline), f.guard), true);
  assertProgress(f.attempt, { state: "complete", confirmedArtifacts: 1, taskConfirmed: true });
});

test("shared Rust Task-bound fixture produces the exact POST pin and retains a separate renamed PUT", async () => {
  const fixture = JSON.parse(await readFile(new URL(
    "../../engine/tests/fixtures/task-bound-content-save.json", import.meta.url), "utf8"));
  const f = await create({ draft: fixture.draft, baseline: fixture.baseline,
    guard: fixture.guard, allocations: fixture.allocations });
  assert.ok(f.draft.payload.length > 0);
  assert.notEqual(f.baseline.task.title, f.draft.title);
  const first = f.attempt.next(f.guard);
  assert.equal(first.kind, "publish"); assert.equal(first.index, 0);
  assert.equal(first.task_id, f.baseline.task.reference.task_id);
  assert.deepEqual(first.body, fixture.publication_body); assertFrozen(first);
  const acknowledged = receipt(first);
  assert.equal(acknowledged.artifact.title, f.baseline.task.title);
  assert.notEqual(acknowledged.artifact.title, f.draft.title);
  assert.equal(f.attempt.confirm(first, acknowledged, f.guard), true);
  for (let index = 1; index < f.draft.payload.length; index++) {
    const step = f.attempt.next(f.guard);
    assert.equal(step.index, index); assert.deepEqual(step.body.expected, f.baseline);
    assert.equal(f.attempt.confirm(step, receipt(step), f.guard), true);
  }
  assert.equal(f.attempt.progress().taskConfirmed, false);
  const replacement = f.attempt.next(f.guard);
  assert.equal(replacement.kind, "replace"); assert.equal(replacement.body.manifest.title, f.draft.title);
  assert.equal(replacement.body.expected_revision, f.baseline.task.revision);
  assert.equal(f.attempt.confirm(replacement, taskResponse(replacement, f.baseline), f.guard), true);
  assertProgress(f.attempt, { state: "complete", confirmedArtifacts: f.draft.payload.length, taskConfirmed: true });
});

test("two plain notes retain one complete original baseline throughout independent Artifact confirmations", async () => {
  const source = draft([item({ id: "note-a", filename: "讨论.md", language: "markdown", text: "讨论记录" }),
    item({ id: "note-b", filename: "讨论.md", language: "future-notes+v9", text: "不需要代码或执行" })]);
  const binding = { kind: "text", filename: "previous.txt", language: "plaintext",
    artifact: reference({ artifact_id: uuid(501), revision_id: uuid(601) }, "previous content") };
  const original = baseline([binding]), f = await create({ draft: source, baseline: original }), pins = [];
  for (let index = 0; index < 2; index++) {
    const step = f.attempt.next(f.guard);
    assert.equal(step.kind, "publish"); assert.equal(step.index, index);
    assert.equal(step.task_id, original.task.reference.task_id);
    assert.deepEqual(step.body.expected, original); assertFrozen(step.body.expected);
    assert.equal(step.body.expected.task.revision, 5);
    const acknowledged = receipt(step); pins.push(acknowledged.artifact.reference);
    assert.equal(f.attempt.confirm(step, acknowledged, f.guard), true);
    assertProgress(f.attempt, { confirmedArtifacts: index + 1 });
  }
  const replacement = f.attempt.next(f.guard);
  assert.equal(replacement.body.expected_revision, 5);
  assert.deepEqual(replacement.body.manifest.payload.map(value => value.artifact), pins);
  assert.deepEqual(replacement.body.manifest.payload.map(({ filename, language }) => ({ filename, language })),
    source.payload.map(({ filename, language }) => ({ filename, language })));
});

test("named empty text, CRLF, multilingual bytes and future language hints remain ordinary payload", async () => {
  const source = draft([item({ id: "empty", text: "", filename: "empty.txt", language: "future-lang-v9" }),
    item({ id: "notes", text: "合作\r\n한국어 café 😀\r\n", filename: "자료.txt", language: "plaintext" })]);
  const f = await create({ draft: source });
  for (const [index, payload] of source.payload.entries()) {
    const step = f.attempt.next(f.guard);
    assert.equal(step.body.text, payload.text); assert.equal(step.index, index);
    assert.equal(f.attempt.confirm(step, receipt(step), f.guard), true);
  }
  const replacement = f.attempt.next(f.guard);
  assert.deepEqual(replacement.body.manifest.payload.map(({ language }) => language), ["future-lang-v9", "plaintext"]);
  assert.equal(f.attempt.confirm(replacement, taskResponse(replacement), f.guard), true);
});

test("all caller inputs are owned and deeply frozen before the first asynchronous digest", async () => {
  const binding = { kind: "text", filename: "old.txt", language: "plaintext",
    artifact: reference({ artifact_id: uuid(501), revision_id: uuid(601) }, "old") };
  const source = draft(), original = baseline([binding]), selected = guard(), targets = allocations(1);
  const expected = clone(original), wantedDraft = clone(source), wantedTargets = clone(targets);
  const promise = prepareTaskBoundContentSave(source, original, selected, targets);
  source.title = "caller changed title"; source.payload[0].text = "caller changed bytes";
  source.payload[0].filename = "changed.txt"; source.payload[0].language = "rust";
  original.task.reference.task_id = uuid(999); original.task.reference.workspace.workspace_id = uuid(999);
  original.task.owner.principal_id = uuid(999); original.task.title = "caller changed baseline";
  original.task.revision = 999; original.task.created_at = updated; original.task.updated_at = updated;
  original.task.input_refs[0].sha256 = "0".repeat(64); original.manifest.title = "caller changed manifest";
  original.manifest.payload[0].filename = "different.txt";
  original.manifest.payload[0].artifact.sha256 = "1".repeat(64);
  selected.contextEpoch = 999; targets[0].artifact_id = uuid(999);
  const attempt = await promise, step = attempt.next(guard());
  assert.deepEqual(step.body.expected, expected); assertFrozen(step);
  assert.equal(step.task_id, expected.task.reference.task_id);
  assert.equal(step.body.text, wantedDraft.payload[0].text);
  assert.equal(step.body.artifact_id, wantedTargets[0].artifact_id);
  assert.throws(() => { step.body.expected.task.owner.principal_id = uuid(9); }, TypeError);
  assert.throws(() => { step.body.expected.manifest.payload[0].artifact.sha256 = "0".repeat(64); }, TypeError);
  assert.equal(attempt.confirm(step, receipt(step), guard()), true);
  const replacement = attempt.next(guard());
  assert.equal(replacement.body.manifest.title, wantedDraft.title);
  assert.equal(replacement.body.manifest.payload[0].filename, wantedDraft.payload[0].filename);
  assert.equal(replacement.body.expected_revision, expected.task.revision);
});

test("zero payload skips publication and requires the original independent replacement confirmation", async () => {
  const f = await create({ draft: draft([]) }), step = f.attempt.next(f.guard);
  assert.deepEqual(step, { kind: "replace", task_id: f.baseline.task.reference.task_id,
    body: { schema_version: 1, expected_revision: f.baseline.task.revision,
      manifest: { schema_version: 1, title: f.draft.title, payload: [] } } });
  assertProgress(f.attempt, { state: "pending", pending: { kind: "replace",
    task_id: step.task_id, expected_revision: f.baseline.task.revision } });
  assert.equal(f.attempt.confirm(step, taskResponse(step), f.guard), true);
  assertProgress(f.attempt, { state: "complete", taskConfirmed: true });
});

test("local payload IDs and revisions never become allocated server IDs or Task revisions", async () => {
  const source = draft([item({ id: uuid(321), revision: 1234 })]), f = await create({ draft: source });
  const step = f.attempt.next(f.guard);
  assert.equal(step.body.artifact_id, f.allocations[0].artifact_id);
  assert.equal(step.body.revision_id, f.allocations[0].revision_id);
  assert.ok(!JSON.stringify(step).includes(uuid(321)));
  assert.equal(f.attempt.confirm(step, receipt(step), f.guard), true);
  assert.equal(f.attempt.next(f.guard).body.expected_revision, 5);
});

const invalidReceipts = [
  ["draft title instead of baseline title", (response, f) => { response.artifact.title = f.draft.title; }],
  ["different owner", response => { response.artifact.owner.principal_id = uuid(999); }],
  ["different digest", response => { response.artifact.reference.sha256 = "0".repeat(64); }],
  ["different Artifact slot", response => { response.artifact.reference.artifact_id = uuid(999); }],
  ["different revision slot", response => { response.artifact.reference.revision_id = uuid(999); }],
];
for (const [name, change] of invalidReceipts) test(`Task-bound ${name} receipt cannot advance publication`, async () => {
  const f = await create(), step = f.attempt.next(f.guard), acknowledged = receipt(step);
  change(acknowledged, f);
  assert.equal(f.attempt.confirm(step, acknowledged, f.guard), false);
  assertProgress(f.attempt, { state: "stopped", reason: "invalid_confirmation", pending: pendingPublish(f) });
  assert.equal(f.attempt.confirm(step, receipt(step), f.guard), false);
  assert.equal(f.attempt.next(f.guard), null);
});

test("later receipts cannot replace a confirmed prefix with a different owner or an earlier slot", async () => {
  for (const fault of ["owner", "slot"]) {
    const f = await create({ draft: draft([item(), item({ id: "other" })]) });
    const first = f.attempt.next(f.guard), accepted = receipt(first);
    assert.equal(f.attempt.confirm(first, accepted, f.guard), true);
    accepted.artifact.reference.artifact_id = uuid(999);
    const second = f.attempt.next(f.guard), acknowledged = receipt(second);
    if (fault === "owner") acknowledged.artifact.owner.principal_id = uuid(999);
    else acknowledged.artifact.reference = receipt(first).artifact.reference;
    assert.equal(f.attempt.confirm(second, acknowledged, f.guard), false);
    assertProgress(f.attempt, { state: "stopped", reason: "invalid_confirmation", confirmedArtifacts: 1,
      pending: pendingPublish(f, 1) });
  }
});

test("step identity and one-shot handoff remain mandatory for the new factory", async () => {
  const f = await create(), step = f.attempt.next(f.guard), acknowledged = receipt(step);
  assert.throws(() => f.attempt.next(f.guard), { message: "taskSave.busy" });
  assert.equal(f.attempt.confirm(clone(step), acknowledged, f.guard), false);
  assert.equal(f.attempt.confirm(step, acknowledged, f.guard), true);
  assert.equal(f.attempt.confirm(step, acknowledged, f.guard), false);
  const replacement = f.attempt.next(f.guard);
  assert.equal(f.attempt.confirm(step, acknowledged, f.guard), false);
  assert.equal(f.attempt.confirm(replacement, taskResponse(replacement), f.guard), true);
  assert.equal(f.attempt.next(f.guard), null);
});

for (const field of ["generation", "revision", "contextEpoch"]) {
  test(`Task-bound ${field} changes before and after handoff stop without adopting a late receipt`, async () => {
    const before = await create(), changed = { ...before.guard, [field]: before.guard[field] + 1 };
    assert.equal(before.attempt.next(changed), null);
    assertProgress(before.attempt, { state: "stopped", reason: "stale" });
    const after = await create(), step = after.attempt.next(after.guard);
    assert.equal(after.attempt.confirm(step, receipt(step), changed), false);
    assertProgress(after.attempt, { state: "stopped", reason: "stale", pending: pendingPublish(after) });
    assert.equal(after.attempt.confirm(step, receipt(step), after.guard), false);
  });
}

for (const reason of ["unconfirmed", "conflict"]) {
  test(`Task-bound POST ${reason} preserves the confirmed prefix and never advances to PUT`, async () => {
    const f = await create({ draft: draft([item(), item({ id: "other" })]) });
    const first = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(first, receipt(first), f.guard), true);
    const second = f.attempt.next(f.guard); f.attempt.stop(reason);
    assertProgress(f.attempt, { state: "stopped", reason, confirmedArtifacts: 1, pending: pendingPublish(f, 1) });
    assert.equal(f.attempt.confirm(second, receipt(second), f.guard), false);
    assert.equal(f.attempt.next(f.guard), null);
  });
  test(`Task-bound PUT ${reason} cannot erase confirmed Artifacts or report Task completion`, async () => {
    const f = await create(), first = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(first, receipt(first), f.guard), true);
    const replacement = f.attempt.next(f.guard); f.attempt.stop(reason);
    assertProgress(f.attempt, { state: "stopped", reason, confirmedArtifacts: 1,
      pending: { kind: "replace", task_id: f.baseline.task.reference.task_id, expected_revision: 5 } });
    assert.equal(f.attempt.confirm(replacement, taskResponse(replacement), f.guard), false);
    assert.equal(f.attempt.next(f.guard), null);
  });
}

test("manual baseline validation remains complete rather than trusting only a Task ID and revision", async () => {
  for (const change of [
    value => { delete value.task.definition; }, value => { value.task.definition = {}; },
    value => { value.task.status = "ready"; }, value => { delete value.task.updated_at; },
    value => { value.task.owner.authority_id = uuid(999); },
    value => { value.task.title = "not the original manifest"; },
    value => { value.task.input_refs = [reference(allocations(1)[0], "not in the manifest")]; },
    value => { value.manifest.payload = [item()]; },
  ]) {
    const original = baseline(); change(original);
    await assert.rejects(() => create({ baseline: original }), { message: "taskSave.invalid" });
  }
});

test("the complete escaped publication envelope is bounded and oversized text fails before hashing", async () => {
  const old = Array.from({ length: 32 }, (_, index) => ({ kind: "text", filename: `old-${index}.txt`,
    language: "future-old+v9", artifact: reference({ artifact_id: uuid(1000 + index), revision_id: uuid(2000 + index) }, "old") }));
  const source = draft([item({ text: "\\".repeat(256 * 1024) })]);
  const f = await create({ draft: source, baseline: baseline(old) }), step = f.attempt.next(f.guard);
  assert.deepEqual(step.body.expected, f.baseline);
  assert.ok(Buffer.byteLength(JSON.stringify(step.body)) <= 2 * 1024 * 1024);
  assert.ok(Buffer.byteLength(JSON.stringify(step.body)) > 512 * 1024, "count escaping and the complete baseline");
  const digest = crypto.subtle.digest; let hashes = 0;
  crypto.subtle.digest = async () => { hashes++; throw Error("oversized text reached hashing"); };
  try { await assert.rejects(() => create({ draft: draft([item({ text: "x".repeat(256 * 1024 + 1) })]) })); }
  finally { crypto.subtle.digest = digest; }
  assert.equal(hashes, 0);
});

test("progress stays frozen and body-free, and pure confirmations access no HTTP, Cookie or storage", async () => {
  const names = ["fetch", "localStorage", "sessionStorage", "document", "WebSocket"];
  const originals = names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  try {
    for (const name of names) Object.defineProperty(globalThis, name, {
      configurable: true, get() { throw Error(`forbidden side effect: ${name}`); },
    });
    const f = await create(), step = f.attempt.next(f.guard), progress = f.attempt.progress();
    assertFrozen(progress);
    for (const key of ["text", "body", "manifest", "expected", "cookie", "token"]) assert.ok(!(key in progress.pending));
    assert.ok(!JSON.stringify(progress).includes(f.draft.payload[0].text));
    assert.equal(f.attempt.confirm(step, receipt(step), f.guard), true);
    const replacement = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(replacement, taskResponse(replacement), f.guard), true);
    assertProgress(f.attempt, { state: "complete", confirmedArtifacts: 1, taskConfirmed: true });
  } finally {
    for (const [name, original] of originals) {
      if (original) Object.defineProperty(globalThis, name, original); else delete globalThis[name];
    }
  }
});
