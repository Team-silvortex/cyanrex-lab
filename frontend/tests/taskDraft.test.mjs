import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";
import { BUILTIN_LANGUAGES } from "../src/features/editor/languages.ts";

// Match the bundler's one extensionless TypeScript dependency without rewriting production code.
const draftUrl = new URL("../src/features/tasks/taskDraft.ts", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, nextResolve) {
  if (context.parentURL === draftUrl && specifier === "../editor/languages") {
    return nextResolve(new URL("../src/features/editor/languages.ts", import.meta.url).href, context);
  }
  return nextResolve(specifier, context);
} });
let api;
try { api = await import(draftUrl); } finally { resolver.deregister(); }
const { createTaskDraft, createTextPayloadItem, updateTaskTitle, addPayloadItem, updatePayloadItem,
  removePayloadItem, serializeTaskDraft, parseTaskDraft, MAX_TASK_PAYLOAD_ITEMS, MAX_TASK_DRAFT_BYTES } = api;
const encoder = new TextEncoder();
const item = (patch = {}) => ({ id: "local_1", revision: 1, kind: "text", filename: "untitled.txt",
  language: "plaintext", text: "", ...patch });
const draft = (payload = [item()], patch = {}) => ({ format: "cyanrex.task-draft", version: 1, title: "", payload, ...patch });
const bytes = value => encoder.encode(JSON.stringify(value));
const parse = value => parseTaskDraft(bytes(value));
const fails = (fn, key = "invalid") => assert.throws(fn, { name: "Error", message: `taskDraft.${key}` });
const frozen = value => {
  assert.ok(Object.isFrozen(value)); assert.ok(Object.isFrozen(value.payload));
  for (const entry of value.payload) assert.ok(Object.isFrozen(entry));
};

test("empty factories are explicit local drafts, with no server identity or implicit content", () => {
  assert.deepEqual(createTaskDraft(), draft([]));
  assert.deepEqual(createTextPayloadItem("local_1"), item());
  frozen(createTaskDraft()); assert.ok(Object.isFrozen(createTextPayloadItem("local_1")));
  assert.equal(MAX_TASK_PAYLOAD_ITEMS, 32); assert.equal(MAX_TASK_DRAFT_BYTES, 8 * 1024 * 1024);
});

test("title updates preserve content, enforce UTF-8 bytes and never mutate the source", () => {
  const source = draft(); const next = updateTaskTitle(source, "中文 task");
  assert.equal(next.title, "中文 task"); assert.equal(source.title, "");
  assert.deepEqual(next.payload, source.payload); assert.notEqual(next.payload, source.payload); frozen(next);
  assert.equal(updateTaskTitle(next, "").title, "");
  assert.equal(updateTaskTitle(next, "é".repeat(128)).title.length, 128);
  fails(() => updateTaskTitle(next, "é".repeat(129)), "tooLarge");
  for (const title of [null, 4, "bad\nname", "bad\tname", "\0", "\ud800"]) fails(() => updateTaskTitle(next, title));
});

test("adding content preserves order and owns an immutable copy of caller data", () => {
  const source = draft(); const added = item({ id: "second", text: "fn main() {}", language: "rust", filename: "main.rs" });
  const next = addPayloadItem(source, added);
  assert.deepEqual(next.payload.map(value => value.id), ["local_1", "second"]); assert.equal(source.payload.length, 1);
  added.text = "changed outside"; source.payload[0].text = "also changed";
  assert.equal(next.payload[1].text, "fn main() {}"); assert.equal(next.payload[0].text, ""); frozen(next);
  assert.throws(() => { next.payload[0].text = "not allowed"; }, TypeError);
});

test("bounded local IDs and duplicate detection never treat IDs as server references", () => {
  for (const id of ["a", "_-A09", "x".repeat(80), "__proto__", "constructor"]) {
    assert.equal(createTextPayloadItem(id).id, id);
  }
  for (const id of ["", "x".repeat(81), "has space", "../x", "a/b", "a\\b", "中", null, 42]) fails(() => createTextPayloadItem(id));
  fails(() => addPayloadItem(draft(), item()));
  fails(() => parse(draft([item(), item()])));
  fails(() => serializeTaskDraft(draft([item(), item()])));
});

test("item changes are revision-checked immutable updates of only filename, language and text", () => {
  const source = draft([item(), item({ id: "other" })]);
  const next = updatePayloadItem(source, "local_1", 1, { filename: "hello.py", language: "python", text: "print('你好')\n" });
  assert.deepEqual(next.payload[0], item({ revision: 2, filename: "hello.py", language: "python", text: "print('你好')\n" }));
  assert.deepEqual(source.payload[0], item()); assert.deepEqual(next.payload[1], item({ id: "other" })); frozen(next);
  assert.equal(updatePayloadItem(next, "local_1", 2, {}).payload[0].revision, 3);
  fails(() => updatePayloadItem(next, "local_1", 1, { text: "old response" }), "stale");
  fails(() => updatePayloadItem(next, "absent", 1, { text: "x" }));
  for (const patch of [null, [], { id: "other" }, { revision: 1 }, { kind: "artifact" }, { text: undefined }, { future: true }]) {
    fails(() => updatePayloadItem(next, "local_1", 2, patch));
  }
});

test("removing content requires the current revision and does not mutate survivors", () => {
  const source = draft([item(), item({ id: "other", revision: 3 })]);
  fails(() => removePayloadItem(source, "other", 1), "stale");
  fails(() => removePayloadItem(source, "absent", 1));
  const next = removePayloadItem(source, "other", 3);
  assert.deepEqual(next.payload, [item()]); assert.equal(source.payload.length, 2); frozen(next);
  assert.deepEqual(removePayloadItem(next, "local_1", 1).payload, []);
});

test("revisions must be positive JSON-safe integers and cannot overflow on mutation", () => {
  for (const revision of [0, -1, 0.5, Number.MAX_SAFE_INTEGER + 1, NaN, Infinity, "1", null]) {
    fails(() => addPayloadItem(createTaskDraft(), item({ revision })));
    fails(() => updatePayloadItem(draft(), "local_1", revision, { text: "x" }));
    fails(() => removePayloadItem(draft(), "local_1", revision));
  }
  const max = draft([item({ revision: Number.MAX_SAFE_INTEGER })]);
  assert.equal(parse(max).payload[0].revision, Number.MAX_SAFE_INTEGER);
  fails(() => updatePayloadItem(max, "local_1", Number.MAX_SAFE_INTEGER, { text: "x" }));
  assert.deepEqual(removePayloadItem(max, "local_1", Number.MAX_SAFE_INTEGER).payload, []);
});

test("all registered languages are supported without guessing unknown language IDs", () => {
  for (const { id } of BUILTIN_LANGUAGES) assert.equal(parse(draft([item({ language: id })])).payload[0].language, id);
  for (const language of ["", "Python", "tsx", "js", "__proto__", "constructor", null]) fails(() => parse(draft([item({ language })])));
});

test("filenames are bounded display basenames, never paths or control sequences", () => {
  for (const filename of ["README", "中文.md", ".env", "a".repeat(128)]) {
    assert.equal(parse(draft([item({ filename })])).payload[0].filename, filename);
  }
  for (const filename of ["", ".", "..", "a".repeat(129), "/tmp/x", "a/b", "C:\\x", "a\nb", "a\tb", "a\0b", "a\x7fb", "\udfff", null]) {
    fails(() => parse(draft([item({ filename })])));
  }
});

test("text round trips byte-for-byte, including Unicode, indentation and literal HTML", () => {
  const source = draft([item({ text: "\t<script>not executed()</script>\r\n中文 👩‍💻 é \" \\\n", filename: "readme.md", language: "markdown" })], { title: "Draft 🌟" });
  const serialized = serializeTaskDraft(source);
  assert.equal(typeof serialized, "string"); assert.deepEqual(parseTaskDraft(encoder.encode(serialized)), source);
  frozen(parseTaskDraft(encoder.encode(serialized)));
});

test("binary controls and invalid Unicode cannot enter through JSON escapes or mutation", () => {
  for (const text of ["\0", "\x01", "\x08", "\x0b", "\x0c", "\x1b", "\x1f", "\x7f", "\x85", "\ud800", "\udfff"]) {
    fails(() => parse(draft([item({ text })])));
    fails(() => updatePayloadItem(draft(), "local_1", 1, { text }));
  }
  assert.equal(parse(draft([item({ text: "\t\n\r😀" })])).payload[0].text, "\t\n\r😀");
});

test("per-item text limit counts UTF-8 bytes rather than characters", () => {
  const limit = 256 * 1024;
  for (const text of ["x".repeat(limit), "😀".repeat(limit / 4)]) {
    assert.equal(parse(draft([item({ text })])).payload[0].text, text);
  }
  for (const text of ["x".repeat(limit + 1), "😀".repeat(limit / 4) + "x"]) {
    fails(() => parse(draft([item({ text })])), "tooLarge");
    fails(() => serializeTaskDraft(draft([item({ text })])), "tooLarge");
  }
});

test("at most 32 items are accepted and the 33rd cannot be added or imported", () => {
  const full = draft(Array.from({ length: 32 }, (_, index) => item({ id: `item_${index}` })));
  assert.equal(parse(full).payload.length, 32);
  fails(() => addPayloadItem(full, item({ id: "one_more" })), "tooMany");
  fails(() => parse({ ...full, payload: [...full.payload, item({ id: "one_more" })] }), "tooMany");
  fails(() => serializeTaskDraft({ ...full, payload: [...full.payload, item({ id: "one_more" })] }), "tooMany");
});

test("JSON byte limit is enforced before decoding and exactly at the import boundary", () => {
  const value = bytes(draft([]));
  const exact = new Uint8Array(MAX_TASK_DRAFT_BYTES).fill(32); exact.set(value);
  assert.deepEqual(parseTaskDraft(exact), draft([]));
  fails(() => parseTaskDraft(new Uint8Array(MAX_TASK_DRAFT_BYTES + 1).fill(0xff)), "tooLarge");
});

test("JSON escaping overhead is bounded on export, not hidden by the raw text limit", () => {
  const large = draft(Array.from({ length: 17 }, (_, index) => item({ id: `i_${index}`, text: "\\".repeat(256 * 1024) })));
  fails(() => serializeTaskDraft(large), "tooLarge");
  const ordinary = draft(Array.from({ length: 16 }, (_, index) => item({ id: `i_${index}`, text: "x".repeat(256 * 1024) })));
  assert.deepEqual(parseTaskDraft(encoder.encode(serializeTaskDraft(ordinary))), ordinary);
});

test("invalid UTF-8 and malformed or wrong-shaped JSON never silently recover", () => {
  for (const data of [new Uint8Array([0xc3, 0x28]), new Uint8Array([0xff]), new Uint8Array(), encoder.encode("{"), encoder.encode("{} trailing")]) {
    fails(() => parseTaskDraft(data));
  }
  for (const value of [null, true, 1, "draft", [], {}, { ...draft(), payload: null }, { ...draft(), payload: {} }]) fails(() => parse(value));
  for (const value of [null, "{}", [], new ArrayBuffer(2)]) fails(() => parseTaskDraft(value));
});

test("format, version, kind and exact field sets cannot masquerade as server objects", () => {
  for (const patch of [{ format: "TaskSnapshot" }, { version: 2 }, { version: "1" }, { owner: "teacher" }, { input_refs: [] }, { artifact_ref: {} }, { task_id: "x" }]) {
    fails(() => parse(draft([item()], patch)));
  }
  for (const patch of [{ kind: "artifact" }, { sha256: "abc" }, { owner: "teacher" }, { authority_id: "x" }, { extra: true }]) {
    fails(() => parse(draft([item(patch)])));
  }
  for (const key of Object.keys(draft())) { const source = draft(); delete source[key]; fails(() => parse(source)); }
  for (const key of Object.keys(item())) { const source = item(); delete source[key]; fails(() => parse(draft([source]))); }
  const polluted = JSON.parse('{"format":"cyanrex.task-draft","version":1,"title":"","payload":[],"__proto__":{"polluted":true}}');
  fails(() => parse(polluted)); assert.equal({}.polluted, undefined);
});

test("serializing and all mutations revalidate caller-provided state rather than trusting types", () => {
  const bad = draft([item({ kind: "artifact" })]);
  for (const invoke of [() => serializeTaskDraft(bad), () => updateTaskTitle(bad, "x"),
    () => addPayloadItem(bad, item({ id: "next" })), () => updatePayloadItem(bad, "local_1", 1, { text: "x" }),
    () => removePayloadItem(bad, "local_1", 1)]) fails(invoke);
  const source = draft(); source.payload.push(undefined); fails(() => serializeTaskDraft(source));
  const extra = draft(); extra[Symbol("extra")] = true; fails(() => serializeTaskDraft(extra));
  const custom = Object.assign(Object.create({ title: "hidden" }), draft()); fails(() => serializeTaskDraft(custom));
  const getter = draft(); Object.defineProperty(getter, "title", { get() { throw new Error("must not execute getter"); }, enumerable: true });
  fails(() => serializeTaskDraft(getter));
});

test("payload arrays cannot hide extra properties, holes, accessors or custom iterators", () => {
  const extra = [item()]; extra.future = true;
  const sparse = new Array(1);
  const getter = [item()]; Object.defineProperty(getter, "0", { get() { throw new Error("must not execute array getter"); } });
  const iterator = [item()]; iterator[Symbol.iterator] = function* () { throw new Error("must not execute iterator"); };
  const custom = [item()]; Object.setPrototypeOf(custom, { hidden: true });
  for (const payload of [extra, sparse, getter, iterator, custom]) fails(() => serializeTaskDraft(draft(payload)));
});
