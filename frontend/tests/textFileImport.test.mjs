import assert from "node:assert/strict";
import test from "node:test";
import { registerHooks } from "node:module";

const featureRoot = new URL("../src/features/", import.meta.url).href;
const resolver = registerHooks({ resolve(specifier, context, nextResolve) {
  if (context.parentURL?.startsWith(featureRoot) && specifier.startsWith(".")
    && !/\.[cm]?[jt]sx?$/.test(specifier)) {
    return nextResolve(new URL(`${specifier}.ts`, context.parentURL).href, context);
  }
  return nextResolve(specifier, context);
} });
let api, core;
try {
  api = await import("../src/features/tasks/textFileImport.ts");
  core = await import("../src/features/tasks/taskDraft.ts");
} finally { resolver.deregister(); }
const { preflightTextFiles, importTextFiles, MAX_TEXT_FILE_BYTES, TEXT_FILE_IMPORT_TIMEOUT_MS } = api;
const { createTaskDraft, addPayloadItem, serializeTaskDraft, updateTaskTitle, MAX_TASK_DRAFT_BYTES } = core;
const encoder = new TextEncoder();
const buffer = value => typeof value === "string" ? encoder.encode(value).buffer : new Uint8Array(value).buffer;
const item = (id = "old", extra = {}) => ({ id, revision: 7, kind: "text", filename: "older.txt",
  language: "unknown_hint", text: "original\r\n中文", ...extra });
const draft = (payload = [item()]) => payload.reduce((value, entry) => addPayloadItem(value, entry),
  updateTaskTitle(createTaskDraft(), "Original title"));
const file = (name, text, read = () => Promise.resolve(buffer(text))) => ({ name,
  size: typeof text === "string" ? encoder.encode(text).byteLength : text.length, arrayBuffer: read });
const signal = () => new AbortController().signal;
const rejected = (pending, key) => assert.rejects(pending, { name: "Error", message: `taskDraft.${key}` });
const run = (source, files, options = {}) => importTextFiles(source, files,
  options.id ?? ids(), options.signal ?? signal(), options.guard ?? (() => {}));
const ids = () => { let count = 0; return () => `new_${++count}`; };
const unchanged = (source, before) => assert.equal(serializeTaskDraft(source), before);
function frozen(value) {
  assert.ok(Object.isFrozen(value)); assert.ok(Object.isFrozen(value.payload));
  value.payload.forEach(entry => assert.ok(Object.isFrozen(entry)));
}
function held(name = "held.txt", size = 1) {
  let resolve, reject, announce;
  const pending = new Promise((done, fail) => { resolve = done; reject = fail; });
  const started = new Promise(done => { announce = done; });
  const value = { name, size, reads: 0, arrayBuffer() { this.reads++; announce(); return pending; } };
  return { file: value, started, resolve: value => resolve(buffer(value)), reject };
}
async function started(pending, gate) {
  const phase = await Promise.race([
    gate.started.then(() => "reading"),
    pending.then(() => "finished", () => "rejected"),
  ]);
  assert.equal(phase, "reading", "the real read starts before this event-controlled scenario");
}

test("text file batch appends selected Unicode and non-code texts without changing title or old items", async () => {
  const source = draft(), before = serializeTaskDraft(source), order = [];
  const files = [file("../../notes.md", "# 中文 👩‍💻\r\n", async () => { order.push("notes"); return buffer("# 中文 👩‍💻\r\n"); }),
    file("data.json", '{"answer":42}\n', async () => { order.push("json"); return buffer('{"answer":42}\n'); }),
    file("README", "literal <script>not executed</script>\t", async () => { order.push("readme"); return buffer("literal <script>not executed</script>\t"); })];
  const next = await run(source, files, { id: ids() });
  assert.deepEqual(order, ["notes", "json", "readme"]);
  assert.equal(next.title, source.title); assert.deepEqual(next.payload[0], source.payload[0]);
  assert.deepEqual(next.payload.slice(1), [
    { id: "new_1", revision: 1, kind: "text", filename: "notes.md", language: "markdown", text: "# 中文 👩‍💻\r\n" },
    { id: "new_2", revision: 1, kind: "text", filename: "data.json", language: "json", text: '{"answer":42}\n' },
    { id: "new_3", revision: 1, kind: "text", filename: "README", language: "plaintext", text: "literal <script>not executed</script>\t" },
  ]);
  assert.notEqual(next, source); assert.notEqual(next.payload, source.payload); frozen(next); unchanged(source, before);
});

test("preflight freezes safe display names and cheap invalid input neither reads nor allocates IDs", async () => {
  assert.equal(MAX_TEXT_FILE_BYTES, 256 * 1024); assert.equal(TEXT_FILE_IMPORT_TIMEOUT_MS, 10_000);
  let reads = 0, allocations = 0;
  const checked = (name, size) => ({ name, size, arrayBuffer() { reads++; return Promise.resolve(buffer("x")); } });
  const valid = [checked("C:\\temp\\draft?.py", 1), checked("note\n\u202e.txt", 0)];
  assert.deepEqual(preflightTextFiles(draft(), valid), [{ name: "draft_.py", size: 1 }, { name: "note__.txt", size: 0 }]);
  const cases = [
    [draft(), [], "invalid"],
    [draft(), [checked(null, 1)], "invalid"],
    ...[-1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1, "1"].map(size => [draft(), [checked("bad.txt", size)], "invalid"]),
    [draft(), [checked("large.txt", MAX_TEXT_FILE_BYTES + 1)], "tooLarge"],
    [draft(), Array.from({ length: 32 }, () => checked("extra.txt", 0)), "tooMany"],
    [{ ...draft(), owner: "not-a-server-authority" }, valid, "invalid"],
  ];
  for (const [source, files, key] of cases) {
    assert.throws(() => preflightTextFiles(source, files), { message: `taskDraft.${key}` });
    await rejected(run(source, files, { id: () => { allocations++; return "never"; } }), key);
  }
  assert.equal(reads, 0); assert.equal(allocations, 0);
});

test("duplicate filenames append independent revision-one IDs and invalid ID generation is atomic", async () => {
  const source = draft([item("old", { filename: "same.txt" })]), before = serializeTaskDraft(source);
  const files = [file("same.txt", "first"), file("same.txt", "second")];
  const next = await run(source, files, { id: ids() });
  assert.deepEqual(next.payload.map(entry => entry.filename), ["same.txt", "same.txt", "same.txt"]);
  assert.deepEqual(next.payload.map(entry => entry.id), ["old", "new_1", "new_2"]);
  assert.deepEqual(next.payload.map(entry => entry.revision), [7, 1, 1]);
  for (const id of [() => "old", () => "same_id", () => "../invalid", () => "", () => { throw new Error("private allocation detail"); }]) {
    await rejected(run(source, files, { id }), "invalid"); unchanged(source, before);
  }
});

test("strict UTF-8 controls read failures and actual byte-length lies reject the complete batch", async () => {
  const source = draft(), before = serializeTaskDraft(source);
  const bad = [
    [file("invalid.txt", [0xc3, 0x28]), "invalid"],
    [file("invalid.txt", [0xff]), "invalid"],
    ...["\0", "\x08", "\x0b", "\x0c", "\x1b", "\x7f", "\u0085"].map(text => [file("binary.txt", text), "invalid"]),
    [file("failed.txt", "x", () => Promise.reject(new Error("private local path"))), "readFailed"],
    [file("failed.txt", "x", () => { throw new Error("private read detail"); }), "readFailed"],
    [{ ...file("lie.txt", "x"), size: 0 }, "readFailed"],
    [{ ...file("lie.txt", "x"), size: 2 }, "readFailed"],
    [file("wrong-buffer.txt", "x", () => Promise.resolve("x")), "readFailed"],
    [file("wrong-buffer.txt", "x", () => Promise.resolve(new Uint8Array([120]))), "readFailed"],
  ];
  for (const [value, key] of bad) {
    let laterReads = 0;
    await rejected(run(source, [file("good.txt", "prefix"), value,
      file("later.txt", "later", async () => { laterReads++; return buffer("later"); })], { id: ids() }), key);
    assert.equal(laterReads, 0); unchanged(source, before);
  }
  const next = await run(source, [file("bom.txt", [0xef, 0xbb, 0xbf, 65]), file("empty.txt", "")], { id: ids() });
  assert.deepEqual(next.payload.slice(1).map(entry => entry.text), ["A", ""]); unchanged(source, before);
});

test("32 total items and exact per-file UTF-8 size are accepted but serialized escaping also counts", async () => {
  const thirtyTwo = await run(draft([]), Array.from({ length: 32 }, (_, index) => file(`f${index}.txt`, "")), { id: ids() });
  assert.equal(thirtyTwo.payload.length, 32); frozen(thirtyTwo);
  for (const text of ["x".repeat(MAX_TEXT_FILE_BYTES), "😀".repeat(MAX_TEXT_FILE_BYTES / 4)]) {
    const next = await run(draft([]), [file("exact.txt", text)], { id: ids() });
    assert.equal(next.payload[0].text, text);
  }
  const source = draft([]), before = serializeTaskDraft(source);
  const escaped = Array.from({ length: 17 }, (_, index) => file(`escaped-${index}.txt`, "\\".repeat(MAX_TEXT_FILE_BYTES)));
  assert.ok(17 * MAX_TEXT_FILE_BYTES < MAX_TASK_DRAFT_BYTES);
  await rejected(run(source, escaped, { id: ids() }), "tooLarge"); unchanged(source, before);
  const ordinary = await run(source, Array.from({ length: 16 }, (_, index) => file(`text-${index}.txt`, "x".repeat(MAX_TEXT_FILE_BYTES))), { id: ids() });
  assert.ok(encoder.encode(serializeTaskDraft(ordinary)).byteLength < MAX_TASK_DRAFT_BYTES);
});

test("selection metadata is captured once before reads and asynchronous files stay ordered", { timeout: 3000 }, async () => {
  const first = held("first.md"), second = file("original.py", "x"), third = file("third.txt", "last");
  const files = [first.file, second, third], source = draft(), before = serializeTaskDraft(source);
  const pending = run(source, files, { id: ids() }); await started(pending, first);
  second.name = "changed.json"; second.size = MAX_TEXT_FILE_BYTES + 1; files.reverse();
  first.resolve("a"); const next = await pending;
  assert.deepEqual(next.payload.slice(1).map(entry => [entry.filename, entry.language, entry.text]),
    [["first.md", "markdown", "a"], ["original.py", "python", "x"], ["third.txt", "plaintext", "last"]]);
  unchanged(source, before);
});

test("pre-aborted input and failed initial review do no reads or ID allocation", async () => {
  const source = draft(), before = serializeTaskDraft(source), parent = new AbortController(); parent.abort();
  let reads = 0, allocations = 0;
  const value = file("untouched.txt", "x", async () => { reads++; return buffer("x"); });
  const id = () => { allocations++; return "new"; };
  await rejected(run(source, [value], { signal: parent.signal, id }), "stale");
  await rejected(run(source, [value], { id, guard: () => { throw new Error("translated old review"); } }), "stale");
  assert.equal(reads, 0); assert.equal(allocations, 0); unchanged(source, before);
});

test("abort during a pending read rejects immediately and late buffers never continue the batch", { timeout: 3000 }, async t => {
  const source = draft(), before = serializeTaskDraft(source), first = held(), parent = new AbortController();
  let laterReads = 0, allocations = 0;
  const add = t.mock.method(parent.signal, "addEventListener"), remove = t.mock.method(parent.signal, "removeEventListener");
  const pending = run(source, [first.file, file("later.txt", "later", async () => { laterReads++; return buffer("later"); })],
    { signal: parent.signal, id: () => { allocations++; return "new"; } });
  const stopped = rejected(pending, "stale"); await started(pending, first); parent.abort(); await stopped;
  first.resolve("x"); await Promise.resolve(); await Promise.resolve();
  assert.equal(laterReads, 0); assert.equal(allocations, 0); unchanged(source, before);
  assert.ok(add.mock.callCount() > 0); assert.equal(remove.mock.callCount(), add.mock.callCount());
});

test("review is rechecked after awaits and ID callbacks and just before exposing a new draft", { timeout: 3000 }, async () => {
  const source = draft(), before = serializeTaskDraft(source), first = held(); let current = true, allocations = 0;
  const guard = () => { if (!current) throw new Error("localized stale review"); };
  const pending = run(source, [first.file], { guard, id: () => { allocations++; return "new"; } });
  const stopped = rejected(pending, "stale"); await started(pending, first); current = false; first.resolve("x"); await stopped;
  assert.equal(allocations, 0); unchanged(source, before);
  current = true;
  await rejected(run(source, [file("fresh.txt", "x")], { guard, id: () => { current = false; return "new"; } }), "stale");
  unchanged(source, before);
});

test("one fixed deadline covers successive reads and late timeout results allocate no IDs", { timeout: 3000 }, async t => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1000 });
  const source = draft(), before = serializeTaskDraft(source), first = held("first.txt"), second = held("second.txt");
  let allocations = 0, laterReads = 0;
  const pending = run(source, [first.file, second.file, file("third.txt", "x", async () => { laterReads++; return buffer("x"); })],
    { id: () => { allocations++; return "new"; } });
  const stopped = rejected(pending, "readFailed"); await started(pending, first);
  t.mock.timers.tick(6000); first.resolve("a"); await started(pending, second);
  t.mock.timers.tick(3999); assert.equal(second.file.reads, 1);
  t.mock.timers.tick(1); await stopped; second.resolve("b"); await Promise.resolve(); await Promise.resolve();
  assert.equal(allocations, 0); assert.equal(laterReads, 0); unchanged(source, before);
});

test("elapsed synchronous preparation is checked before a result escapes the fixed batch deadline", async t => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1000 });
  const source = draft(), before = serializeTaskDraft(source);
  await rejected(run(source, [file("fresh.txt", "x")], { id: () => {
    t.mock.timers.tick(TEXT_FILE_IMPORT_TIMEOUT_MS); return "new";
  } }), "readFailed"); unchanged(source, before);
});

test("success and failure remove abort listeners and deadlines without network or profile access", async t => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1000 });
  const network = t.mock.method(globalThis, "fetch", () => { throw new Error("text import must remain local"); });
  for (const fails of [false, true]) {
    const parent = new AbortController(), source = draft(), before = serializeTaskDraft(source);
    const add = t.mock.method(parent.signal, "addEventListener"), remove = t.mock.method(parent.signal, "removeEventListener");
    const files = [file("local.txt", "x", () => fails ? Promise.reject(new Error("private read")) : Promise.resolve(buffer("x")))];
    const pending = run(source, files, { signal: parent.signal, id: ids() });
    if (fails) await rejected(pending, "readFailed"); else assert.equal((await pending).payload.length, 2);
    assert.ok(add.mock.callCount() > 0); assert.equal(remove.mock.callCount(), add.mock.callCount());
    parent.abort(); t.mock.timers.tick(TEXT_FILE_IMPORT_TIMEOUT_MS); unchanged(source, before);
  }
  assert.equal(network.mock.callCount(), 0);
});
