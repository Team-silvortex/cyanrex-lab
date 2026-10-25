import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { assertDocumentationLinks, assertDocumentationReference } from "./helpers/documentation-navigation.mjs";

const root = new URL("../../", import.meta.url);
const read = file => readFile(new URL(file, root), "utf8");
const structure = source => [...source.matchAll(/^(#{1,6}) /gm)].map(match => match[1].length);
const localizedPath = file => file.replace(/^docs\/(?:en|zh-CN)\//, "docs/LOCALE/").replace(/\.zh-CN\.md$/, ".md");

async function inventory(directory) {
  const result = [];
  for (const entry of await readdir(new URL(`${directory}/`, root), { withFileTypes: true })) {
    const file = path.posix.join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await inventory(file));
    else if (entry.isFile() && entry.name.endsWith(".md")) result.push(file);
  }
  return result.sort();
}

test("documentation entry pages remain bounded while English and Chinese retain the same structure", async () => {
  for (const [file, maximum] of [["README.md", 220], ["README.zh-CN.md", 220],
    ...["en", "zh-CN"].flatMap(locale => [[`docs/${locale}/README.md`, 160],
      [`docs/${locale}/project-status.md`, 320], [`docs/${locale}/reference-index.md`, 320]])]) {
    assert.ok((await read(file)).trimEnd().split("\n").length <= maximum, `${file}: entry page exceeds ${maximum} lines`);
  }
  for (const name of ["README", "reference-index"])
    assert.deepEqual(structure(await read(`docs/en/${name}.md`)), structure(await read(`docs/zh-CN/${name}.md`)), name);
});

test("the bilingual topic indexes explicitly cover every documentation file without duplicate destinations", async () => {
  const byLocale = {};
  for (const locale of ["en", "zh-CN"]) {
    const index = `docs/${locale}/reference-index.md`, files = await inventory(`docs/${locale}`);
    const links = await assertDocumentationLinks(index);
    const targets = links.filter(link => link.file.startsWith(`docs/${locale}/`)).map(link => link.file);
    assert.equal(new Set(targets).size, targets.length, `${locale}: repeated local topic links`);
    assert.deepEqual([...targets].sort(), files.filter(file => file !== index), `${locale}: every source topic and lab has a real direct link`);
    byLocale[locale] = links.map(link => localizedPath(link.file)).sort();
  }
  assert.deepEqual(byLocale.en, byLocale["zh-CN"], "translated indexes retain the same exact target inventory");
});

test("every documentation path is accepted by the real course route segment policy", async () => {
  const route = await read("frontend/pages/learn/[...slug].tsx");
  assert.match(route, /value\.every\(\s*\(?part\)?\s*=>\s*\/\^\[a-z0-9-\]\+\$\/i\.test\(part\)\s*\)/);
  for (const locale of ["en", "zh-CN"]) {
    for (const file of await inventory(`docs/${locale}`)) {
      const slug = file.slice(`docs/${locale}/`.length, -3);
      assert.ok(slug.split("/").every(part => /^[a-z0-9-]+$/i.test(part)), `${file}: rejected by the course route`);
    }
  }
});

test("every source guide remains reachable from its language and repository entry points in two real links", async () => {
  for (const locale of ["en", "zh-CN"]) {
    const entry = `docs/${locale}/README.md`, publicEntry = locale === "en" ? "README.md" : "README.zh-CN.md";
    for (const file of await inventory(`docs/${locale}`)) {
      await assertDocumentationReference(entry, file);
      await assertDocumentationReference(publicEntry, file);
    }
  }
});

test("source guides keep existing local links and fragment anchors valid instead of accepting filename mentions", async () => {
  const files = ["README.md", "README.zh-CN.md", ...await inventory("docs/en"), ...await inventory("docs/zh-CN")];
  const failures = [];
  for (const file of files) {
    try { await assertDocumentationLinks(file); }
    catch (error) { failures.push(`${file}: ${error.message}`); }
  }
  assert.deepEqual(failures, []);
});
