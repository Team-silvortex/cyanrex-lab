import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HTTP_METHODS = new Set(["get", "post", "put", "patch", "delete", "head", "options"]);
const SDK_EXEMPT_OPERATIONS = new Set([
  "POST /runner/agent/heartbeat",
  "POST /runner/agent/jobs/claim",
  "POST /runner/agent/jobs/result",
  "POST /runner/agent/jobs/sync",
  "POST /runner/agent/register",
]);
const ROUTER_ACCESS = new Map([
  ["public_routes", "public"],
  ["csrf_protected_public_routes", "optional-session-csrf"],
  ["authenticated_routes", "authenticated"],
  ["staff_routes", "staff"],
  ["admin_routes", "admin"],
  ["runner_agent_routes", "runner-agent-signed"],
]);

export function extractEngineOperations(source) {
  const operations = new Set();
  let cursor = 0;

  while ((cursor = source.indexOf(".route", cursor)) !== -1) {
    const open = source.indexOf("(", cursor + ".route".length);
    if (open === -1) break;
    const close = findMatchingDelimiter(source, open, "(", ")");
    if (close === -1) throw new Error(`unbalanced Axum route call near byte ${cursor}`);
    const call = source.slice(open + 1, close);
    const pathMatch = call.match(/^\s*"([^"]+)"\s*,/);
    if (pathMatch) {
      const routePath = pathMatch[1];
      for (const match of call.matchAll(/\b(get|post|put|patch|delete|head|options)\s*\(/g)) {
        operations.add(`${match[1].toUpperCase()} ${routePath}`);
      }
    }
    cursor = close + 1;
  }

  return [...operations].sort();
}

export function extractEngineAccess(source) {
  const accessByOperation = {};
  const functionPattern = /\bfn\s+([a-zA-Z0-9_]+)\s*\([^)]*\)\s*->[^\{]+\{/g;

  for (const match of source.matchAll(functionPattern)) {
    const defaultAccess = ROUTER_ACCESS.get(match[1]);
    if (!defaultAccess) continue;
    const open = source.indexOf("{", match.index);
    const close = findMatchingDelimiter(source, open, "{", "}");
    if (close === -1) throw new Error(`unbalanced router function ${match[1]}`);
    for (const operation of extractEngineOperations(source.slice(open + 1, close))) {
      const access = operation === "POST /runner/agent/register"
        ? "runner-agent-bootstrap"
        : defaultAccess;
      if (accessByOperation[operation] && accessByOperation[operation] !== access) {
        throw new Error(`Engine operation ${operation} appears in multiple access tiers`);
      }
      accessByOperation[operation] = access;
    }
  }

  return Object.fromEntries(Object.entries(accessByOperation).sort(([left], [right]) => left.localeCompare(right)));
}

export function extractSdkOperations(source) {
  const operations = new Set();
  const callPattern = /this\.(get|post|download|websocketUrl)\b/g;

  for (const match of source.matchAll(callPattern)) {
    const open = source.indexOf("(", match.index + match[0].length);
    if (open === -1) continue;
    const pathMatch = source.slice(open + 1).match(/^\s*"([^"]+)"/);
    if (!pathMatch) continue;
    const method = match[1] === "post" ? "POST" : "GET";
    operations.add(`${method} ${pathMatch[1]}`);
  }

  return [...operations].sort();
}

export function openApiOperations(document) {
  const operations = new Set();
  for (const [routePath, pathItem] of Object.entries(document.paths ?? {})) {
    if (!pathItem || typeof pathItem !== "object") continue;
    for (const method of Object.keys(pathItem)) {
      if (HTTP_METHODS.has(method.toLowerCase())) {
        operations.add(`${method.toUpperCase()} ${routePath}`);
      }
    }
  }
  return [...operations].sort();
}

export function compareOperationSets(expected, actual) {
  const expectedSet = new Set(expected);
  const actualSet = new Set(actual);
  return {
    missing: [...expectedSet].filter((item) => !actualSet.has(item)).sort(),
    extra: [...actualSet].filter((item) => !expectedSet.has(item)).sort(),
  };
}

export function validateOpenApiDocument(document, input) {
  const errors = [];
  if (document.openapi !== "3.1.0") errors.push("OpenAPI version must be 3.1.0");
  if (document.info?.version !== input.version) {
    errors.push(`OpenAPI info.version is ${document.info?.version ?? "missing"}; expected ${input.version}`);
  }

  const specOperations = openApiOperations(document);
  appendDriftErrors(errors, "Engine/OpenAPI", compareOperationSets(input.engineOperations, specOperations));

  if (input.engineAccess) {
    for (const operation of input.engineOperations) {
      const [method, routePath] = operation.split(" ", 2);
      const actual = document.paths?.[routePath]?.[method.toLowerCase()]?.["x-cyanrex-access"];
      const expected = input.engineAccess[operation];
      if (!expected) errors.push(`Engine access tier missing for ${operation}`);
      else if (actual !== expected) {
        errors.push(`Engine/OpenAPI access mismatch for ${operation}: ${actual ?? "missing"}; expected ${expected}`);
      }
    }
  }

  const sdkExpected = input.engineOperations.filter(
    (operation) => !SDK_EXEMPT_OPERATIONS.has(operation),
  );
  appendDriftErrors(errors, "Engine/SDK", compareOperationSets(sdkExpected, input.sdkOperations));

  const operationIds = new Set();
  for (const [routePath, pathItem] of Object.entries(document.paths ?? {})) {
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (!HTTP_METHODS.has(method)) continue;
      if (!operation?.operationId) errors.push(`${method.toUpperCase()} ${routePath} lacks operationId`);
      else if (operationIds.has(operation.operationId)) errors.push(`duplicate operationId ${operation.operationId}`);
      else operationIds.add(operation.operationId);
      if (!operation?.responses || Object.keys(operation.responses).length === 0) {
        errors.push(`${method.toUpperCase()} ${routePath} lacks responses`);
      }
      if (!Array.isArray(operation?.tags) || operation.tags.length === 0) {
        errors.push(`${method.toUpperCase()} ${routePath} lacks tags`);
      }
    }
  }

  const schemaNames = new Set(Object.keys(document.components?.schemas ?? {}));
  for (const reference of collectReferences(document)) {
    const prefix = "#/components/schemas/";
    if (reference.startsWith(prefix) && !schemaNames.has(reference.slice(prefix.length))) {
      errors.push(`unresolved schema reference ${reference}`);
    }
  }
  return [...new Set(errors)];
}

export async function checkRepositoryContract(projectRoot = defaultProjectRoot()) {
  const [application, sdk, rawDocument, cargo] = await Promise.all([
    readFile(path.join(projectRoot, "engine/src/application.rs"), "utf8"),
    readFile(path.join(projectRoot, "sdk-js/src/index.ts"), "utf8"),
    readFile(path.join(projectRoot, "engine/openapi/openapi.json"), "utf8"),
    readFile(path.join(projectRoot, "engine/Cargo.toml"), "utf8"),
  ]);
  const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) throw new Error("could not read Engine version");
  const document = JSON.parse(rawDocument);
  const errors = validateOpenApiDocument(document, {
    version,
    engineOperations: extractEngineOperations(application),
    engineAccess: extractEngineAccess(application),
    sdkOperations: extractSdkOperations(sdk),
  });
  if (errors.length > 0) throw new Error(errors.join("\n"));
  return {
    version,
    operations: openApiOperations(document).length,
    sdkOperations: extractSdkOperations(sdk).length,
  };
}

function appendDriftErrors(errors, label, drift) {
  for (const item of drift.missing) errors.push(`${label} missing ${item}`);
  for (const item of drift.extra) errors.push(`${label} extra ${item}`);
}

function collectReferences(value, references = []) {
  if (Array.isArray(value)) {
    for (const item of value) collectReferences(item, references);
  } else if (value && typeof value === "object") {
    for (const [key, item] of Object.entries(value)) {
      if (key === "$ref" && typeof item === "string") references.push(item);
      else collectReferences(item, references);
    }
  }
  return references;
}

function findMatchingDelimiter(source, openIndex, opening, closing) {
  let depth = 0;
  let quote = null;
  let escaped = false;
  for (let index = openIndex; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }
    if (character === opening) depth += 1;
    else if (character === closing && --depth === 0) return index;
  }
  return -1;
}

function defaultProjectRoot() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
}

async function main() {
  const result = await checkRepositoryContract();
  console.log(
    `OpenAPI contract is synchronized at ${result.version}: ${result.operations} Engine operations, ${result.sdkOperations} SDK operations.`,
  );
}

const invokedAsScript = process.argv[1]
  && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;
if (invokedAsScript) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
