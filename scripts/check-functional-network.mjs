#!/usr/bin/env node

// Read-only drift check for the dated, manually reviewed functional-network snapshot.
import { createHash } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { checkRepositoryContract, extractEngineAccess, extractEngineOperations } from "./openapi-contract.mjs";

export function validateNetwork(network, inputs) {
  const errors = [];
  const unique = (label, values) => {
    const seen = new Set();
    for (const value of values) {
      if (seen.has(value)) errors.push(`duplicate ${label} ${value}`);
      seen.add(value);
    }
    return seen;
  };
  const compare = (label, expected, actual) => {
    const wanted = new Set(expected);
    const present = unique(label, actual);
    for (const value of wanted) if (!present.has(value)) errors.push(`${label} missing ${value}`);
    for (const value of present) if (!wanted.has(value)) errors.push(`${label} extra ${value}`);
  };
  const modules = unique("module", network.modules.map(item => item.id));
  const flows = unique("workflow", network.workflows.map(item => item.id));
  unique("edge", network.edges.map(item => item.id));
  const requireFlow = (id) => { if (!flows.has(id)) errors.push(`unknown workflow ${id}`); };
  const owner = new Map();
  for (const flow of network.workflows) {
    if (!modules.has(flow.module)) errors.push(`unknown module ${flow.module}`);
    for (const operation of flow.api) owner.set(operation, flow.id);
  }
  for (const edge of network.edges) {
    requireFlow(edge.from);
    requireFlow(edge.to);
  }
  for (const page of network.pages) {
    if (!page.workflows.length) errors.push(`page ${page.route} has no workflow`);
    page.workflows.forEach(requireFlow);
  }
  compare("workflow operation", inputs.operations, network.workflows.flatMap(item => item.api));
  compare("API operation", inputs.operations, network.api_operations.map(item => item.operation));
  for (const api of network.api_operations) {
    if (inputs.access[api.operation] !== api.access) errors.push(`access drift ${api.operation}`);
    if (owner.get(api.operation) !== api.workflow) errors.push(`operation ownership drift ${api.operation}`);
  }
  compare("page", inputs.pages.map(item => `${item.route}:${item.source}`),
    network.pages.map(item => `${item.route}:${item.source}`));
  compare("template", inputs.templates, network.catalogs.templates.map(item => item.id));
  compare("lab", inputs.labs, network.catalogs.labs.map(item => item.id));
  compare("bundled module", inputs.bundledModules, network.catalogs.bundled_modules.map(item => item.name));
  if (network.snapshot.version !== inputs.version) errors.push(`version drift: snapshot ${network.snapshot.version}, source ${inputs.version}`);

  const manifest = unique("source", network.source_inputs.map(item => item.path));
  const references = [
    ...network.workflows.flatMap(item => item.sources),
    ...network.pages.map(item => item.source),
    ...Object.values(network.catalogs).flatMap(items => items.map(item => item.source)),
    ...(network.storage_boundaries ?? []).flatMap(item => item.sources),
  ];
  for (const reference of new Set(references)) {
    if (!manifest.has(reference)) errors.push(`missing source manifest ${reference}`);
  }
  for (const source of network.source_inputs) {
    if (inputs.sourceHashes[source.path] !== source.sha256) errors.push(`source drift ${source.path}`);
  }
  const counts = {
    modules: network.modules.length, workflows: network.workflows.length, edges: network.edges.length,
    api_operations: network.api_operations.length, pages: network.pages.length,
    templates: network.catalogs.templates.length, labs: network.catalogs.labs.length,
    bundled_modules: network.catalogs.bundled_modules.length,
  };
  for (const [key, value] of Object.entries(counts)) {
    if (network.counts[key] !== value) errors.push(`count drift ${key}: expected ${value}`);
  }
  return [...new Set(errors)];
}

async function filesUnder(root, relative) {
  const files = [];
  for (const entry of await readdir(path.join(root, relative), { withFileTypes: true })) {
    const next = path.posix.join(relative, entry.name);
    if (entry.isDirectory()) files.push(...await filesUnder(root, next));
    else if (entry.isFile()) files.push(next);
  }
  return files.sort();
}

export function frontendPageEntries(files) {
  return files.filter(source => /\.(tsx?|jsx?)$/.test(source) && !path.basename(source).startsWith("_"))
    .map(source => ({
      source,
      route: (`/${source.slice("frontend/pages/".length).replace(/\.(tsx?|jsx?)$/, "")}`).replace(/\/index$/, "") || "/",
    }));
}

export async function checkFunctionalNetwork(root) {
  const network = JSON.parse(await readFile(path.join(root, "docs/functional-network.json"), "utf8"));
  const contract = await checkRepositoryContract(root);
  const application = await readFile(path.join(root, "engine/src/application.rs"), "utf8");
  const pages = frontendPageEntries(await filesUnder(root, "frontend/pages"));
  const templateSources = (await filesUnder(root, "engine/src/routes/ebpf"))
    .filter(source => /\/templates[^/]*\.inc\.rs$/.test(source));
  const templates = (await Promise.all(templateSources.map(async source =>
    [...(await readFile(path.join(root, source), "utf8")).matchAll(/id: "([^"]+)"/g)].map(match => match[1]),
  ))).flat();
  const labs = [...(await readFile(path.join(root, "engine/src/services/learning_catalog.rs"), "utf8"))
    .matchAll(/id: "([^"]+)"/g)].map(match => match[1]);
  const moduleSources = (await filesUnder(root, "modules"))
    .filter(source => /^modules\/[^/]+\/module\.json$/.test(source));
  const bundledModules = await Promise.all(moduleSources.map(async source =>
    JSON.parse(await readFile(path.join(root, source), "utf8")).name,
  ));
  const sourceHashes = Object.fromEntries(await Promise.all(network.source_inputs.map(async source => {
    const resolved = path.resolve(root, source.path);
    if (!resolved.startsWith(`${path.resolve(root)}${path.sep}`)) throw new Error(`source outside repository: ${source.path}`);
    try {
      return [source.path, createHash("sha256").update(await readFile(resolved)).digest("hex")];
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      return [source.path, null];
    }
  })));
  const errors = validateNetwork(network, {
    version: contract.version, operations: extractEngineOperations(application),
    access: extractEngineAccess(application), pages, templates, labs, bundledModules, sourceHashes,
  });
  if (errors.length) throw new Error(errors.join("\n"));
  return { version: contract.version, ...network.counts, sdk_operations: contract.sdkOperations, source_inputs: network.source_inputs.length };
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMain) {
  if (process.argv.length !== 2) throw new Error("Usage: node scripts/check-functional-network.mjs");
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
  try {
    console.log(JSON.stringify({ status: "source-snapshot-matches", ...await checkFunctionalNetwork(root) }, null, 2));
  } catch (error) {
    console.error(`[functional-network] ${error.message}`);
    process.exitCode = 1;
  }
}
