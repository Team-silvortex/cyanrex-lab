import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";
import { readFile } from "node:fs/promises";
import {
  allocations, artifactResponse, assertFrozen, assertProgress, baseline, clone,
  draft, guard, item, owner, prepare, reference, taskResponse, updated, uuid,
} from "./helpers/taskContentSaveFixtures.mjs";

// Match the bundler's extensionless local TS imports; do not rewrite product source.
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
const { prepareTaskContentSave } = api;
const create = options => prepare(prepareTaskContentSave, options);

test("empty payload plans only existing Task replacement and is not saved before its exact confirmation", async () => {
  const f = await create({ draft: draft([]) });
  assertProgress(f.attempt, {});
  const step = f.attempt.next(f.guard);
  assert.deepEqual(step, { kind: "replace", task_id: uuid(3), body: { schema_version: 1,
    expected_revision: 5, manifest: { schema_version: 1, title: f.draft.title, payload: [] } } });
  assertFrozen(step);
  assertProgress(f.attempt, { state: "pending", pending: { kind: "replace", task_id: uuid(3), expected_revision: 5 } });
  assert.equal(f.attempt.confirm(step, taskResponse(step), f.guard), true);
  assertProgress(f.attempt, { state: "complete", taskConfirmed: true });
  assert.equal(f.attempt.next(f.guard), null);
});

test("ordered non-code payloads publish separately despite equal bytes or duplicate filenames", async () => {
  const source = draft([item({ id: "first", filename: "讲义.txt", language: "future-lang+v2" }),
    item({ id: "second", filename: "讲义.txt", language: "markdown" })]);
  const f = await create({ draft: source });
  const bindings = [];
  for (let index = 0; index < source.payload.length; index++) {
    const step = f.attempt.next(f.guard);
    assert.deepEqual(step, { kind: "publish", index, body: { schema_version: 1,
      ...f.allocations[index], title: source.title, text: source.payload[index].text } });
    assertFrozen(step);
    assertProgress(f.attempt, { state: "pending", confirmedArtifacts: index,
      pending: { kind: "publish", index, ...f.allocations[index] } });
    assert.throws(() => f.attempt.next(f.guard), { message: "taskSave.busy" });
    const response = artifactResponse(step);
    assert.equal(f.attempt.confirm(step, response, f.guard), true);
    assertProgress(f.attempt, { confirmedArtifacts: index + 1 });
    bindings.push({ kind: "text", filename: source.payload[index].filename,
      language: source.payload[index].language, artifact: response.artifact.reference });
  }
  const step = f.attempt.next(f.guard);
  assert.deepEqual(step, { kind: "replace", task_id: uuid(3), body: { schema_version: 1,
    expected_revision: 5, manifest: { schema_version: 1, title: source.title, payload: bindings } } });
  assert.equal(f.attempt.confirm(step, taskResponse(step), f.guard), true);
  assertProgress(f.attempt, { state: "complete", confirmedArtifacts: 2, taskConfirmed: true });
});

for (const filename of ["named-empty.json", "multilingual.json", "future-language.json"]) {
  test(`shared Rust publication fixture ${filename} preserves its exact labels and text`, async () => {
    const source = JSON.parse(await readFile(new URL(
      `../../engine/tests/fixtures/task-draft-publication/${filename}`, import.meta.url), "utf8"));
    const f = await create({ draft: source });
    for (let index = 0; index < source.payload.length; index++) {
      const step = f.attempt.next(f.guard);
      assert.equal(step.kind, "publish"); assert.equal(step.index, index);
      assert.equal(step.body.title, source.title); assert.equal(step.body.text, source.payload[index].text);
      assert.equal(f.attempt.confirm(step, artifactResponse(step), f.guard), true);
    }
    const step = f.attempt.next(f.guard);
    assert.deepEqual(step.body.manifest.payload.map(({ filename, language }) => ({ filename, language })),
      source.payload.map(({ filename, language }) => ({ filename, language })));
    assert.equal(f.attempt.confirm(step, taskResponse(step), f.guard), true);
    assert.equal(f.attempt.progress().taskConfirmed, true);
  });
}

test("factory snapshots every caller input before asynchronous SHA-256 work", async () => {
  const source = draft(), original = baseline(), current = guard(), targets = allocations(1);
  const promise = prepareTaskContentSave(source, original, current, targets);
  source.title = "mutated title"; source.payload[0].text = "mutated content";
  source.payload[0].filename = "mutated.txt"; source.payload.push(item({ id: "extra" }));
  original.task.revision = 999; original.task.reference.task_id = uuid(999);
  original.manifest.title = "mutated baseline"; current.contextEpoch = 999;
  targets[0].artifact_id = uuid(999); targets.push(allocations(2)[1]);
  const attempt = await promise;
  const step = attempt.next(guard());
  assert.deepEqual(step.body, { schema_version: 1, ...allocations(1)[0],
    title: draft().title, text: item().text });
  assert.equal(attempt.confirm(step, artifactResponse(step), guard()), true);
  const replacement = attempt.next(guard());
  assert.equal(replacement.task_id, uuid(3)); assert.equal(replacement.body.expected_revision, 5);
  assert.equal(replacement.body.manifest.payload[0].filename, item().filename);
  assert.equal(attempt.confirm(replacement, taskResponse(replacement), guard()), true);
});

test("local editing identities never become server identities or expected revisions", async () => {
  const f = await create({ draft: draft([item({ id: uuid(321), revision: 1234 })]) });
  const step = f.attempt.next(f.guard);
  assert.equal(step.body.artifact_id, f.allocations[0].artifact_id);
  assert.equal(step.body.revision_id, f.allocations[0].revision_id);
  assert.ok(!JSON.stringify(step).includes(uuid(321)));
  assert.equal(f.attempt.confirm(step, artifactResponse(step), f.guard), true);
  assert.equal(f.attempt.next(f.guard).body.expected_revision, 5);
});

test("an already-confirmed empty revision-one baseline still requires separate content replacement", async () => {
  const original = baseline(); original.task.revision = 1;
  const f = await create({ baseline: original });
  assertProgress(f.attempt, {});
  const published = f.attempt.next(f.guard);
  assert.equal(f.attempt.confirm(published, artifactResponse(published), f.guard), true);
  assert.equal(f.attempt.progress().taskConfirmed, false);
  const step = f.attempt.next(f.guard);
  assert.equal(step.kind, "replace"); assert.equal(step.body.expected_revision, 1);
  assert.equal(f.attempt.confirm(step, taskResponse(step, original), f.guard), true);
});

test("all 32 payload items may be plain empty notes with bounded unknown language hints", async () => {
  const source = draft(Array.from({ length: 32 }, (_, index) => item({ id: `i${index}`,
    text: "", language: "future+v3", filename: "notes.txt" })), { title: "é".repeat(128) });
  const f = await create({ draft: source });
  for (let index = 0; index < 32; index++) {
    const step = f.attempt.next(f.guard);
    assert.equal(step.index, index); assert.equal(step.body.text, "");
    assert.equal(f.attempt.confirm(step, artifactResponse(step), f.guard), true);
  }
  const replacement = f.attempt.next(f.guard);
  assert.equal(replacement.body.manifest.payload.length, 32);
  assert.equal(f.attempt.confirm(replacement, taskResponse(replacement), f.guard), true);
});

test("accepted mutable response objects cannot later rewrite the confirmed Artifact or Task", async () => {
  const f = await create(), published = f.attempt.next(f.guard), response = artifactResponse(published);
  assert.equal(f.attempt.confirm(published, response, f.guard), true);
  response.artifact.reference.artifact_id = uuid(999); response.artifact.owner.principal_id = uuid(999);
  const step = f.attempt.next(f.guard);
  assert.equal(step.body.manifest.payload[0].artifact.artifact_id, f.allocations[0].artifact_id);
  const saved = taskResponse(step);
  assert.equal(f.attempt.confirm(step, saved, f.guard), true);
  saved.task.revision = 100; saved.manifest.title = "changed outside";
  assertProgress(f.attempt, { state: "complete", confirmedArtifacts: 1, taskConfirmed: true });
});

test("factory rejects invalid unfinished draft values without allocating a writable attempt", async () => {
  const sources = [draft([], { title: "" }), draft([], { title: " \n\t" }),
    draft([], { title: "é".repeat(129) }), draft([item({ text: "\0" })]),
    draft([item({ text: "\ud800" })]), draft([item({ language: "Rust" })]),
    draft([item({ filename: "../notes.txt" })]), draft([item(), item()]),
    draft([item({ revision: Number.MAX_SAFE_INTEGER + 1 })]), draft([item({ kind: "code" })]),
    draft(Array.from({ length: 33 }, (_, index) => item({ id: `i${index}` })))];
  for (const source of sources) await assert.rejects(() => create({ draft: source }));
});

test("payload size uses UTF-8 bytes and the whole portable JSON includes escaping overhead", async () => {
  const exact = draft([item({ text: "😀".repeat(65536) })]);
  const f = await create({ draft: exact });
  assert.equal(f.attempt.next(f.guard).body.text, exact.payload[0].text);
  await assert.rejects(() => create({ draft: draft([item({ text: exact.payload[0].text + "x" })]) }));
  const escaped = draft(Array.from({ length: 17 }, (_, index) => item({ id: `i${index}`, text: "\\".repeat(262144) })));
  assert.ok(Buffer.byteLength(JSON.stringify(escaped)) > 8 * 1024 * 1024);
  await assert.rejects(() => create({ draft: escaped }));
});

test("baseline is an exact manual Draft snapshot, not a claimed owner or a new-Task request", async () => {
  const mutations = [
    value => { delete value.task; }, value => { value.schema_version = 1; },
    value => { value.task.definition_ref = null; }, value => { delete value.task.definition; },
    value => { value.task.definition = {}; }, value => { value.task.status = "ready"; },
    value => { value.task.schema_version = 2; }, value => { value.manifest.schema_version = 2; },
    value => { value.task.reference.task_id = "00000000-0000-0000-0000-000000000000"; },
    value => { value.task.reference.workspace.workspace_id = uuid(0xabcd).toUpperCase(); },
    value => { value.task.owner.authority_id = uuid(9); },
    value => { value.task.title = "not the manifest title"; },
    value => { value.task.revision = 0; }, value => { value.task.revision = Number.MAX_SAFE_INTEGER; },
    value => { value.task.revision = "5"; }, value => { value.task.revision = 1.5; },
    value => { value.task.input_refs = [reference(allocations(1)[0], "old")]; },
    value => { value.task.created_at = "not a timestamp"; },
    value => { value.task.updated_at = "2026-02-30T00:00:00Z"; },
    value => { value.task.created_at = "2026-10-26"; },
    value => { value.task.reference.extra = "hidden route"; },
  ];
  for (const change of mutations) {
    const original = baseline(); change(original);
    await assert.rejects(() => create({ baseline: original }), { message: "taskSave.invalid" });
  }
  for (const value of [null, [], { manifest: baseline().manifest }, { task: baseline().task }]) {
    await assert.rejects(() => create({ baseline: value }), { message: "taskSave.invalid" });
  }
});

test("existing baseline payload bindings must match ordered input_refs and the exact workspace", async () => {
  const binding = { kind: "text", filename: "old.md", language: "markdown",
    artifact: reference(allocations(1)[0], "old notes") };
  const original = baseline([binding]);
  const f = await create({ draft: draft([]), baseline: original });
  assert.equal(f.attempt.next(f.guard).body.expected_revision, 5);
  for (const change of [
    value => { value.manifest.payload[0].artifact.sha256 = "A".repeat(64); },
    value => { value.manifest.payload[0].artifact.workspace.workspace_id = uuid(9);
      value.task.input_refs = [clone(value.manifest.payload[0].artifact)]; },
    value => { value.manifest.payload.push(clone(value.manifest.payload[0]));
      value.task.input_refs.push(clone(value.task.input_refs[0])); },
    value => { value.manifest.payload[0].filename = "."; },
  ]) {
    const bad = clone(original); change(bad);
    await assert.rejects(() => create({ baseline: bad }), { message: "taskSave.invalid" });
  }
});

test("guard and allocation envelopes reject omissions, extra authority fields, invalid numbers and alias IDs", async () => {
  for (const field of ["generation", "revision", "contextEpoch"]) {
    for (const value of [-1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, "1", null]) {
      await assert.rejects(() => create({ guard: guard({ [field]: value }) }), { message: "taskSave.invalid" });
    }
    const incomplete = guard(); delete incomplete[field];
    await assert.rejects(() => create({ guard: incomplete }), { message: "taskSave.invalid" });
  }
  await assert.rejects(() => create({ guard: guard({ owner: owner() }) }), { message: "taskSave.invalid" });
  const f = await create({ guard: guard({ generation: 0, revision: 0, contextEpoch: 0 }) });
  assert.equal(f.attempt.next(f.guard).kind, "publish");
  for (const targets of [[], allocations(2), [{ ...allocations(1)[0], owner: owner() }],
    [{ artifact_id: uuid(1) }], [{ artifact_id: "local_only", revision_id: uuid(2) }],
    [{ artifact_id: uuid(0xabcd).toUpperCase(), revision_id: uuid(2) }],
    [{ artifact_id: "00000000-0000-0000-0000-000000000000", revision_id: uuid(2) }]]) {
    await assert.rejects(() => create({ allocations: targets }), { message: "taskSave.invalid" });
  }
  for (const key of ["artifact_id", "revision_id"]) {
    const targets = allocations(2); targets[1][key] = targets[0][key];
    await assert.rejects(() => create({ draft: draft([item(), item({ id: "other" })]), allocations: targets }),
      { message: "taskSave.invalid" });
  }
});

test("strict snapshots reject custom prototypes, accessors, symbols and array extras without invoking user code", async () => {
  let touched = 0;
  const accessor = baseline();
  Object.defineProperty(accessor.task, "title", { enumerable: true, get() { touched++; return "Original collaboration task"; } });
  const inherited = Object.assign(Object.create({ invisible: true }), baseline());
  const symbolic = baseline(); symbolic[Symbol("private")] = true;
  const arrayExtra = allocations(1); arrayExtra.extra = true;
  for (const options of [{ baseline: accessor }, { baseline: inherited }, { baseline: symbolic },
    { allocations: arrayExtra }]) await assert.rejects(() => create(options));
  assert.equal(touched, 0);
});

test("publish receipt validation rejects every mismatched field and leaves its target unconfirmed", async () => {
  const mutations = [
    value => { value.schema_version = 2; }, value => { value.text = "unexpected disclosure"; },
    value => { value.artifact.schema_version = 2; },
    value => { value.artifact.reference.artifact_id = uuid(999); },
    value => { value.artifact.reference.revision_id = uuid(999); },
    value => { value.artifact.reference.workspace.workspace_id = uuid(999); },
    value => { value.artifact.reference.sha256 = "0".repeat(64); },
    value => { value.artifact.owner.principal_id = uuid(999); },
    value => { value.artifact.owner.authority_id = uuid(999); },
    value => { value.artifact.sequence = 2; }, value => { value.artifact.parent = clone(value.artifact.reference); },
    value => { value.artifact.kind = "example.other"; }, value => { value.artifact.media_type = "application/json"; },
    value => { value.artifact.title = "A filename is not this task title"; },
    value => { value.artifact.byte_length++; }, value => { value.artifact.byte_length = item().text.length; },
    value => { value.artifact.created_at = "2026-02-30T00:00:00Z"; },
    value => { value.artifact.reference.extra = true; }, value => { delete value.artifact.parent; },
  ];
  for (const change of mutations) {
    const f = await create(), step = f.attempt.next(f.guard), response = artifactResponse(step);
    change(response);
    assert.equal(f.attempt.confirm(step, response, f.guard), false);
    assertProgress(f.attempt, { state: "stopped", reason: "invalid_confirmation",
      pending: { kind: "publish", index: 0, ...f.allocations[0] } });
    assert.equal(f.attempt.next(f.guard), null);
    assert.equal(f.attempt.confirm(step, artifactResponse(step), f.guard), false);
  }
});

test("later Artifact receipts cannot change owner or satisfy a different allocated slot", async () => {
  for (const fault of ["owner", "slot"]) {
    const f = await create({ draft: draft([item(), item({ id: "other" })]) });
    const first = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(first, artifactResponse(first), f.guard), true);
    const second = f.attempt.next(f.guard), response = artifactResponse(second);
    if (fault === "owner") response.artifact.owner.principal_id = uuid(9);
    else response.artifact.reference = artifactResponse(first).artifact.reference;
    assert.equal(f.attempt.confirm(second, response, f.guard), false);
    assert.equal(f.attempt.progress().confirmedArtifacts, 1);
    assert.equal(f.attempt.progress().reason, "invalid_confirmation");
  }
});

test("wrong step objects, cloned steps and repeated receipts cannot consume the pending slot", async () => {
  const f = await create(), step = f.attempt.next(f.guard), response = artifactResponse(step);
  assert.equal(f.attempt.confirm(clone(step), response, f.guard), false);
  assert.equal(f.attempt.confirm({}, response, f.guard), false);
  assertProgress(f.attempt, { state: "pending", pending: { kind: "publish", index: 0, ...f.allocations[0] } });
  assert.equal(f.attempt.confirm(step, response, f.guard), true);
  assert.equal(f.attempt.confirm(step, response, f.guard), false);
  const replacement = f.attempt.next(f.guard);
  assert.equal(f.attempt.confirm(step, response, f.guard), false);
  assert.equal(f.attempt.confirm(replacement, taskResponse(replacement), f.guard), true);
});

test("replacement receipt requires the exact manual Draft, revision, original creation time and manifest", async () => {
  const mutations = [
    value => { value.schema_version = 2; }, value => { value.contents = []; },
    value => { value.task.schema_version = 2; }, value => { value.task.definition = {}; },
    value => { value.task.definition_ref = null; }, value => { value.task.status = "ready"; },
    value => { value.task.reference.task_id = uuid(999); },
    value => { value.task.reference.workspace.workspace_id = uuid(999); },
    value => { value.task.owner.principal_id = uuid(999); },
    value => { value.task.revision = 5; }, value => { value.task.revision = 7; },
    value => { value.task.created_at = updated; }, value => { value.task.updated_at = "invalid"; },
    value => { value.task.title += "!"; }, value => { value.manifest.title += "!"; },
    value => { value.task.input_refs = []; }, value => { value.manifest.payload = []; },
  ];
  for (const change of mutations) {
    const f = await create(), published = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(published, artifactResponse(published), f.guard), true);
    const step = f.attempt.next(f.guard), response = taskResponse(step); change(response);
    assert.equal(f.attempt.confirm(step, response, f.guard), false);
    assertProgress(f.attempt, { state: "stopped", reason: "invalid_confirmation", confirmedArtifacts: 1,
      pending: { kind: "replace", task_id: uuid(3), expected_revision: 5 } });
  }
});

test("malformed DTOs are rejected without invoking getters or accepting a fabricated checkpoint", async () => {
  for (const value of [null, [], "confirmed", { reported_state: "complete", reported_confirmed_artifact_count: 1 }]) {
    const f = await create(), step = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(step, value, f.guard), false);
    assert.equal(f.attempt.progress().reason, "invalid_confirmation");
  }
  let touched = 0;
  const f = await create(), step = f.attempt.next(f.guard), response = artifactResponse(step);
  Object.defineProperty(response, "artifact", { enumerable: true, get() { touched++; throw Error("getter ran"); } });
  assert.equal(f.attempt.confirm(step, response, f.guard), false); assert.equal(touched, 0);
});

for (const field of ["generation", "revision", "contextEpoch"]) {
  test(`changing ${field} before handoff stops without manufacturing an unknown write`, async () => {
    const f = await create();
    assert.equal(f.attempt.next({ ...f.guard, [field]: f.guard[field] + 1 }), null);
    assertProgress(f.attempt, { state: "stopped", reason: "stale" });
    assert.equal(f.attempt.next(f.guard), null);
  });
  test(`changing ${field} after handoff preserves an unconfirmed target and ignores late success`, async () => {
    const f = await create(), step = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(step, artifactResponse(step), { ...f.guard, [field]: f.guard[field] + 1 }), false);
    assertProgress(f.attempt, { state: "stopped", reason: "stale",
      pending: { kind: "publish", index: 0, ...f.allocations[0] } });
    assert.equal(f.attempt.confirm(step, artifactResponse(step), f.guard), false);
    assert.equal(f.attempt.next(f.guard), null);
  });
}

test("cancel before handoff is local cancellation, not Task success or dispatched uncertainty", async () => {
  const f = await create({ draft: draft([]) });
  f.attempt.stop("cancelled");
  assertProgress(f.attempt, { state: "stopped", reason: "cancelled" });
  assert.equal(f.attempt.next(f.guard), null);
});

for (const reason of ["cancelled", "conflict", "unconfirmed", "rejected", "stale"]) {
  test(`${reason} preserves the confirmed Artifact prefix and exact unknown next target without retry`, async () => {
    const f = await create({ draft: draft([item(), item({ id: "other" })]) });
    const first = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(first, artifactResponse(first), f.guard), true);
    const second = f.attempt.next(f.guard); f.attempt.stop(reason);
    assertProgress(f.attempt, { state: "stopped", reason, confirmedArtifacts: 1,
      pending: { kind: "publish", index: 1, ...f.allocations[1] } });
    assert.equal(f.attempt.next(f.guard), null);
    assert.equal(f.attempt.confirm(second, artifactResponse(second), f.guard), false);
    assert.equal(f.attempt.progress().taskConfirmed, false);
  });
}

test("unknown Task replacement never erases prior Artifacts or pretends publication saved the Task", async () => {
  const f = await create(), published = f.attempt.next(f.guard);
  assert.equal(f.attempt.confirm(published, artifactResponse(published), f.guard), true);
  assertProgress(f.attempt, { confirmedArtifacts: 1 });
  const step = f.attempt.next(f.guard); f.attempt.stop("unconfirmed");
  assertProgress(f.attempt, { state: "stopped", reason: "unconfirmed", confirmedArtifacts: 1,
    pending: { kind: "replace", task_id: uuid(3), expected_revision: 5 } });
  assert.equal(f.attempt.confirm(step, taskResponse(step), f.guard), false);
  assert.equal(f.attempt.next(f.guard), null);
});

test("progress exposes frozen body-free metadata and cannot be mutated into completion", async () => {
  const f = await create(), step = f.attempt.next(f.guard), progress = f.attempt.progress();
  assertFrozen(progress); assertFrozen(step);
  assert.ok(!JSON.stringify(progress).includes(item().text));
  for (const key of ["body", "text", "token", "cookie", "manifest"]) assert.ok(!(key in progress.pending));
  assert.throws(() => { progress.taskConfirmed = true; }, TypeError);
  assert.throws(() => { progress.pending.artifact_id = uuid(999); }, TypeError);
  assert.throws(() => { step.body.text = "mutate"; }, TypeError);
  assert.equal(f.attempt.progress().taskConfirmed, false);
});

test("planning and confirmations perform no network, Cookie, localStorage or sessionStorage access", async () => {
  const names = ["fetch", "localStorage", "sessionStorage", "document"];
  const descriptors = names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  try {
    for (const name of names) Object.defineProperty(globalThis, name, {
      configurable: true, get() { throw Error(`forbidden side effect: ${name}`); },
    });
    const f = await create(), published = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(published, artifactResponse(published), f.guard), true);
    const step = f.attempt.next(f.guard);
    assert.equal(f.attempt.confirm(step, taskResponse(step), f.guard), true);
  } finally {
    for (const [name, descriptor] of descriptors) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete globalThis[name];
    }
  }
});
