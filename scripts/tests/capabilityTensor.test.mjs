import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { checkCapabilityTensor, renderTensorRows, validateTensor } from "../check-capability-tensor.mjs";

const label = (en, zh = en) => ({ en, "zh-CN": zh });
function fixture() {
  return {
    schema_version: 1,
    snapshot: { date: "2026-10-03", source_version: "0.4.9", base_commit: "a".repeat(40),
      scope: "Reviewed working tree", verification: "Existing evidence; no new deployment acceptance" },
    axes: { architecture: [{ id: "A01", label: label("Browser", "浏览器") }],
      maturity: ["D", "C", "V", "O"].map(id => ({ id, label: label(id), levels: ["none", "contract", "module", "boundary"] })) },
    functions: [{ id: "F01", label: label("Edit local draft", "编辑本地草稿") },
      { id: "F02", label: label("Save server draft", "保存服务端草稿") }],
    implementations: [{ id: "I01", sources: ["src/editor.ts"], tests: ["tests/editor.test.mjs"] },
      { id: "I02", sources: [], tests: [] }],
    evidence: [{ id: "EV01", date: "2026-10-03", kind: "browser-record", reference: "docs/status.md", note: "Mock Engine responses" }],
    cells: [{ id: "S01", architecture: "A01", function: "F01", implementation: "I01", state: "local",
      scores: { D: 3, C: 2, V: 2, O: null }, evidence: ["EV01"], gap: label("No server persistence", "未接服务端保存") },
    { id: "S02", architecture: "A01", function: "F02", implementation: "I02", state: "planned",
      scores: { D: 0, C: 0, V: 0, O: 0 }, evidence: [], gap: label("Public adapter missing", "缺公共适配器") }],
    edges: [{ id: "E01", from: "S01", to: "S02", state: "missing", boundary: label("No save API", "没有保存 API") }],
    chains: [{ id: "CH01", label: label("Draft to save", "草稿到保存"), cells: ["S01", "S02"], edges: ["E01"],
      boundary: label("Local export is not server saving", "本地导出不等于服务端保存") }],
  };
}
const errorsFor = (change) => { const data = fixture(); change(data); return validateTensor(data).join("\n"); };

async function withRepository(action) {
  const root = await mkdtemp(path.join(os.tmpdir(), "cyanrex-capability-tensor-"));
  const data = fixture();
  try {
    for (const directory of ["docs", "src", "tests"]) await mkdir(path.join(root, directory));
    for (const file of ["src/editor.ts", "tests/editor.test.mjs", "docs/status.md"]) await writeFile(path.join(root, file), "fixture\n");
    const save = () => writeFile(path.join(root, "docs/platform-capability-tensor.json"), JSON.stringify(data));
    await save();
    await action(root, data, save);
  } finally { await rm(root, { recursive: true, force: true }); }
}

test("capability tensor accepts a sparse dated snapshot and unevaluated null scores", () => {
  assert.deepEqual(validateTensor(fixture()), []);
});

test("capability tensor rejects malformed schema, text, dates and score values without throwing", () => {
  for (const value of [null, [], {}, "invalid"]) assert.ok(validateTensor(value).length);
  for (const score of [-1, 4, 1.5, "2", undefined, NaN]) {
    assert.match(errorsFor(data => { data.cells[0].scores.D = score; }), /scores\.D/);
  }
  assert.match(errorsFor(data => { data.functions[0].label.en = "bad\uFFFD"; }), /label\.en/);
  assert.match(errorsFor(data => { data.functions[0].label.en = "bad\uD800"; }), /label\.en/);
  assert.match(errorsFor(data => { data.snapshot.date = "2026-02-30"; }), /snapshot.date/);
  assert.match(errorsFor(data => { delete data.cells[0].scores.O; }), /scores.O/);
  assert.match(errorsFor(data => { data.axes.maturity[0].levels = ["only one"]; }), /levels/);
});

test("capability tensor rejects duplicate IDs and duplicate architecture/function/implementation coordinates", () => {
  assert.match(errorsFor(data => { data.functions.push(data.functions[0]); }), /duplicate.*F01/);
  assert.match(errorsFor(data => { data.cells.push({ ...data.cells[0], id: "S03" }); }), /duplicate coordinate/);
});

test("capability tensor rejects missing implementations, unknown references and invalid connection states", () => {
  for (const [field, id] of [["architecture", "A99"], ["function", "F99"], ["implementation", "I99"]]) {
    assert.match(errorsFor(data => { data.cells[0][field] = id; }), new RegExp(`unknown.*${id}`));
  }
  assert.match(errorsFor(data => { data.cells[0].evidence = ["EV99"]; }), /unknown.*EV99/);
  assert.match(errorsFor(data => { data.edges[0].to = "S99"; }), /unknown.*S99/);
  assert.match(errorsFor(data => { data.edges[0].state = "maybe"; }), /state/);
});

test("capability tensor does not inflate planned or unsupported capabilities", () => {
  for (const [dimension, score] of [["D", 2], ["C", 1], ["V", 1]]) {
    assert.match(errorsFor(data => { data.cells[1].scores[dimension] = score; }), /planned/);
  }
  assert.match(errorsFor(data => { data.implementations[0].sources = []; }), /sources/);
  assert.match(errorsFor(data => { data.implementations[0].tests = []; }), /tests/);
  assert.match(errorsFor(data => { data.cells[0].evidence = []; }), /evidence/);
});

test("capability tensor rejects mocked browser evidence as current deployment acceptance", () => {
  for (const dimension of ["C", "O"]) {
    assert.match(errorsFor(data => { data.cells[0].scores[dimension] = 3; }), /current-deployment/);
    const data = fixture(); data.cells[0].scores[dimension] = 3; data.evidence[0].kind = "current-deployment";
    assert.deepEqual(validateTensor(data), []);
  }
});

test("capability tensor requires a test source for D=3 even when verification is unscored", () => {
  assert.match(errorsFor(data => {
    data.cells[0].scores.V = 0; data.implementations[0].tests = [];
  }), /D=3.*tests/);
});

test("capability tensor requires explicitly scoped integration evidence for V=3", () => {
  for (const kind of ["browser-record", "release-record", "historical-integration", "design"]) {
    assert.match(errorsFor(data => { data.cells[0].scores.V = 3; data.evidence[0].kind = kind; }), /V=3.*integration evidence/);
  }
  for (const kind of ["database-record", "service-integration", "current-deployment"]) {
    const data = fixture(); data.cells[0].scores.V = 3; data.evidence[0].kind = kind;
    assert.deepEqual(validateTensor(data), []);
  }
});

test("capability tensor never marks a planned endpoint as connected or internally joined", () => {
  for (const state of ["connected", "internal"]) {
    assert.match(errorsFor(data => { data.edges[0].state = state; }), /planned.*missing/);
    assert.match(errorsFor(data => { data.edges[0].state = state; data.edges[0].from = "S02"; data.edges[0].to = "S01"; }), /planned.*missing/);
  }
});

test("capability tensor checks ordered chain edges against adjacent cells", () => {
  assert.match(errorsFor(data => { data.chains[0].edges = ["E99"]; }), /unknown.*E99/);
  assert.match(errorsFor(data => { data.chains[0].edges = []; }), /chain.*edges/);
  assert.match(errorsFor(data => { data.chains[0].cells.reverse(); }), /chain.*adjacent/);
  assert.match(errorsFor(data => { data.chains[0].cells[1] = "S99"; }), /unknown.*S99/);
});

test("capability tensor permits an explicitly ordered feedback loop without losing edge validation", () => {
  const data = fixture();
  data.edges.push({ id: "E02", from: "S02", to: "S01", state: "missing", boundary: label("Read back") });
  data.chains[0].cells = ["S01", "S02", "S01", "S02"];
  data.chains[0].edges = ["E01", "E02", "E01"];
  assert.deepEqual(validateTensor(data), []);
});

test("capability tensor rejects non-repository and traversal path spellings", () => {
  for (const value of ["../secret", "src/../secret", "/tmp/secret", "C:\\secret", "src\\secret", "src//editor.ts", "./src/editor.ts"]) {
    assert.match(errorsFor(data => { data.implementations[0].sources = [value]; }), /repository path/);
  }
  assert.match(errorsFor(data => { data.evidence[0].reference = "https://example.com/report"; }), /repository path/);
});

test("capability tensor file check reports counts without comparing historical version to current Cargo", async () => {
  await withRepository(async (root) => {
    const result = await checkCapabilityTensor(root);
    assert.equal(result.source_version, "0.4.9");
    assert.equal(result.counts.cells, 2);
    assert.deepEqual(result.states, { live: 0, local: 1, prepared: 0, planned: 1 });
    await rm(path.join(root, "tests/editor.test.mjs"));
    await assert.rejects(checkCapabilityTensor(root), /missing.*tests\/editor.test.mjs/);
  });
});

test("capability tensor file check refuses symlink escape as well as lexical traversal", async () => {
  await withRepository(async (root, data, save) => {
    await symlink(os.tmpdir(), path.join(root, "outside"), "dir");
    data.implementations[0].sources = ["outside"];
    await save();
    await assert.rejects(checkCapabilityTensor(root), /outside repository/);
  });
});

test("capability tensor markdown renders bilingual coordinates, scores and source links safely", () => {
  const data = fixture();
  data.functions[0].label["zh-CN"] = "编辑 | <草稿>\n下一行";
  const rows = renderTensorRows(data, "zh-CN");
  assert.match(rows, /S01.*A01/);
  assert.doesNotMatch(rows, /F01|I01/);
  assert.match(rows, /编辑 \\?&#124; &lt;草稿&gt;<br>下一行/);
  assert.match(rows, /3 \/ 2 \/ 2 \/ —/);
  assert.match(rows, /\[editor\.ts\]\(\.\.\/\.\.\/src\/editor\.ts\)/);
  assert.match(rows, /证据/);
  assert.match(rows, /\[EV01\]\(\.\.\/\.\.\/docs\/status\.md\)/);
  assert.match(rows, /未接服务端保存/);
  assert.match(renderTensorRows(data, "en"), /No server persistence/);
  assert.match(renderTensorRows(data, "en"), /Evidence/);
});
