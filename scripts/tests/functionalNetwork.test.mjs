import assert from "node:assert/strict";
import test from "node:test";

import { frontendPageEntries, validateNetwork } from "../check-functional-network.mjs";

function fixture() {
  const network = {
    snapshot: { version: "0.3.8" },
    modules: [{ id: "M00" }],
    workflows: [{ id: "F01", module: "M00", sources: ["source.rs"], api: ["GET /health"] }],
    edges: [],
    api_operations: [{ operation: "GET /health", access: "public", workflow: "F01" }],
    pages: [{ route: "/", source: "page.tsx", workflows: ["F01"] }],
    catalogs: {
      templates: [{ id: "sample", source: "source.rs" }],
      labs: [{ id: "lab", source: "source.rs" }],
      bundled_modules: [{ name: "module-ebpf", source: "source.rs" }],
    },
    source_inputs: [{ path: "source.rs", sha256: "abc" }, { path: "page.tsx", sha256: "def" }],
    counts: { modules: 1, workflows: 1, edges: 0, api_operations: 1, pages: 1, templates: 1, labs: 1, bundled_modules: 1 },
  };
  const inputs = {
    version: "0.3.8",
    operations: ["GET /health"],
    access: { "GET /health": "public" },
    pages: [{ route: "/", source: "page.tsx" }],
    templates: ["sample"],
    labs: ["lab"],
    bundledModules: ["module-ebpf"],
    sourceHashes: { "source.rs": "abc", "page.tsx": "def" },
  };
  return { network, inputs };
}

test("functional network enumerates default Next page extensions and excludes framework files", () => {
  assert.deepEqual(frontendPageEntries([
    "frontend/pages/_app.tsx", "frontend/pages/_error.js", "frontend/pages/index.tsx",
    "frontend/pages/learn/index.jsx", "frontend/pages/learn/[...slug].ts", "frontend/pages/new.js",
    "frontend/pages/note.md",
  ]), [
    { route: "/", source: "frontend/pages/index.tsx" },
    { route: "/learn", source: "frontend/pages/learn/index.jsx" },
    { route: "/learn/[...slug]", source: "frontend/pages/learn/[...slug].ts" },
    { route: "/new", source: "frontend/pages/new.js" },
  ]);
});

test("functional network accepts a complete source snapshot", () => {
  const { network, inputs } = fixture();
  assert.deepEqual(validateNetwork(network, inputs), []);
});

test("functional network rejects missing, extra and duplicate operation ownership", () => {
  const { network, inputs } = fixture();
  network.workflows[0].api.push("GET /health", "POST /unknown");
  inputs.operations.push("GET /new");
  const errors = validateNetwork(network, inputs).join("\n");
  assert.match(errors, /duplicate.*GET \/health/);
  assert.match(errors, /missing.*GET \/new/);
  assert.match(errors, /extra.*POST \/unknown/);
});

test("functional network rejects unknown graph, module and page targets", () => {
  const { network, inputs } = fixture();
  network.workflows[0].module = "M99";
  network.pages[0].workflows = ["F99"];
  network.edges.push({ id: "E01", from: "F01", to: "F99" });
  const errors = validateNetwork(network, inputs).join("\n");
  assert.match(errors, /unknown module M99/);
  assert.match(errors, /unknown workflow F99/);
});

test("functional network detects version, source, access, page and catalog drift", () => {
  const { network, inputs } = fixture();
  inputs.version = "0.3.9";
  inputs.sourceHashes["source.rs"] = "changed";
  inputs.access["GET /health"] = "authenticated";
  inputs.pages = [{ route: "/new", source: "new.tsx" }];
  inputs.templates.push("new-template");
  const errors = validateNetwork(network, inputs).join("\n");
  for (const pattern of [/version drift/, /source drift/, /access drift/, /page.*missing/, /template.*missing/]) {
    assert.match(errors, pattern);
  }
});

test("functional network rejects stale counts and source references missing from manifest", () => {
  const { network, inputs } = fixture();
  network.counts.workflows = 10;
  network.workflows[0].sources.push("untracked.rs");
  const errors = validateNetwork(network, inputs).join("\n");
  assert.match(errors, /count drift.*workflows/);
  assert.match(errors, /missing source manifest.*untracked.rs/);
});

test("functional network rejects mismatched API back-references and duplicate IDs", () => {
  const { network, inputs } = fixture();
  network.api_operations[0].workflow = "F99";
  network.modules.push({ id: "M00" });
  const errors = validateNetwork(network, inputs).join("\n");
  assert.match(errors, /operation ownership drift/);
  assert.match(errors, /duplicate module M00/);
});
