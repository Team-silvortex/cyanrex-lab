import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import {
  MONACO_SANITIZER, prepareMonacoSanitizer, publishMonacoAssets, replaceBundledSanitizer,
  rewriteChunkReferences, wrapOfficialSanitizer,
} from "../scripts/monaco-sanitizer.mjs";

const root = new URL("../", import.meta.url);
const fixture = `/*! @license DOMPurify 3.4.16 | fixture */
const outside = "private";
function createDOMPurify() { const DOMPurify = {}; DOMPurify.version = "3.4.16"; return DOMPurify; }
var purify = createDOMPurify();
export { purify as default };
//# sourceMappingURL=purify.es.mjs.map
`;

test("the official ESM default is isolated inside one expression without leaking bindings", () => {
  const wrapped = wrapOfficialSanitizer(fixture);
  const context = vm.createContext({ outside: "unchanged" });
  const result = vm.runInContext(`var K_ = ${wrapped}; K_.version`, context);
  assert.equal(result, "3.4.16");
  assert.equal(context.outside, "unchanged");
  assert.equal(context.purify, undefined);
  assert.equal(context.createDOMPurify, undefined);
  assert.doesNotMatch(wrapped, /sourceMappingURL/);
  assert.match(wrapped, /@license DOMPurify 3\.4\.16/);
});

function publicationFixture(failure) {
  const root = "/fixture/frontend", target = `${root}/public/monaco/vs`, stage = `${root}/.monaco-assets-owned`;
  const files = new Map([[`${target}/existing.js`, "existing public bytes"]]), events = [];
  const chunkFile = "editor.api-cyanrex-0123456789abcdef.js";
  const prepared = { chunkFile, replacements: new Map([[chunkFile, "patched sanitizer"], ["editor/editor.main.js", "patched reference"]]) };
  const filesystem = {
    async mkdtemp(prefix) { events.push(["mkdtemp", prefix]); assert.equal(prefix, `${root}/.monaco-assets-`); return stage; },
    async mkdir(directory) { events.push(["mkdir", directory]); },
    async cp(source, destination) {
      events.push(["cp", source, destination]);
      if (failure === "copy") throw new Error("fixture copy failed");
      files.set(`${destination}/${MONACO_SANITIZER.originalFile}`, "old sanitizer");
      files.set(`${destination}/editor/editor.main.js`, "old reference");
    },
    async rm(filename, options) {
      events.push(["rm", filename, options]);
      files.delete(filename);
      if (options?.recursive) for (const key of files.keys()) if (key.startsWith(`${filename}/`)) files.delete(key);
    },
    async writeFile(filename, value) {
      events.push(["write", filename]);
      if (failure === "write") throw new Error("fixture write failed");
      files.set(filename, value);
    },
    async rename(source, destination) {
      events.push(["rename", source, destination]);
      assert.equal(files.get(`${source}/${chunkFile}`), "patched sanitizer");
      assert.equal(files.get(`${source}/editor/editor.main.js`), "patched reference");
      assert.equal(files.has(`${source}/${MONACO_SANITIZER.originalFile}`), false);
      for (const [key, value] of [...files]) if (key.startsWith(`${source}/`)) {
        files.set(`${destination}${key.slice(source.length)}`, value); files.delete(key);
      }
    },
  };
  return { root, target, stage, files, events, prepared, filesystem };
}

test("preparation, private copy and private write failures leave existing public assets untouched", async () => {
  for (const failure of ["prepare", "copy", "write"]) {
    const f = publicationFixture(failure);
    const prepare = async () => { if (failure === "prepare") throw new Error("fixture prepare failed"); return f.prepared; };
    await assert.rejects(publishMonacoAssets(f.root, prepare, f.filesystem), /fixture .* failed/);
    assert.deepEqual([...f.files], [[`${f.target}/existing.js`, "existing public bytes"]]);
    assert.equal(f.events.some(([operation, target]) => ["rm", "write", "rename", "mkdir"].includes(operation)
      && target.startsWith(`${f.root}/public`)), false, "public output must remain untouched before preparation completes");
    if (failure === "prepare") assert.deepEqual(f.events, []);
    else assert.ok(f.events.some(([operation, target]) => operation === "rm" && target === f.stage), "owned staging tree is cleaned");
  }
});

test("publication moves only the fully patched private stage and removes no unrelated paths", async () => {
  const f = publicationFixture();
  assert.equal(await publishMonacoAssets(f.root, async () => f.prepared, f.filesystem), f.prepared);
  assert.deepEqual([...f.files].sort(), [
    [`${f.target}/${f.prepared.chunkFile}`, "patched sanitizer"],
    [`${f.target}/editor/editor.main.js`, "patched reference"],
  ].sort());
  assert.ok(f.events.some(([operation, from, to]) => operation === "rename" && from === `${f.stage}/vs` && to === f.target));
  for (const [operation, target] of f.events) if (operation === "rm") {
    assert.ok([f.target, f.stage, `${f.stage}/vs/${MONACO_SANITIZER.originalFile}`].includes(target));
  }
  const lastWrite = f.events.findLastIndex(([operation]) => operation === "write");
  assert.ok(f.events.findIndex(([operation, target]) => operation === "rm" && target === f.target) > lastWrite);
  assert.deepEqual(f.events.at(-1), ["rm", f.stage, { recursive: true, force: true }]);
});

test("unexpected official versions, imports, missing or multiple exports and markers fail closed", () => {
  for (const bad of [
    fixture.replaceAll("3.4.16", "3.4.15"),
    fixture.replace('DOMPurify.version = "3.4.16"', 'DOMPurify.version = "3.4.15"'),
    fixture.replace("export { purify as default };", ""),
    fixture.replace("export { purify as default };", "export { purify };"),
    `${fixture}\nexport const extra = 1;`,
    `import value from "remote";\n${fixture}`,
    fixture.replace("var purify", 'import("remote"); var purify'),
    fixture.replace("/*! @license DOMPurify", "/*! different library"),
    `/*! @license DOMPurify 3.4.16 | duplicate */\n${fixture}`,
  ]) assert.throws(() => wrapOfficialSanitizer(bad));
});

test("AMD module references are replaced only as complete string literal values", () => {
  const stem = MONACO_SANITIZER.originalFile.slice(0, -3), next = "editor.api-cyanrex-0123456789abcdef";
  const input = `define("vs/${stem}",["./${stem}","../../${stem}"],function(){return 7});`;
  assert.equal(rewriteChunkReferences(input, next), input.replaceAll(stem, next));
  for (const bad of [
    `/* ${stem} */ define("safe",[],()=>{});`,
    `define("prefix-${stem}",[],()=>{});`,
    `define("./${stem}?old",[],()=>{});`,
    `const label = \`${stem}\`;`,
  ]) assert.throws(() => rewriteChunkReferences(bad, next));
});

test("a changed upstream chunk never reaches the vendor replacement", async () => {
  const original = await readFile(new URL(`node_modules/monaco-editor/min/vs/${MONACO_SANITIZER.originalFile}`, root), "utf8");
  assert.equal(createHash("sha256").update(original).digest("hex"), MONACO_SANITIZER.originalSha256);
  for (const bad of [
    `${original}\n`,
    original.replace("DOMPurify 3.2.7", "DOMPurify 3.2.8"),
    original.replace("@license DOMPurify", "@license other"),
    original.replace("var K_=pq();", "var other=pq();"),
    `${original}\n/*! @license DOMPurify 3.2.7 */`,
  ]) assert.throws(() => replaceBundledSanitizer(bad, fixture));
});

test("the exact supported bundle keeps every byte outside the vendor block", async () => {
  const original = await readFile(new URL(`node_modules/monaco-editor/min/vs/${MONACO_SANITIZER.originalFile}`, root), "utf8");
  const result = replaceBundledSanitizer(original, fixture);
  const start = original.indexOf("/*! @license DOMPurify"), end = original.indexOf("var K_=pq();") + "var K_=pq();".length;
  assert.equal(result.slice(0, start), original.slice(0, start));
  assert.ok(result.endsWith(original.slice(end)));
  assert.equal(result.slice(start, result.length - original.slice(end).length), `var K_ = ${wrapOfficialSanitizer(fixture)};`);
  assert.doesNotMatch(result, /DOMPurify 3\.2\.7|version="3\.2\.7"/);
});

test("real installed assets get a new deterministic chunk URL and no stale AMD references", async () => {
  const base = new URL("node_modules/monaco-editor/min/vs/", root), files = new Map();
  async function collect(directory, prefix = "") {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const relative = `${prefix}${entry.name}`, url = new URL(entry.name, directory);
      if (entry.isDirectory()) await collect(new URL(`${entry.name}/`, directory), `${relative}/`);
      else if (entry.name.endsWith(".js")) files.set(relative, await readFile(url, "utf8"));
    }
  }
  await collect(base);
  const dompurifySource = await readFile(new URL("node_modules/dompurify/dist/purify.es.mjs", root), "utf8");
  const options = { files, dompurifySource, monacoVersion: "0.55.1", dompurifyVersion: "3.4.16" };
  const result = prepareMonacoSanitizer(options);
  assert.match(result.chunkFile, /^editor\.api-cyanrex-[a-f0-9]{16}\.js$/);
  assert.equal(result.replacements.size, 27);
  assert.equal(result.replacements.has(MONACO_SANITIZER.originalFile), false);
  assert.ok(result.replacements.has(result.chunkFile));
  assert.equal(prepareMonacoSanitizer(options).chunkFile, result.chunkFile);
  for (const source of result.replacements.values()) assert.ok(!source.includes(MONACO_SANITIZER.originalFile.slice(0, -3)));
  assert.match(result.replacements.get("editor/editor.main.js"), new RegExp(path.basename(result.chunkFile, ".js")));
  assert.match(result.replacements.get(result.chunkFile), /DOMPurify 3\.4\.16/);
  for (const change of [{ monacoVersion: "0.57.0" }, { dompurifyVersion: "3.4.15" }, { files: new Map() }]) {
    assert.throws(() => prepareMonacoSanitizer({ ...options, ...change }));
  }
  const missing = new Map(files); missing.delete("editor/editor.main.js");
  assert.throws(() => prepareMonacoSanitizer({ ...options, files: missing }));
});
