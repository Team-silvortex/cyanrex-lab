import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { assertDocumentationLinks, assertDocumentationReference, documentationReferencePath,
  parseDocumentation } from "./helpers/documentation-navigation.mjs";

async function fixture(t, files) {
  const rootDirectory = await mkdtemp(path.join(os.tmpdir(), "cyanrex-doc-navigation-"));
  t.after(() => rm(rootDirectory, { recursive: true, force: true }));
  for (const [file, contents] of Object.entries(files)) {
    await mkdir(path.dirname(path.join(rootDirectory, file)), { recursive: true });
    await writeFile(path.join(rootDirectory, file), contents);
  }
  return { rootDirectory };
}

test("documentation references follow real relative links through at most two files", async t => {
  const options = await fixture(t, {
    "README.md": "[Topics](docs/reference-index.md)\n",
    "docs/reference-index.md": "[Guide](details/guide.md#usage)\n",
    "docs/details/guide.md": "# Guide\n\n## Usage\n",
  });
  assert.deepEqual(await assertDocumentationReference("README.md", "docs/details/guide.md", options),
    ["README.md", "docs/reference-index.md", "docs/details/guide.md"]);
  assert.deepEqual(await assertDocumentationReference("docs/reference-index.md", "docs/details/guide.md#usage", options),
    ["docs/reference-index.md", "docs/details/guide.md"]);
});

test("the depth bound and exact relative path cannot be replaced by a basename or catalogue text", async t => {
  const options = await fixture(t, {
    "README.md": "[First](a.md)\n",
    "a.md": "[Second](docs/index.md)\n",
    "docs/index.md": "[Guide](guide.md)\n",
    "docs/guide.md": "# Guide\n",
    "guide.md": "# Different file\n",
  });
  assert.equal(await documentationReferencePath("README.md", "docs/guide.md", options), null);
  assert.equal(await documentationReferencePath("docs/index.md", "guide.md", options), null);
  await assert.rejects(assertDocumentationReference("README.md", "docs/guide.md", options), /no bounded Markdown navigation/);
  await assert.rejects(documentationReferencePath("README.md", "docs/guide.md", { ...options, maxDepth: 3 }), /depth/);
});

test("cycles terminate and broken intermediate links cannot establish a route", async t => {
  const options = await fixture(t, {
    "README.md": "[Loop](cycle.md)\n[Missing](missing.md)\n",
    "cycle.md": "[Back](README.md)\n",
    "guide.md": "# Guide\n",
  });
  assert.equal(await documentationReferencePath("README.md", "guide.md", options), null);
  await assert.rejects(assertDocumentationLinks("README.md", options), /missing\.md/);
});

test("comments fenced code indented code inline code images escapes and external links are not navigation", async t => {
  const options = await fixture(t, {
    "README.md": ["<!-- [one](guide.md) -->", "~~~markdown", "[two](guide.md)", "~~~",
      "```", "[three](guide.md)", "```", "    [four](guide.md)", "`[five](guide.md)`",
      "![image](guide.md)", "\\[escaped](guide.md)", "[external](https://example.invalid/guide.md)",
      "[protocol relative](//example.invalid/guide.md)", "[mail](mailto:guide.md)",
      "The unlinked filename guide.md is not an edge."].join("\n"),
    "guide.md": "# Guide\n",
  });
  assert.equal(await documentationReferencePath("README.md", "guide.md", options), null);
  await assertDocumentationLinks("README.md", options);
});

test("nested image labels and raw HTML code blocks cannot manufacture Markdown edges", async t => {
  const options = await fixture(t, {
    "README.md": "![image [fake](guide.md)](image.png)\n"
      + ["pre", "script", "style", "textarea"].map(tag => `<${tag}>\n[fake](guide.md)\n</${tag}>`).join("\n"),
    "guide.md": "# Guide\n",
  });
  assert.equal(await documentationReferencePath("README.md", "guide.md", options), null);
  await assertDocumentationLinks("README.md", options);
});

test("HTML attribute text and raw blocks cannot create Markdown links or fragment anchors", async t => {
  const options = await fixture(t, {
    "README.md": '<span title="[fake](guide.md)">label</span>\n\n'
      + ["div", "custom-widget"].map(tag => `<${tag}>\n[fake](guide.md)\n</${tag}>\n`).join("\n"),
    "guide.md": '<span title=\'id="fake"\'>label</span>\n\n'
      + ["div", "custom-widget"].map(tag => `<${tag}>\n# Fake\n</${tag}>\n`).join("\n") + "\n## Real\n",
    "broken.md": "[Fake](guide.md#fake)\n", "positive.md": "[Real](guide.md#real)\n",
  });
  assert.equal(await documentationReferencePath("README.md", "guide.md", options), null);
  await assert.rejects(assertDocumentationLinks("broken.md", options), /fake/);
  await assertDocumentationReference("positive.md", "guide.md#real", options);
});

test("reference links angle destinations titles escapes queries and percent-encoding resolve", async t => {
  const options = await fixture(t, {
    "README.md": "[Inline](<docs/a%20guide.md#usage> \"Guide title\")\n[Full][guide]\n[collapsed][]\n[shortcut]\n"
      + "[guide]: docs/a%20guide.md#usage 'Title'\n[collapsed]: docs/a%20guide.md?raw=1#usage\n[shortcut]: docs/a%20guide.md#usage\n",
    "docs/a guide.md": "# A guide\n\n## Usage\n",
  });
  assert.equal(parseDocumentation(await readFile(path.join(options.rootDirectory, "README.md"), "utf8")).links.length, 4);
  await assertDocumentationReference("README.md", "docs/a guide.md#usage", options);
  await assertDocumentationLinks("README.md", options);
});

test("fragments require real headings duplicate slugs Unicode headings or explicit anchors", async t => {
  const options = await fixture(t, {
    "README.md": "[First](guide.md#hello-world) [Second](guide.md#hello-world-1) [Unicode](guide.md#%E4%BD%BF%E7%94%A8)\n"
      + "[Explicit](guide.md#stable-api) [Setext](guide.md#setext) [Code](guide.md#inline-code)\n",
    "guide.md": "# Hello World!\n## Hello World!\n## 使用\n<a id=\"stable-api\"></a>\nSetext\n------\n## Inline `code`\n",
    "broken.md": "[Gone](guide.md#does-not-exist)\n",
    "other-fragment.md": "[Other](guide.md#hello-world)\n",
  });
  await assertDocumentationLinks("README.md", options);
  await assert.rejects(assertDocumentationLinks("broken.md", options), /does-not-exist/);
  assert.equal(await documentationReferencePath("broken.md", "guide.md", options), null);
  assert.equal(await documentationReferencePath("other-fragment.md", "guide.md#stable-api", options), null);
});

test("headings and anchors hidden in comments or code cannot satisfy a fragment", async t => {
  const options = await fixture(t, {
    "README.md": "[Fake](guide.md#fake)\n",
    "guide.md": "<!-- <a id=\"fake\"></a> -->\n```md\n# Fake\n```\n`<a id=\"fake\"></a>`\n",
  });
  await assert.rejects(assertDocumentationLinks("README.md", options), /fake/);
});

test("file count bytes and repository containment are enforced including symlink escapes", async t => {
  const options = await fixture(t, {
    "README.md": "[A](a.md) [B](b.md)\n",
    "a.md": "[Target](guide.md)\n", "b.md": "# B\n", "guide.md": "# Guide\n",
    "escape.md": "[Outside](../outside.md) [Encoded](%2e%2e/outside.md)\n",
  });
  await assert.rejects(documentationReferencePath("README.md", "guide.md", { ...options, maxDocuments: 1 }), /document limit/);
  await assert.rejects(documentationReferencePath("README.md", "guide.md", { ...options, maxBytes: 4 }), /byte limit/);
  await assert.rejects(assertDocumentationLinks("escape.md", options), /outside repository/);
  await assert.rejects(documentationReferencePath("../outside.md", "guide.md", options), /outside repository/);
  const outside = await fixture(t, { "outside.md": "# Outside\n" });
  await symlink(path.join(outside.rootDirectory, "outside.md"), path.join(options.rootDirectory, "linked.md"));
  await assert.rejects(assertDocumentationReference("README.md", "linked.md", options), /outside repository/);
});
