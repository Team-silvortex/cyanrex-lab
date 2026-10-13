import assert from "node:assert/strict";
import { readFile, readdir, access } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = new URL("../../", import.meta.url).pathname;
const companions = ["README", "CONTRIBUTING", "SECURITY", "CODE_OF_CONDUCT", "TDD",
  "frontend/README", "engine/README", "sdk-js/README", "sdk-js/STABILITY", "scripts/README",
  "docker/README", "docker/README-DEPLOY", "modules/README", "modules/module-network/README",
  "modules/module-protocol/README", "modules/module-ebpf/README"];
const read = name => readFile(path.join(root, name), "utf8");
const fences = text => [...text.matchAll(/^```([^\n]*)\n([\s\S]*?)^```\s*$/gm)].map(match => [match[1], match[2]]);
const headings = text => (text.replace(/^```[^\n]*\n[\s\S]*?^```\s*$/gm, "").match(/^#{1,6} /gm) || []).length;

test("public Chinese companion guides preserve complete section and executable-example inventories", async () => {
  for (const name of companions) {
    const [source, translated] = await Promise.all([read(`${name}.md`), read(`${name}.zh-CN.md`)]);
    assert.equal(headings(translated), headings(source), `${name}: every section remains represented`);
    assert.deepEqual(fences(translated), fences(source), `${name}: commands and examples are unchanged`);
    assert.match(translated, /[\u4e00-\u9fff]/u, `${name}: prose is translated`);
  }
});

test("all bilingual documentation themes have a counterpart including the historical architecture proposal", async () => {
  async function files(directory, prefix = "") {
    const result = [];
    for (const entry of await readdir(path.join(root, directory), { withFileTypes: true })) {
      const name = path.posix.join(prefix, entry.name);
      if (entry.isDirectory()) result.push(...await files(path.posix.join(directory, entry.name), name));
      else if (name.endsWith(".md")) result.push(name);
    }
    return result.sort();
  }
  assert.deepEqual(await files("docs/en"), await files("docs/zh-CN"));
  const [source, translated] = await Promise.all([read("docs/zh-CN/next-architecture.md"), read("docs/en/next-architecture.md")]);
  assert.equal(headings(translated), headings(source));
  // Architecture diagrams are explanatory text, not executable examples: translate labels,
  // but retain the exact graph IDs/edges and the original state sequence.
  const labels = {
    "协作 GUI / 领域工作台": "Collaboration GUI / domain workspace",
    "API / 语义命令 / Lese 适配": "API / semantic commands / Lese adapter",
    "Artifact / Task / Review 服务": "Artifact / Task / Review services",
    "Run 编排与租约": "Run orchestration and leases",
    "事务数据库与内容存储": "Transactional database and content storage",
    "Outbox / 业务事件流": "Outbox / business event stream",
    "Worker 协议与资源网关": "Worker protocol and resource gateway",
    "执行节点与 EnvironmentInstance": "Execution nodes and EnvironmentInstance",
    "eBPF 领域执行器": "eBPF domain executor",
    "构建 / 测试 / 仿真 / yalivia 适配": "Build / test / simulation / yalivia adapter",
    "教学与其他领域包": "Teaching and other domain packages",
    "已核实终态": "verified terminal state",
  };
  assert.deepEqual(fences(translated), fences(source).map(([language, body]) => [language,
    Object.entries(labels).reduce((text, [original, english]) => text.replaceAll(original, english), body)]));
  assert.doesNotMatch(translated, /[\u4e00-\u9fff]/u);
  for (const token of ["f36f2bdcd4732f6eb78a1631520f9e8143f5cb4d", "2026-09-27", "2026-10-03", "0.4.3", "0.4.9", "0.5.4"]) assert.ok(translated.includes(token), token);
});

test("translated public guides resolve local Markdown destinations rather than orphaning their navigation", async () => {
  for (const name of companions) {
    const file = `${name}.zh-CN.md`, text = await read(file);
    for (const match of text.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
      const target = match[1].split("#")[0];
      if (!target.endsWith(".md") || /^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
      await access(path.resolve(root, path.dirname(file), target));
    }
  }
});
