import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { prepareMetadataSave, parseMetadataSaved } from "../../engine/src/bin/cyanrex-platform/session_ui/task-contract.js";

const uuid = n => `00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;
const scope = { authority_id: uuid(1), workspace_id: uuid(2) };
const stamp = "2026-10-26T01:02:03.123456789Z";
const clone = value => structuredClone(value);
const raw = value => JSON.stringify(value);
const invalid = { message: "taskMetadata.invalid" };
function baseline(texts = ["Notes are optional payload.\n", "同名笔记 😀\t"]) {
  const payload = texts.map((text, index) => ({ kind: "text", filename: "notes.txt", language: "future-lang+v2",
    artifact: { workspace: { ...scope }, artifact_id: uuid(100 + index), revision_id: uuid(200 + index),
      sha256: createHash("sha256").update(text, "utf8").digest("hex") } }));
  return { schema_version: 1, task: { schema_version: 1,
    reference: { workspace: { ...scope }, task_id: uuid(3) },
    owner: { authority_id: scope.authority_id, principal_id: uuid(4) }, title: "Discussion", definition: null,
    input_refs: payload.map(item => clone(item.artifact)), status: "draft", revision: 1,
    created_at: stamp, updated_at: stamp },
  manifest: { schema_version: 1, title: "Discussion", payload },
  contents: payload.map((item, index) => ({ artifact: clone(item.artifact), text: texts[index] })) };
}
function edits(value, patch = {}) {
  return { title: "Renamed discussion", items: value.manifest.payload.map(({ filename, language }) => ({ filename, language })), ...patch };
}
function receipt(value, descriptor) {
  return { schema_version: 1, task: { ...clone(value.task), title: descriptor.body.manifest.title,
    input_refs: descriptor.body.manifest.payload.map(item => clone(item.artifact)),
    revision: descriptor.body.expected_revision + 1, updated_at: "2026-10-26T01:02:04.987654321Z" },
  manifest: clone(descriptor.body.manifest) };
}
function frozen(value) {
  if (!value || typeof value !== "object") return;
  assert.ok(Object.isFrozen(value)); for (const child of Object.values(value)) frozen(child);
}

test("an empty manual Draft can prepare one explicit title-only metadata replacement", () => {
  const value = baseline([]), descriptor = prepareMetadataSave(value, edits(value));
  assert.deepEqual(descriptor, { task_id: uuid(3), body: { schema_version: 1, expected_revision: 1,
    manifest: { schema_version: 1, title: "Renamed discussion", payload: [] } } });
  frozen(descriptor);
  assert.ok(!("owner" in descriptor.body) && !("contents" in descriptor.body));
});

test("metadata edits preserve exact Artifact references and order while allowing inert duplicate labels", () => {
  const value = baseline(), input = edits(value, { title: value.task.title });
  input.items[0] = { filename: "same.md", language: "unknown-next.v9" };
  input.items[1] = { filename: "same.md", language: "plaintext" };
  const descriptor = prepareMetadataSave(value, input);
  assert.deepEqual(descriptor.body.manifest.payload.map(item => item.artifact), value.task.input_refs);
  assert.deepEqual(descriptor.body.manifest.payload.map(({ filename, language }) => ({ filename, language })), input.items);
  assert.equal(descriptor.body.manifest.payload[0].kind, "text"); frozen(descriptor);
});

test("input objects remain owned by the caller while the outbound descriptor is deeply snapshotted", () => {
  const value = baseline(), input = edits(value), before = clone(value), beforeEdits = clone(input);
  const descriptor = prepareMetadataSave(value, input), expected = clone(descriptor);
  assert.deepEqual(value, before); assert.deepEqual(input, beforeEdits);
  assert.ok(!Object.isFrozen(value) && !Object.isFrozen(input));
  value.task.reference.task_id = uuid(9); value.manifest.payload[0].artifact.sha256 = "a".repeat(64);
  value.contents[0].text = "changed outside"; input.title = "changed outside"; input.items[0].filename = "elsewhere";
  assert.deepEqual(descriptor, expected); frozen(descriptor);
  assert.throws(() => { descriptor.body.manifest.payload[0].filename = "replace"; }, TypeError);
});

test("unchanged manifests are rejected locally instead of creating a fake successful save", () => {
  for (const value of [baseline([]), baseline()]) {
    assert.throws(() => prepareMetadataSave(value, edits(value, { title: value.task.title })), invalid);
  }
});

test("only an existing manual Draft is eligible; every other readable status remains read-only", () => {
  for (const status of ["ready", "in_progress", "blocked", "in_review", "cancelled", "accepted"]) {
    const value = baseline(); value.task.status = status;
    assert.throws(() => prepareMetadataSave(value, edits(value)), invalid, status);
  }
  const value = baseline(); value.task.definition = {};
  assert.throws(() => prepareMetadataSave(value, edits(value)), invalid);
});

test("the exact full baseline requires matching Task/manifest/input/content pins and text profile", () => {
  const mutations = [value => { delete value.contents; }, value => { value.extra = true; },
    value => { value.schema_version = 2; }, value => { value.task.definition_ref = null; },
    value => { value.task.title = "different manifest title"; }, value => { value.task.input_refs.reverse(); },
    value => { value.contents.reverse(); }, value => { value.contents.pop(); },
    value => { value.contents[0].extra = true; }, value => { value.contents[0].text = "\0"; },
    value => { value.contents[0].text = "x".repeat(256 * 1024 + 1); },
    value => { value.task.owner.authority_id = uuid(9); },
    value => { value.manifest.payload[0].artifact.workspace.workspace_id = uuid(9); },
    value => { value.task.reference.task_id = "00000000-0000-0000-0000-000000000000"; },
    value => { value.task.revision = 0; }, value => { value.task.created_at = "2026-02-30T00:00:00Z"; }];
  for (const change of mutations) {
    const value = baseline(); change(value);
    assert.throws(() => prepareMetadataSave(value, edits(value)), invalid);
  }
});

test("edits cannot add or remove payload, supply refs, text, kind, owner or any extra record keys", () => {
  const value = baseline(), mutations = [input => { input.items.pop(); },
    input => { input.items.push({ filename: "new.txt", language: "plaintext" }); },
    input => { input.items[0].artifact = clone(value.manifest.payload[1].artifact); },
    input => { input.items[0].text = "new bytes"; }, input => { input.items[0].kind = "text"; },
    input => { input.owner = clone(value.task.owner); }, input => { delete input.title; },
    input => { delete input.items[0].language; }];
  for (const change of mutations) {
    const input = edits(value); change(input);
    assert.throws(() => prepareMetadataSave(value, input), invalid);
  }
});

test("title and display-label validation exactly bounds UTF-8, UTF-16 and lowercase language hints", () => {
  const value = baseline(), good = edits(value, { title: "😀".repeat(64) });
  good.items[0] = { filename: "😀".repeat(64), language: "a".repeat(64) };
  assert.equal(prepareMetadataSave(value, good).body.manifest.title, good.title);
  const mutations = [input => { input.title = "😀".repeat(64) + "x"; }, input => { input.title = " \t"; },
    input => { input.title = "control\x85"; }, input => { input.title = "\ud800"; },
    ...["", ".", "..", "../item", "dir\\item", "line\n", "😀".repeat(64) + "x"].map(filename => input => { input.items[0].filename = filename; }),
    ...["", "Rust", "a".repeat(65), "has space", "-first"].map(language => input => { input.items[0].language = language; })];
  for (const change of mutations) {
    const input = edits(value); change(input);
    assert.throws(() => prepareMetadataSave(value, input), invalid);
  }
});

test("the maximum bounded payload produces a complete canonical body within the 64KiB wire budget", () => {
  const value = baseline(Array(32).fill("x")), input = edits(value, { title: "😀".repeat(64) });
  input.items.forEach(item => { item.filename = "😀".repeat(64); item.language = "a".repeat(64); });
  const descriptor = prepareMetadataSave(value, input);
  assert.equal(descriptor.body.manifest.payload.length, 32);
  assert.ok(Buffer.byteLength(raw(descriptor.body)) <= 64 * 1024);
  assert.deepEqual(descriptor.body.manifest.payload.map(item => item.artifact), value.task.input_refs);
  const tooMany = baseline(Array(33).fill("x"));
  assert.throws(() => prepareMetadataSave(tooMany, edits(tooMany)), invalid);
});

test("revision increment is exact through MAX_SAFE but cannot round or overflow an expected revision", () => {
  const value = baseline([]); value.task.revision = Number.MAX_SAFE_INTEGER - 1;
  const descriptor = prepareMetadataSave(value, edits(value));
  const result = parseMetadataSaved(raw(receipt(value, descriptor)), value, descriptor);
  assert.equal(result.task.revision, Number.MAX_SAFE_INTEGER);
  for (const revision of [Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1, 1.5, "1"]) {
    value.task.revision = revision;
    assert.throws(() => prepareMetadataSave(value, edits(value)), invalid);
  }
});

test("metadata confirmation returns only strict frozen Task/manifest, never a content-read receipt", () => {
  for (const value of [baseline([]), baseline()]) {
    const descriptor = prepareMetadataSave(value, edits(value)), expected = receipt(value, descriptor);
    const result = parseMetadataSaved(raw(expected), value, descriptor);
    assert.deepEqual(result, expected); frozen(result);
    assert.deepEqual(Object.keys(result), ["schema_version", "task", "manifest"]);
    assert.ok(!("contents" in result) && !("authenticated" in result) && !("committed" in result));
  }
});

test("valid UTC updated time can move backwards without fabricating a server clock guarantee", () => {
  const value = baseline(), descriptor = prepareMetadataSave(value, edits(value)), response = receipt(value, descriptor);
  response.task.updated_at = "2025-01-01T00:00:00.000000001+00:00";
  assert.equal(parseMetadataSaved(raw(response), value, descriptor).task.updated_at, response.task.updated_at);
});

test("confirmation preserves same Task, workspace, owner, creation time and exact one-step Draft revision", () => {
  const value = baseline(), descriptor = prepareMetadataSave(value, edits(value));
  const mutations = [response => { response.task.reference.task_id = uuid(9); },
    response => { response.task.reference.workspace.workspace_id = uuid(9); },
    response => { response.task.owner.principal_id = uuid(9); }, response => { response.task.owner.authority_id = uuid(9); },
    response => { response.task.created_at = "2026-10-25T01:02:03Z"; },
    response => { response.task.revision = 1; }, response => { response.task.revision = 3; },
    response => { response.task.status = "ready"; }, response => { response.task.definition = {}; },
    response => { response.task.updated_at = "2026-02-30T00:00:00Z"; }];
  for (const change of mutations) {
    const response = receipt(value, descriptor); change(response);
    assert.throws(() => parseMetadataSaved(raw(response), value, descriptor), invalid);
  }
});

test("confirmation rejects different metadata, missing/reordered/new pins and full GET bodies", () => {
  const value = baseline(), descriptor = prepareMetadataSave(value, edits(value));
  const mutations = [response => { response.manifest.title = response.task.title = "unissued title"; },
    response => { response.manifest.payload[0].filename = "unissued-name"; },
    response => { response.manifest.payload[0].language = "different"; }, response => { response.task.input_refs.reverse(); },
    response => { response.manifest.payload.reverse(); response.task.input_refs.reverse(); },
    response => { response.manifest.payload.pop(); response.task.input_refs.pop(); },
    response => { response.manifest.payload[0].artifact.sha256 = response.task.input_refs[0].sha256 = "a".repeat(64); },
    response => { response.contents = clone(value.contents); }, response => { response.owner = value.task.owner; },
    response => { delete response.manifest; }, response => { response.schema_version = 2; }];
  for (const change of mutations) {
    const response = receipt(value, descriptor); change(response);
    assert.throws(() => parseMetadataSaved(raw(response), value, descriptor), invalid);
  }
});

test("descriptor validation independently blocks arbitrary caller substitution and non-metadata edits", () => {
  const value = baseline(), original = prepareMetadataSave(value, edits(value));
  const mutations = [descriptor => { descriptor.task_id = uuid(9); }, descriptor => { descriptor.extra = true; },
    descriptor => { descriptor.body.expected_revision = 2; }, descriptor => { descriptor.body.schema_version = 2; },
    descriptor => { descriptor.body.owner = value.task.owner; }, descriptor => { descriptor.body.manifest.payload.reverse(); },
    descriptor => { descriptor.body.manifest.payload.pop(); },
    descriptor => { descriptor.body.manifest.payload[0].artifact.artifact_id = uuid(9); },
    descriptor => { descriptor.body.manifest.payload[0].text = "bytes"; },
    descriptor => { descriptor.body.manifest = clone(value.manifest); }];
  for (const change of mutations) {
    const descriptor = clone(original); change(descriptor);
    assert.throws(() => parseMetadataSaved(raw(receipt(value, descriptor)), value, descriptor), invalid);
  }
});

test("strict receipt decoding refuses duplicate or escaped keys and unsafe numeric tokens", () => {
  const value = baseline(), descriptor = prepareMetadataSave(value, edits(value)), response = raw(receipt(value, descriptor));
  for (const candidate of [response.replace('"schema_version":1', '"schema_version":1,"schema_version":1'),
    response.replace('"title":', '"ti\\u0074le":'), response.replace('"filename":', '"filename":"same","filename":'),
    ...["9007199254740992", "9007199254740993", "2.0", "2e0", "-0"].map(token => response.replace('"revision":2', `"revision":${token}`)),
    response + "x", "[1,{},{}]", "null"]) {
    assert.throws(() => parseMetadataSaved(candidate, value, descriptor), invalid);
  }
});

test("receipt size, depth and token budgets reject before ordinary JSON allocation", () => {
  const value = baseline([]), descriptor = prepareMetadataSave(value, edits(value));
  const original = JSON.parse; let calls = 0;
  JSON.parse = function(...args) { calls++; return original.apply(this, args); };
  try {
    for (const candidate of [" ".repeat(128 * 1024 + 1), "[".repeat(40) + "0" + "]".repeat(40),
      "{" + Array.from({ length: 4097 }, (_, index) => `"x${index}":0`).join(",") + "}"]) {
      assert.throws(() => parseMetadataSaved(candidate, value, descriptor), invalid);
    }
    assert.equal(calls, 0);
  } finally { JSON.parse = original; }
});

test("metadata receipt wire budget is inclusive and measures UTF-8 bytes, not JavaScript characters", () => {
  const value = baseline([]), descriptor = prepareMetadataSave(value, edits(value, { title: "讨论 😀" }));
  const response = receipt(value, descriptor), encoded = raw(response);
  const exact = encoded + " ".repeat(128 * 1024 - Buffer.byteLength(encoded));
  assert.equal(Buffer.byteLength(exact), 128 * 1024);
  assert.deepEqual(parseMetadataSaved(exact, value, descriptor), response);
  assert.throws(() => parseMetadataSaved(exact + " ", value, descriptor), invalid);
  assert.throws(() => parseMetadataSaved(exact + "é", value, descriptor), invalid);
});

test("caller DTO accessors, symbol extras, sparse lists and unexpected prototypes cannot dispatch getters", () => {
  let touched = 0; const value = baseline(), input = edits(value);
  Object.defineProperty(input.items[0], "filename", { enumerable: true, get() { touched++; return "notes.txt"; } });
  assert.throws(() => prepareMetadataSave(value, input), invalid);
  const accessorBaseline = baseline();
  Object.defineProperty(accessorBaseline.contents[0], "text", { enumerable: true, get() { touched++; return "bytes"; } });
  assert.throws(() => prepareMetadataSave(accessorBaseline, edits(accessorBaseline)), invalid);
  const symbolic = edits(value); symbolic[Symbol("owner")] = uuid(4);
  const sparse = edits(value); delete sparse.items[0];
  const inherited = Object.assign(Object.create({ trusted: true }), edits(value));
  for (const input of [symbolic, sparse, inherited]) assert.throws(() => prepareMetadataSave(value, input), invalid);
  const descriptor = clone(prepareMetadataSave(value, edits(value)));
  Object.defineProperty(descriptor.body, "expected_revision", { enumerable: true, get() { touched++; return 1; } });
  assert.throws(() => parseMetadataSaved("{}", value, descriptor), invalid);
  assert.equal(touched, 0);
});

test("supplied baseline bytes are shape-checked but metadata helpers do not rehash or establish provenance", () => {
  const value = baseline(); value.contents[0].text = "supplied input need not be independently hashed here";
  const keys = ["fetch", "document", "localStorage", "sessionStorage", "crypto"];
  const originals = keys.map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]);
  try {
    for (const key of keys) Object.defineProperty(globalThis, key, { configurable: true, get() { throw Error("forbidden I/O or hash"); } });
    const descriptor = prepareMetadataSave(value, edits(value));
    assert.deepEqual(parseMetadataSaved(raw(receipt(value, descriptor)), value, descriptor), receipt(value, descriptor));
    assert.ok(!raw(descriptor).includes(value.contents[0].text));
  } finally {
    for (const [key, property] of originals) {
      if (property) Object.defineProperty(globalThis, key, property); else delete globalThis[key];
    }
  }
});
