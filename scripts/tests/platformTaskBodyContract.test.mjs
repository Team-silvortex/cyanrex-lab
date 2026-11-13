import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { prepareBodySave, decodeSaveReceipt, editTextareaText } from "../../engine/src/bin/cyanrex-platform/session_ui/body-contract.js";

const uuid = n => `00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;
const scope = { authority_id: uuid(1), workspace_id: uuid(2) };
const stamp = "2026-10-26T01:02:03.123456789Z";
const clone = value => structuredClone(value);
const digest = text => createHash("sha256").update(text, "utf8").digest("hex");
const invalid = { message: "taskSave.invalid" };
const guard = () => ({ generation: 2, revision: 1, contextEpoch: 3 });
const allocations = count => Array.from({ length: count }, (_, index) => ({
  artifact_id: uuid(300 + index), revision_id: uuid(400 + index),
}));
function snapshot(texts = ["Notes are optional payload.\r\n", "同名笔记 😀\t"]) {
  const payload = texts.map((text, index) => ({ kind: "text", filename: "notes.txt", language: "future-lang+v2",
    artifact: { workspace: { ...scope }, artifact_id: uuid(100 + index), revision_id: uuid(200 + index), sha256: digest(text) } }));
  return { schema_version: 1, task: { schema_version: 1,
    reference: { workspace: { ...scope }, task_id: uuid(3) },
    owner: { authority_id: scope.authority_id, principal_id: uuid(4) }, title: "Discussion", definition: null,
    input_refs: payload.map(item => clone(item.artifact)), status: "draft", revision: 7,
    created_at: stamp, updated_at: stamp },
  manifest: { schema_version: 1, title: "Discussion", payload },
  contents: payload.map((item, index) => ({ artifact: clone(item.artifact), text: texts[index] })) };
}
function edits(value, patch = {}) {
  return { title: "Renamed discussion", items: value.manifest.payload.map(({ filename, language }, index) => ({
    filename, language, text: value.contents[index].text,
  })), ...patch };
}
const prepare = (value, input = edits(value), selected = guard(), targets = allocations(input.items.length)) =>
  prepareBodySave(value, input, selected, targets);
function receipt(step) {
  const original = step.body.expected;
  return { schema_version: 1, artifact: { schema_version: 1,
    reference: { workspace: clone(original.task.reference.workspace), artifact_id: step.body.artifact_id,
      revision_id: step.body.revision_id, sha256: digest(step.body.text) },
    owner: clone(original.task.owner), sequence: 1, parent: null, kind: "cyanrex.task-text",
    title: original.task.title, media_type: "text/plain", byte_length: Buffer.byteLength(step.body.text), created_at: stamp } };
}
function replacement(step, value) {
  return { schema_version: 1, task: { ...clone(value.task), title: step.body.manifest.title,
    input_refs: step.body.manifest.payload.map(item => clone(item.artifact)),
    revision: value.task.revision + 1, updated_at: "2026-10-26T01:02:04.987654321Z" }, manifest: clone(step.body.manifest) };
}
function frozen(value) {
  if (!value || typeof value !== "object") return;
  assert.ok(Object.isFrozen(value));
  for (const child of Object.values(value)) frozen(child);
}
const decoded = value => decodeSaveReceipt(JSON.stringify(value));

test("textarea no-op keeps exact CRLF and mixed original bytes", () => {
  for (const original of ["", "plain 😀", "alpha\r\nbeta\r\n", "alpha\rbeta\ngamma\r\n", "😀\r\n合作\t\r"]) {
    assert.equal(editTextareaText(original, original.replace(/\r\n?/g, "\n")), original);
  }
});

test("textarea inserts and replacements retain CRLF with the original first newline style", () => {
  assert.equal(editTextareaText("alpha\r\nbeta\r\n", "alpha\nnew\nbeta\n"), "alpha\r\nnew\r\nbeta\r\n");
  assert.equal(editTextareaText("alpha\r\nbeta\r\n", "alpha\nBETA\n"), "alpha\r\nBETA\r\n");
  assert.equal(editTextareaText("alpha\r\nbeta\r\n", "alpha\n"), "alpha\r\n");
  assert.equal(editTextareaText("alpha", "alpha\nbeta"), "alpha\nbeta");
  assert.equal(editTextareaText("alpha\rbeta", "alpha\nnew\nbeta"), "alpha\rnew\rbeta");
});

test("textarea edits change only the middle while mixed ending prefixes and suffixes keep their bytes", () => {
  assert.equal(editTextareaText("alpha\r\nbeta\rgamma\ndelta\r\n", "alpha\nbeta\nNEW\ndelta\n"),
    "alpha\r\nbeta\rNEW\ndelta\r\n");
  assert.equal(editTextareaText("alpha\rbeta\ngamma\r\n", "alpha\nbeta\nnew\ngamma\n"),
    "alpha\rbeta\nnew\rgamma\r\n");
  assert.equal(editTextareaText("alpha\nbeta\r\n", "new\nalpha\nbeta\n"), "new\nalpha\nbeta\r\n");
});

test("textarea normalized offsets preserve supplementary Unicode and insert exact edited characters", () => {
  assert.equal(editTextareaText("😀 café\r\n合作\r\n끝", "😁 café\n合作\n끝"), "😁 café\r\n合作\r\n끝");
  assert.equal(editTextareaText("😀 café\r\n合作\r\n끝", "😀 café\n合作 新\n끝"), "😀 café\r\n合作 新\r\n끝");
  assert.equal(editTextareaText("合作\r\n😀\r\n끝", "合作\n🎉\n끝"), "合作\r\n🎉\r\n끝");
});

test("fixed slots publish all edited bytes in order, then separately confirm the renamed replacement", async () => {
  const value = snapshot(), input = edits(value);
  input.items[0] = { filename: "同名.md", language: "next-notes+v9", text: "讨论\r\n한국어 café 😀\t\r\n" };
  input.items[1] = { filename: "同名.md", language: "plaintext", text: "" };
  const attempt = await prepare(value, input), pins = [];
  for (const [index, item] of input.items.entries()) {
    const step = attempt.next(guard());
    assert.deepEqual(step, { kind: "publish", index, task_id: value.task.reference.task_id,
      body: { schema_version: 1, expected: { task: value.task, manifest: value.manifest },
        ...allocations(input.items.length)[index], text: item.text } });
    assert.ok(!JSON.stringify(step).includes('"contents"'));
    frozen(step);
    const acknowledged = decoded(receipt(step)); pins.push(acknowledged.artifact.reference);
    assert.equal(acknowledged.artifact.title, value.task.title);
    assert.equal(attempt.confirm(step, acknowledged, guard()), true);
    assert.equal(attempt.progress().taskConfirmed, false);
  }
  const step = attempt.next(guard());
  assert.deepEqual(step.body, { schema_version: 1, expected_revision: 7,
    manifest: { schema_version: 1, title: input.title, payload: input.items.map((item, index) => ({
      kind: "text", filename: item.filename, language: item.language, artifact: pins[index],
    })) } });
  assert.equal(attempt.confirm(step, decoded(replacement(step, value)), guard()), true);
  assert.deepEqual(attempt.progress(), { state: "complete", confirmedArtifacts: 2, taskConfirmed: true, pending: null, reason: null });
  assert.equal(attempt.next(guard()), null);
});

test("empty manual Drafts retain the planner's independently confirmed title-only replacement", async () => {
  const value = snapshot([]), attempt = await prepare(value), step = attempt.next(guard());
  assert.equal(step.kind, "replace"); assert.equal(step.body.expected_revision, 7);
  assert.deepEqual(step.body.manifest.payload, []);
  assert.equal(attempt.progress().taskConfirmed, false);
  assert.equal(attempt.confirm(step, decoded(replacement(step, value)), guard()), true);
  assert.equal(attempt.progress().taskConfirmed, true);
});

test("full caller inputs are captured before the first asynchronous digest without freezing the caller", async () => {
  const value = snapshot(), input = edits(value), selected = guard(), targets = allocations(2);
  const original = clone(value), wanted = clone(input), wantedTargets = clone(targets);
  const promise = prepare(value, input, selected, targets);
  value.task.reference.task_id = uuid(99); value.task.owner.principal_id = uuid(99);
  value.task.title = "outside"; value.task.revision = 99; value.manifest.title = "outside";
  value.manifest.payload[0].artifact.sha256 = "0".repeat(64); value.contents[0].text = "outside";
  input.title = "outside"; input.items[0].filename = "outside"; input.items[0].language = "rust";
  input.items[0].text = "outside"; input.items.reverse(); selected.contextEpoch = 99;
  targets[0].artifact_id = uuid(99); targets.reverse();
  const attempt = await promise, step = attempt.next(guard());
  assert.deepEqual(step.body.expected, { task: original.task, manifest: original.manifest });
  assert.equal(step.body.text, wanted.items[0].text); assert.equal(step.body.artifact_id, wantedTargets[0].artifact_id);
  assert.ok(!Object.isFrozen(value) && !Object.isFrozen(input) && !Object.isFrozen(targets)); frozen(step);
  assert.equal(attempt.confirm(step, decoded(receipt(step)), guard()), true);
  const next = attempt.next(guard()); assert.equal(next.body.text, wanted.items[1].text);
  assert.equal(attempt.confirm(next, decoded(receipt(next)), guard()), true);
  const last = attempt.next(guard()); assert.equal(last.body.manifest.title, wanted.title);
  assert.deepEqual(last.body.manifest.payload.map(({ filename, language }) => ({ filename, language })),
    wanted.items.map(({ filename, language }) => ({ filename, language })));
});

test("only a complete internally matching manual Draft snapshot can prepare a body save", async () => {
  const mutations = [value => { delete value.contents; }, value => { value.extra = true; },
    value => { value.schema_version = 2; }, value => { value.task.definition = {}; },
    ...["ready", "in_progress", "blocked", "in_review", "cancelled", "accepted"].map(status => value => { value.task.status = status; }),
    value => { value.task.title = "other"; }, value => { value.task.input_refs.reverse(); },
    value => { value.contents.reverse(); }, value => { value.contents.pop(); },
    value => { value.contents[0].extra = true; }, value => { value.contents[0].text = "\0"; },
    value => { value.contents[0].text = "x".repeat(256 * 1024 + 1); },
    value => { value.task.owner.authority_id = uuid(99); },
    value => { value.manifest.payload[0].artifact.workspace.workspace_id = uuid(99); },
    value => { value.task.revision = Number.MAX_SAFE_INTEGER; }, value => { value.task.created_at = "2026-02-30T00:00:00Z"; }];
  for (const change of mutations) {
    const value = snapshot(), input = edits(value); change(value);
    await assert.rejects(() => prepare(value, input), invalid);
  }
});

test("fixed edits exclude adding/removing slots, supplied identities and extra record fields", async () => {
  const value = snapshot(), mutations = [input => { input.items.pop(); },
    input => { input.items.push({ filename: "new.txt", language: "plaintext", text: "new" }); },
    input => { input.items[0].artifact = clone(value.manifest.payload[0].artifact); },
    input => { input.items[0].id = uuid(99); }, input => { input.items[0].revision = 5; },
    input => { input.items[0].kind = "text"; }, input => { input.owner = clone(value.task.owner); },
    input => { delete input.items[0].text; }, input => { delete input.items[0].language; }, input => { delete input.title; }];
  for (const change of mutations) {
    const input = edits(value); change(input);
    await assert.rejects(() => prepare(value, input), invalid);
  }
});

test("the portable draft preserves labels and bounds UTF-8 text, titles, language hints and filenames", async () => {
  const value = snapshot([""]), input = edits(value, { title: "😀".repeat(64) });
  input.items[0] = { filename: "😀".repeat(64), language: "a".repeat(64), text: "😀".repeat(64 * 1024) };
  const attempt = await prepare(value, input), step = attempt.next(guard());
  assert.equal(Buffer.byteLength(step.body.text), 256 * 1024);
  assert.equal(attempt.confirm(step, decoded(receipt(step)), guard()), true);
  assert.equal(attempt.next(guard()).body.manifest.payload[0].filename, input.items[0].filename);
  const mutations = [input => { input.title += "x"; }, input => { input.title = " \t"; },
    input => { input.items[0].text += "x"; }, input => { input.items[0].text = "\ud800"; },
    input => { input.items[0].text = "control\x85"; },
    ...["", ".", "..", "../item", "dir\\item", "line\n", "😀".repeat(64) + "x"].map(filename => input => { input.items[0].filename = filename; }),
    ...["", "Rust", "a".repeat(65), "line\n", "has space", "-first"].map(language => input => { input.items[0].language = language; })];
  for (const change of mutations) {
    const bad = clone(input); change(bad);
    await assert.rejects(() => prepare(value, bad), invalid);
  }
});

test("32 slots are allowed and the entire escaped 8MiB portable draft is bounded before hashing", async () => {
  const value = snapshot(Array(32).fill("")), input = edits(value), attempt = await prepare(value, input);
  assert.equal(attempt.next(guard()).index, 0);
  const tooMany = snapshot(Array(33).fill(""));
  await assert.rejects(() => prepare(tooMany), invalid);
  const original = crypto.subtle.digest; let calls = 0;
  crypto.subtle.digest = async () => { calls++; throw Error("invalid input reached hashing"); };
  try {
    input.items.forEach(item => { item.text = "x".repeat(256 * 1024); });
    await assert.rejects(() => prepare(value, input), invalid);
    input.items.forEach(item => { item.text = "\\".repeat(256 * 1024); });
    await assert.rejects(() => prepare(value, input), invalid);
  } finally { crypto.subtle.digest = original; }
  assert.equal(calls, 0);
});

test("the full portable draft byte budget includes every local slot and permits exactly 8MiB", async () => {
  const value = snapshot(Array(32).fill("")), input = edits(value);
  const portable = () => ({ format: "cyanrex.task-draft", version: 1, title: input.title,
    payload: input.items.map((item, index) => ({ id: `slot-${index}`, revision: 1, kind: "text", ...item })) });
  const overhead = Buffer.byteLength(JSON.stringify(portable()));
  input.items.forEach(item => { item.text = "x".repeat(256 * 1024); });
  input.items.at(-1).text = input.items.at(-1).text.slice(overhead);
  assert.equal(Buffer.byteLength(JSON.stringify(portable())), 8 * 1024 * 1024);
  const attempt = await prepare(value, input); assert.equal(attempt.next(guard()).index, 0);
  input.items.at(-1).text += "x";
  const original = crypto.subtle.digest; let calls = 0;
  crypto.subtle.digest = async () => { calls++; throw Error("one oversized byte reached hashing"); };
  try { await assert.rejects(() => prepare(value, input), invalid); }
  finally { crypto.subtle.digest = original; }
  assert.equal(calls, 0);
});

test("allocations remain independent server UUID slots and are checked by the reused planner", async () => {
  const value = snapshot(), input = edits(value);
  for (const targets of [allocations(1), [allocations(2)[0], allocations(2)[0]],
    [{ artifact_id: "slot-0", revision_id: uuid(400) }, allocations(2)[1]],
    [{ ...allocations(2)[0], extra: true }, allocations(2)[1]]]) {
    await assert.rejects(() => prepare(value, input, guard(), targets), invalid);
  }
});

test("receipt decoding keeps original DTO data deeply frozen and leaves all semantics to planner confirmation", async () => {
  const value = snapshot(["old"]), attempt = await prepare(value), step = attempt.next(guard());
  const acknowledged = receipt(step), result = decoded(acknowledged);
  assert.deepEqual(result, acknowledged); frozen(result);
  assert.equal(attempt.confirm(clone(step), result, guard()), false);
  assert.equal(attempt.confirm(step, result, guard()), true);
  assert.equal(attempt.confirm(step, result, guard()), false);
  const last = attempt.next(guard()), wrong = replacement(last, value); wrong.task.owner.principal_id = uuid(99);
  assert.equal(attempt.confirm(last, decoded(wrong), guard()), false);
  assert.equal(attempt.progress().state, "stopped"); assert.equal(attempt.progress().reason, "invalid_confirmation");
});

test("strict raw receipt decoding rejects duplicate/escaped keys and noncanonical or unsafe number tokens", async () => {
  const value = snapshot(["old"]), attempt = await prepare(value), step = attempt.next(guard());
  const raw = JSON.stringify(receipt(step));
  const bad = [raw.replace('"schema_version":1', '"schema_version":1,"schema_version":1'),
    raw.replace('"title":', '"ti\\u0074le":'), raw.replace('"reference":', '"reference":{},"reference":'),
    ...["9007199254740992", "9007199254740993", "1.0", "1e0", "-0", "01"].map(token => raw.replace('"sequence":1', `"sequence":${token}`)),
    raw + "x"];
  for (const candidate of bad) assert.throws(() => decodeSaveReceipt(candidate), invalid);
  assert.equal(attempt.progress().state, "pending");
  assert.equal(attempt.confirm(step, decoded(receipt(step)), guard()), true);
});

test("receipt byte/depth/token/list limits reject before JSON.parse and include UTF-8 byte accounting", async () => {
  const parse = JSON.parse; let calls = 0;
  JSON.parse = function(...args) { calls++; return parse.apply(this, args); };
  try {
    for (const raw of [" ".repeat(128 * 1024 + 1), '"' + "é".repeat(64 * 1024) + '"',
      "[".repeat(17) + "0" + "]".repeat(17),
      "{" + Array.from({ length: 4097 }, (_, index) => `"x${index}":0`).join(",") + "}",
      "[" + Array(33).fill("0").join(",") + "]"]) assert.throws(() => decodeSaveReceipt(raw), invalid);
    assert.equal(calls, 0);
  } finally { JSON.parse = parse; }
  const value = snapshot(["old"]), attempt = await prepare(value), step = attempt.next(guard());
  const response = receipt(step), raw = JSON.stringify(response);
  assert.deepEqual(decodeSaveReceipt(raw + " ".repeat(128 * 1024 - Buffer.byteLength(raw))), response);
  assert.throws(() => decodeSaveReceipt(raw + " ".repeat(128 * 1024 - Buffer.byteLength(raw)) + " "), invalid);
});

test("publication receipts reject all mismatched pins, bytes, owners and baseline titles", async () => {
  for (const change of [response => { response.artifact.title = "Renamed discussion"; },
    response => { response.artifact.reference.sha256 = "0".repeat(64); },
    response => { response.artifact.reference.artifact_id = uuid(99); },
    response => { response.artifact.reference.revision_id = uuid(99); },
    response => { response.artifact.reference.workspace.workspace_id = uuid(99); },
    response => { response.artifact.owner.principal_id = uuid(99); },
    response => { response.artifact.byte_length++; }, response => { response.artifact.extra = true; },
    response => { response.artifact.created_at = "2026-02-30T00:00:00Z"; }]) {
    const attempt = await prepare(snapshot(["old"])), step = attempt.next(guard()), response = receipt(step);
    change(response);
    assert.equal(attempt.confirm(step, decoded(response), guard()), false);
    assert.equal(attempt.progress().reason, "invalid_confirmation"); assert.equal(attempt.next(guard()), null);
  }
});

test("stale guards and explicit conflicts preserve confirmed prefix without advancing to replacement", async () => {
  for (const field of ["generation", "revision", "contextEpoch"]) {
    const attempt = await prepare(snapshot()), step = attempt.next(guard());
    const changed = { ...guard(), [field]: guard()[field] + 1 };
    assert.equal(attempt.confirm(step, decoded(receipt(step)), changed), false);
    assert.equal(attempt.progress().reason, "stale"); assert.equal(attempt.next(guard()), null);
  }
  const attempt = await prepare(snapshot()), first = attempt.next(guard());
  assert.equal(attempt.confirm(first, decoded(receipt(first)), guard()), true);
  const second = attempt.next(guard()); attempt.stop("conflict");
  assert.equal(attempt.progress().confirmedArtifacts, 1); assert.equal(attempt.progress().taskConfirmed, false);
  assert.equal(attempt.confirm(second, decoded(receipt(second)), guard()), false); assert.equal(attempt.next(guard()), null);
});

test("malformed caller DTOs never execute accessors, iterators or toJSON hooks", async () => {
  let touched = 0;
  const value = snapshot(), input = edits(value);
  Object.defineProperty(input.items[0], "text", { enumerable: true, get() { touched++; return "bytes"; } });
  await assert.rejects(() => prepare(value, input), invalid);
  const other = snapshot(), wanted = edits(other);
  Object.defineProperty(other.contents[0], "text", { enumerable: true, get() { touched++; return "bytes"; } });
  await assert.rejects(() => prepare(other, wanted), invalid);
  const sparse = edits(value); delete sparse.items[0];
  const symbolic = edits(value); symbolic[Symbol("extra")] = true;
  const inherited = Object.assign(Object.create({ trusted: true }), edits(value));
  const custom = edits(value); custom.items.toJSON = () => { touched++; return []; };
  const iterator = edits(value); iterator.items[Symbol.iterator] = () => { touched++; return [][Symbol.iterator](); };
  for (const bad of [sparse, symbolic, inherited, custom, iterator]) await assert.rejects(() => prepare(value, bad), invalid);
  assert.equal(touched, 0);
});

test("pure bridge and receipt confirmation need no network, DOM, credentials or storage", async () => {
  const names = ["fetch", "document", "localStorage", "sessionStorage", "WebSocket"];
  const originals = names.map(name => [name, Object.getOwnPropertyDescriptor(globalThis, name)]);
  try {
    for (const name of names) Object.defineProperty(globalThis, name, {
      configurable: true, get() { throw Error(`forbidden I/O: ${name}`); },
    });
    const value = snapshot(["old"]), attempt = await prepare(value), step = attempt.next(guard());
    frozen(attempt.progress());
    assert.equal(attempt.confirm(step, decoded(receipt(step)), guard()), true);
    const last = attempt.next(guard());
    assert.equal(attempt.confirm(last, decoded(replacement(last, value)), guard()), true);
  } finally {
    for (const [name, descriptor] of originals) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor); else delete globalThis[name];
    }
  }
});
