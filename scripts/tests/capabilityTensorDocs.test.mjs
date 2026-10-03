import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { checkCapabilityTensor, renderTensorRows } from "../check-capability-tensor.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

test("the maintained capability tensor has valid source, test and evidence references", async () => {
  const result = await checkCapabilityTensor(root);
  assert.ok(result.counts.cells > 0);
});

for (const locale of ["en", "zh-CN"]) {
  test(`the ${locale} capability reading view matches the machine coordinates and scores`, async () => {
    const data = JSON.parse(await readFile(path.join(root, "docs/platform-capability-tensor.json"), "utf8"));
    const document = await readFile(path.join(root, `docs/${locale}/capability-maturity.md`), "utf8");
    const start = "<!-- capability-tensor:start -->";
    const end = "<!-- capability-tensor:end -->";
    assert.equal(document.split(start).length, 2, "exactly one generated view start marker");
    assert.equal(document.split(end).length, 2, "exactly one generated view end marker");
    assert.ok(document.indexOf(start) < document.indexOf(end), "ordered view markers");
    const actual = document.slice(document.indexOf(start) + start.length, document.indexOf(end)).trim();
    assert.equal(actual, renderTensorRows(data, locale));
  });
}
