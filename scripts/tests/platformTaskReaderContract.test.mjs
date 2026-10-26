import assert from "node:assert/strict";
import test from "node:test";
import { createHash } from "node:crypto";
import { parseIndex, parseContent } from "../../engine/src/bin/cyanrex-platform/session_ui/task-contract.js";

const uuid = n => `00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;
const scope = { authority_id: uuid(1), workspace_id: uuid(2) };
const stamp = "2026-10-26T01:02:03.123456789Z";
const digest = text => createHash("sha256").update(text, "utf8").digest("hex");
const reference = (n = 3) => ({ workspace: { ...scope }, task_id: uuid(n) });
const entry = (patch = {}) => ({ reference: reference(), revision: 1, title: "Private notes", status: "draft",
  definition_ref: null, updated_at: stamp, payload_items: 0, ...patch });
const wire = value => JSON.stringify(value);
const page = (entries = [], next_after = null) => ({ schema_version: 1, entries, next_after });
const invalid = { message: "taskRead.invalid" };

test("empty owner-private metadata is a frozen page, not readable content or authorization", () => {
  const value = parseIndex(wire(page()), { after: null, scope: null });
  assert.deepEqual(value, page());
  assert.ok(Object.isFrozen(value) && Object.isFrozen(value.entries));
});

const clone = value => structuredClone(value);
function content(texts = ["讨论笔记 😀\n"]) {
  const payload = texts.map((text, index) => ({ kind: "text", filename: "notes.txt", language: "future-lang+v2",
    artifact: { workspace: { ...scope }, artifact_id: uuid(100 + index), revision_id: uuid(200 + index), sha256: digest(text) } }));
  return { schema_version: 1, task: { schema_version: 1, reference: reference(),
    owner: { authority_id: scope.authority_id, principal_id: uuid(4) }, title: "Private notes", definition: null,
    input_refs: payload.map(item => clone(item.artifact)), status: "draft", revision: 1,
    created_at: stamp, updated_at: stamp }, manifest: { schema_version: 1, title: "Private notes", payload },
    contents: payload.map((item, index) => ({ artifact: clone(item.artifact), text: texts[index] })) };
}
function selected(value) { return entry({ payload_items: value.contents.length }); }
function frozen(value) {
  if (!value || typeof value !== "object") return;
  assert.ok(Object.isFrozen(value)); for (const child of Object.values(value)) frozen(child);
}

test("index validates exact ordered references, explicit null and one position-only cursor", () => {
  const rows = [entry(), entry({ reference: reference(5), title: "<script>inert</script>" })];
  const result = parseIndex(wire(page(rows, reference(5))), { scope, limit: 2, after: uuid(2) });
  assert.deepEqual(result, page(rows, reference(5))); frozen(result);
  for (const value of [page(rows, reference(3)), page(rows, reference(6)), page([], reference(3)),
    page(rows.slice(0, 1), reference(3)), page([...rows].reverse())]) {
    assert.throws(() => parseIndex(wire(value), { scope, limit: 2 }), invalid);
  }
  assert.throws(() => parseIndex(wire(page([entry()])), { scope, after: uuid(3) }), invalid);
});

test("index scope inference checks the whole page and never fabricates owner or content metadata", () => {
  const result = parseIndex(wire(page([entry()])), { scope: null });
  assert.deepEqual(result.entries[0].reference.workspace, scope);
  for (const field of ["owner", "manifest", "contents", "artifacts", "text"]) assert.ok(!(field in result.entries[0]));
  const bad = entry(); bad.reference.workspace.workspace_id = uuid(9);
  assert.throws(() => parseIndex(wire(page([entry(), bad])), { scope: null }), invalid);
  assert.throws(() => parseIndex(wire(page([bad])), { scope }), invalid);
});

test("default pages stop at twenty while an explicit bounded thirty-two page remains exact", () => {
  const rows = Array.from({ length: 32 }, (_, index) => entry({ reference: reference(10 + index) }));
  assert.equal(parseIndex(wire(page(rows.slice(0, 20), reference(29)))).entries.length, 20);
  assert.throws(() => parseIndex(wire(page(rows.slice(0, 21)))), invalid);
  const value = parseIndex(wire(page(rows, reference(41))), { scope, limit: 32 });
  assert.equal(value.entries.length, 32); assert.deepEqual(value.next_after, reference(41));
  assert.throws(() => parseIndex(wire(page([...rows, entry({ reference: reference(42) })])), { limit: 32 }), invalid);
});

test("all six Task statuses are readable without treating cancelled as a browser operation result", async () => {
  for (const status of ["draft", "ready", "in_progress", "blocked", "in_review", "cancelled"]) {
    const row = entry({ status });
    assert.equal(parseIndex(wire(page([row])), { scope }).entries[0].status, status);
    const value = content([]); value.task.status = status;
    const result = await parseContent(wire(value), row, scope);
    assert.equal(result.task.status, status); frozen(result);
  }
});

test("index rejects wrong keys, legacy aliases, catalogue fallback, invalid scalars and times", () => {
  const mutations = [
    value => { value.schema_version = 2; }, value => { delete value.next_after; },
    value => { value.owner = uuid(4); }, value => { value.entries[0].definition = null; },
    value => { value.entries[0].definition_ref = {}; }, value => { delete value.entries[0].definition_ref; },
    value => { value.entries[0].revision = 0; }, value => { value.entries[0].status = "accepted"; },
    value => { value.entries[0].payload_items = 33; }, value => { value.entries[0].payload_items = -1; },
    value => { value.entries[0].reference.task_id = "00000000-0000-0000-0000-000000000000"; },
    value => { value.entries[0].reference.task_id = uuid(0xabcd).toUpperCase(); },
    value => { value.entries[0].title = " "; }, value => { value.entries[0].title = "é".repeat(129); },
    value => { value.entries[0].title = "title\t"; }, value => { value.entries[0].updated_at = "2026-02-30T00:00:00Z"; },
    value => { value.entries[0].updated_at = "2026-10-26"; },
  ];
  for (const change of mutations) {
    const value = page([entry()]); change(value);
    assert.throws(() => parseIndex(wire(value), { scope }), invalid);
  }
  for (const options of [{ limit: 0 }, { limit: 33 }, { limit: 1.5 }, { limit: "20" },
    { after: "not-a-task" }, { scope: { ...scope, owner: uuid(4) } }]) {
    assert.throws(() => parseIndex(wire(page()), options), invalid);
  }
});

test("wire lexer rejects duplicate and escaped alias keys before ordinary JSON.parse", async () => {
  const validPage = wire(page([entry()]));
  for (const raw of [
    validPage.replace('"schema_version":1', '"schema_version":1,"schema_version":1'),
    validPage.replace('"revision":1', '"revision":1,"revision":1'),
    validPage.replace('"title":', '"ti\\u0074le":'),
    validPage.replace('"authority_id":', '"authority_\\u0069d":'),
  ]) assert.throws(() => parseIndex(raw, { scope }), invalid);
  const value = content(["plain"]);
  for (const raw of [
    wire(value).replace('"text":"plain"', '"text":"plain","text":"plain"'),
    wire(value).replace('"artifact_id":', '"artifact_\\u0069d":'),
  ]) await assert.rejects(() => parseContent(raw, selected(value), scope), invalid);
});

test("unsafe or noncanonical numeric tokens cannot be rounded into an otherwise valid DTO", () => {
  for (const token of ["9007199254740992", "9007199254740993", "9007199254740991.1", "1.0", "1e0", "1E0", "-0", "01"]) {
    const raw = wire(page([entry()])).replace('"revision":1', `"revision":${token}`);
    assert.throws(() => parseIndex(raw, { scope }), invalid, token);
  }
  const result = parseIndex(wire(page([entry({ revision: Number.MAX_SAFE_INTEGER })])), { scope });
  assert.equal(result.entries[0].revision, Number.MAX_SAFE_INTEGER);
});

test("lexical depth/token and raw wire limits reject before JSON allocation", async () => {
  let calls = 0; const original = JSON.parse;
  JSON.parse = function(...args) { calls++; return original.apply(this, args); };
  try {
    assert.throws(() => parseIndex(" ".repeat(128 * 1024 + 1), { scope }), invalid);
    assert.throws(() => parseIndex("[".repeat(40) + "0" + "]".repeat(40), { scope }), invalid);
    assert.throws(() => parseIndex("[" + Array(9000).fill("0").join(",") + "]", { scope }), invalid);
    const manyKeys = "{" + Array.from({ length: 4097 }, (_, index) => `"x${index}":0`).join(",") + "}";
    assert.throws(() => parseIndex(manyKeys, { scope }), invalid);
    await assert.rejects(() => parseContent(" ".repeat(50 * 1024 * 1024 + 1), entry(), scope), invalid);
    assert.equal(calls, 0);
  } finally { JSON.parse = original; }
});

test("strict wire syntax rejects malformed delimiters but retains escaped inert value strings", async () => {
  for (const raw of ["", "null", "[]", "true", "{}{}", "{\"entries\":[] ,}",
    "{\"entries\":[0,]}", wire(page()) + "x", "\u00a0" + wire(page())]) {
    assert.throws(() => parseIndex(raw, { scope }), invalid, raw);
  }
  const value = content(["literal \\\"value\\\"\t\r\n😀"]), raw = wire(value)
    .replaceAll("Private notes", "Private \\u006eotes");
  assert.deepEqual(await parseContent(raw, selected(value), scope), value);
});

test("details match every ordered manifest/input/content pin and exact recomputed UTF-8 digest", async () => {
  const value = content(["讨论 😀\r\n", "same", "same"]);
  const result = await parseContent(wire(value), selected(value), scope);
  assert.deepEqual(result, value); frozen(result);
  assert.equal(result.manifest.payload[0].language, "future-lang+v2");
  assert.equal(result.manifest.payload[0].filename, result.manifest.payload[1].filename);
  assert.throws(() => { result.contents[0].text = "replace"; }, TypeError);
});

test("empty manual Task returns explicit contents and does not require a code/editor payload", async () => {
  const value = content([]), result = await parseContent(wire(value), selected(value), scope);
  assert.deepEqual(result.contents, []); assert.equal(result.task.definition, null);
  assert.equal(result.task.created_at, stamp); assert.equal(result.task.updated_at, stamp);
});

test("fresh details may advance beyond an index row while same-revision metadata cannot drift", async () => {
  const original = content(), row = selected(original);
  const advanced = clone(original); advanced.task.revision = 2; advanced.task.status = "ready";
  advanced.task.title = advanced.manifest.title = "Edited after listing";
  advanced.task.updated_at = "2026-10-26T02:03:04.123456789Z";
  assert.equal((await parseContent(wire(advanced), row, scope)).task.revision, 2);
  for (const change of [
    value => { value.task.title = value.manifest.title = "Drift at same revision"; },
    value => { value.task.status = "ready"; },
    value => { value.task.updated_at = "2026-10-26T02:03:04Z"; },
  ]) {
    const bad = clone(original); change(bad);
    await assert.rejects(() => parseContent(wire(bad), row, scope), invalid);
  }
  await assert.rejects(() => parseContent(wire(original), { ...row, revision: 2 }, scope), invalid);
});

test("detail confirmation rejects wrong Task/owner/scope/pin/order/shape instead of showing partial text", async () => {
  const mutations = [
    value => { value.task.reference.task_id = uuid(9); }, value => { value.task.owner.authority_id = uuid(9); },
    value => { value.task.definition = {}; }, value => { value.task.definition_ref = null; },
    value => { delete value.contents; }, value => { value.contents[0].extra = true; },
    value => { value.task.input_refs.reverse(); }, value => { value.contents.reverse(); },
    value => { value.contents[0].text = "same length? definitely not exact"; },
    value => { value.manifest.payload[0].artifact.sha256 = "A".repeat(64); },
    value => { value.manifest.payload[0].filename = "../secret"; },
    value => { value.manifest.payload[0].language = "Rust"; },
    value => { value.task.input_refs[0].workspace.workspace_id = uuid(9); },
    value => { value.manifest.payload[1] = clone(value.manifest.payload[0]); },
  ];
  for (const change of mutations) {
    const value = content(["first", "second"]), row = selected(value); change(value);
    await assert.rejects(() => parseContent(wire(value), row, scope), invalid);
  }
});

test("text uses a 256KiB UTF-8 bound, rejects controls/lone surrogates, and admits only tab CR LF", async () => {
  for (const text of ["\t\r\n😀", "😀".repeat(65536)]) {
    const value = content([text]); assert.deepEqual((await parseContent(wire(value), selected(value), scope)).contents, value.contents);
  }
  for (const text of ["😀".repeat(65536) + "x", "\0", "\x7f", "\x85", "\ud800"]) {
    const value = content([text]); await assert.rejects(() => parseContent(wire(value), selected(value), scope), invalid);
  }
});

test("full 32-item backslash content can legitimately exceed 16MiB after metadata and escaping", async () => {
  const value = content(Array(32).fill("\\".repeat(256 * 1024))), raw = wire(value);
  assert.ok(Buffer.byteLength(raw) > 16 * 1024 * 1024);
  const result = await parseContent(raw, selected(value), scope);
  assert.equal(result.contents.length, 32); assert.equal(result.contents.at(-1).text, value.contents.at(-1).text);
});

test("display basenames retain UTF-16 bounds and different revisions of one Artifact stay separate", async () => {
  const value = content(["old", "new"]);
  value.manifest.payload[0].filename = "😀".repeat(64);
  value.manifest.payload[1].artifact.artifact_id = value.manifest.payload[0].artifact.artifact_id;
  value.task.input_refs = value.manifest.payload.map(item => clone(item.artifact));
  value.contents.forEach((item, index) => { item.artifact = clone(value.manifest.payload[index].artifact); });
  assert.deepEqual(await parseContent(wire(value), selected(value), scope), value);
  value.manifest.payload[0].filename += "x";
  await assert.rejects(() => parseContent(wire(value), selected(value), scope), invalid);
});

test("selected metadata is snapshotted before hashing and malformed objects cannot execute getters", async () => {
  const value = content(), row = selected(value), selectedScope = { ...scope };
  const promise = parseContent(wire(value), row, selectedScope);
  row.reference.task_id = uuid(9); row.title = "changed outside"; selectedScope.workspace_id = uuid(9);
  assert.equal((await promise).task.reference.task_id, uuid(3));
  let touched = 0; const accessor = entry();
  Object.defineProperty(accessor, "title", { enumerable: true, get() { touched++; return "Private notes"; } });
  await assert.rejects(() => parseContent(wire(value), accessor, scope), invalid); assert.equal(touched, 0);
});

test("caller-selected metadata rejects accessors, symbol extras and unexpected prototypes", async () => {
  let touched = 0;
  const options = {};
  Object.defineProperty(options, "limit", { enumerable: true, get() { touched++; return 20; } });
  assert.throws(() => parseIndex(wire(page()), options), invalid);
  const symbolic = entry(); symbolic[Symbol("owner")] = uuid(4);
  const inherited = Object.assign(Object.create({ trusted: true }), entry());
  for (const row of [symbolic, inherited]) {
    await assert.rejects(() => parseContent(wire(content([])), row, scope), invalid);
  }
  assert.equal(touched, 0);
});

test("pure parsing and local SHA-256 do not fetch, access Cookies, storage or infer a teaching role", async () => {
  const keys = ["fetch", "document", "localStorage", "sessionStorage"];
  const originals = keys.map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]);
  try {
    for (const key of keys) Object.defineProperty(globalThis, key, { configurable: true, get() { throw Error("forbidden I/O"); } });
    const value = content();
    const rows = parseIndex(wire(page([selected(value)])), { scope });
    const result = await parseContent(wire(value), rows.entries[0], scope);
    assert.ok(!("role" in result.task) && !("authenticated" in result));
  } finally {
    for (const [key, descriptor] of originals) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor); else delete globalThis[key];
    }
  }
});
