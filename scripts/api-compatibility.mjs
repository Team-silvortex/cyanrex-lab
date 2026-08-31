import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HTTP_METHODS = new Set(["get", "post", "put", "patch", "delete", "head", "options"]);
const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const currentPath = path.join(projectRoot, "engine/openapi/openapi.json");
const baselinePath = path.join(projectRoot, "sdk-js/compatibility/openapi-baseline.json");

export function compareApiCompatibility(baseline, candidate) {
  const errors = [];
  const baselineOperations = operations(baseline);
  const candidateOperations = operations(candidate);

  for (const [name, previous] of baselineOperations) {
    const next = candidateOperations.get(name);
    if (!next) {
      errors.push(`operation removed: ${name}`);
      continue;
    }
    const previousAccess = previous["x-cyanrex-access"] ?? "missing";
    const nextAccess = next["x-cyanrex-access"] ?? "missing";
    if (previousAccess !== nextAccess) {
      errors.push(`access changed for ${name}: ${previousAccess} -> ${nextAccess}`);
    }
    compareParameters(baseline, candidate, previous, next, name, errors);
    compareRequestBody(baseline, candidate, previous, next, name, errors);
    compareResponses(baseline, candidate, previous, next, name, errors);
  }
  return errors;
}

export function createCompatibilitySnapshot(document) {
  const paths = {};
  for (const [routePath, pathItem] of Object.entries(document.paths ?? {})) {
    const operationsForPath = {};
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (!HTTP_METHODS.has(method)) continue;
      operationsForPath[method] = pick(operation, [
        "operationId", "parameters", "requestBody", "responses", "x-cyanrex-access",
      ]);
    }
    if (Object.keys(operationsForPath).length > 0) paths[routePath] = operationsForPath;
  }
  return {
    openapi: document.openapi,
    info: pick(document.info ?? {}, ["title", "version"]),
    paths,
    components: { schemas: clone(document.components?.schemas ?? {}) },
  };
}

export async function checkRepositoryCompatibility(root = projectRoot) {
  const [baseline, candidate] = await Promise.all([
    readJson(path.join(root, "sdk-js/compatibility/openapi-baseline.json")),
    readJson(path.join(root, "engine/openapi/openapi.json")),
  ]);
  const errors = compareApiCompatibility(baseline, candidate);
  if (errors.length > 0) throw new Error(errors.join("\n"));
  return {
    baselineVersion: baseline.info?.version ?? "unknown",
    candidateVersion: candidate.info?.version ?? "unknown",
    operations: operations(baseline).size,
  };
}

function compareParameters(oldDocument, newDocument, oldOperation, newOperation, name, errors) {
  const oldParameters = parameterMap(oldOperation.parameters);
  const newParameters = parameterMap(newOperation.parameters);
  for (const [key, oldParameter] of oldParameters) {
    const next = newParameters.get(key);
    const label = `${name} parameter ${oldParameter.in}.${oldParameter.name}`;
    if (!next) {
      errors.push(`${label} was removed`);
      continue;
    }
    if (!oldParameter.required && next.required) errors.push(`${label} is now required`);
    compareSchema(oldDocument, newDocument, oldParameter.schema ?? {}, next.schema ?? {}, "input", label, errors);
  }
  for (const [key, parameter] of newParameters) {
    if (!oldParameters.has(key) && parameter.required) {
      errors.push(`${name} added required parameter ${parameter.in}.${parameter.name}`);
    }
  }
}

function compareRequestBody(oldDocument, newDocument, oldOperation, newOperation, name, errors) {
  const oldBody = oldOperation.requestBody;
  const newBody = newOperation.requestBody;
  if (!oldBody) {
    if (newBody?.required) errors.push(`${name} added a required request body`);
    return;
  }
  if (!newBody) {
    errors.push(`${name} no longer accepts its request body`);
    return;
  }
  if (!oldBody.required && newBody.required) errors.push(`${name} request body is now required`);
  for (const [mediaType, oldMedia] of Object.entries(oldBody.content ?? {})) {
    const next = newBody.content?.[mediaType];
    const label = `${name} request`;
    if (!next) {
      errors.push(`${label} no longer accepts ${mediaType}`);
      continue;
    }
    compareSchema(oldDocument, newDocument, oldMedia.schema ?? {}, next.schema ?? {}, "input", label, errors);
  }
}

function compareResponses(oldDocument, newDocument, oldOperation, newOperation, name, errors) {
  for (const [status, oldResponse] of Object.entries(oldOperation.responses ?? {})) {
    if (!isSuccessStatus(status)) continue;
    const next = newOperation.responses?.[status];
    const label = `${name} response[${status}]`;
    if (!next) {
      errors.push(`${name} removed successful response ${status}`);
      continue;
    }
    for (const [mediaType, oldMedia] of Object.entries(oldResponse.content ?? {})) {
      const nextMedia = next.content?.[mediaType];
      if (!nextMedia) {
        errors.push(`${label} no longer returns ${mediaType}`);
        continue;
      }
      compareSchema(oldDocument, newDocument, oldMedia.schema ?? {}, nextMedia.schema ?? {}, "output", label, errors);
    }
  }
}

function compareSchema(oldDocument, newDocument, oldRaw, newRaw, direction, label, errors, depth = 0) {
  if (depth > 64) throw new Error(`schema nesting exceeds compatibility limit at ${label}`);
  const oldNullable = unwrapNullable(oldDocument, oldRaw);
  const newNullable = unwrapNullable(newDocument, newRaw);
  if (direction === "input" && oldNullable.nullable && !newNullable.nullable) {
    errors.push(`${label} no longer accepts null`);
  }
  if (direction === "output" && !oldNullable.nullable && newNullable.nullable) {
    errors.push(`${label} may now return null`);
  }
  const oldSchema = resolveSchema(oldDocument, oldNullable.schema);
  const newSchema = resolveSchema(newDocument, newNullable.schema);
  compareEnums(oldSchema.enum, newSchema.enum, direction, label, errors);

  const oldKind = schemaKind(oldSchema);
  const newKind = schemaKind(newSchema);
  if (!kindCompatible(oldKind, newKind, direction)) {
    const verb = direction === "input" ? "no longer accepts" : "may now return";
    errors.push(`${label} ${verb} ${newKind} instead of ${oldKind}`);
    return;
  }
  if (oldKind === "object" && newKind === "object") {
    compareObjects(oldDocument, newDocument, oldSchema, newSchema, direction, label, errors, depth);
  } else if (oldKind === "array" && newKind === "array") {
    compareSchema(
      oldDocument,
      newDocument,
      oldSchema.items ?? {},
      newSchema.items ?? {},
      direction,
      `${label}[]`,
      errors,
      depth + 1,
    );
  }
  compareConstraints(oldSchema, newSchema, direction, label, errors);
}

function compareObjects(oldDocument, newDocument, oldSchema, newSchema, direction, label, errors, depth) {
  const oldProperties = oldSchema.properties ?? {};
  const newProperties = newSchema.properties ?? {};
  const oldRequired = new Set(oldSchema.required ?? []);
  const newRequired = new Set(newSchema.required ?? []);

  for (const [property, oldProperty] of Object.entries(oldProperties)) {
    const propertyLabel = `${label}.${property}`;
    const next = newProperties[property];
    if (!next) {
      if (direction === "output") errors.push(`${label} removed property ${property}`);
      else if (!allowsAdditionalProperty(newSchema)) errors.push(`${label} no longer accepts property ${property}`);
      continue;
    }
    if (direction === "output" && oldRequired.has(property) && !newRequired.has(property)) {
      errors.push(`${label} no longer guarantees property ${property}`);
    }
    compareSchema(oldDocument, newDocument, oldProperty, next, direction, propertyLabel, errors, depth + 1);
  }

  if (direction === "input") {
    for (const property of newRequired) {
      if (!oldRequired.has(property)) errors.push(`${label} added required property ${property}`);
    }
    if (allowsAdditionalProperty(oldSchema) && !allowsAdditionalProperty(newSchema)) {
      errors.push(`${label} no longer accepts additional properties`);
    }
  }
}

function compareEnums(oldEnum, newEnum, direction, label, errors) {
  if (!oldEnum && !newEnum) return;
  if (direction === "input") {
    if (!oldEnum && newEnum) {
      errors.push(`${label} now restricts values to an enum`);
      return;
    }
    if (!newEnum) return;
    for (const value of oldEnum ?? []) {
      if (!newEnum.some((candidate) => Object.is(candidate, value))) {
        errors.push(`${label} no longer accepts enum value ${JSON.stringify(value)}`);
      }
    }
  } else {
    if (oldEnum && !newEnum) {
      errors.push(`${label} may now return values outside its enum`);
      return;
    }
    if (!oldEnum) return;
    for (const value of newEnum ?? []) {
      if (!oldEnum.some((candidate) => Object.is(candidate, value))) {
        errors.push(`${label} may now return enum value ${JSON.stringify(value)}`);
      }
    }
  }
}

function compareConstraints(oldSchema, newSchema, direction, label, errors) {
  const ranges = [
    ["minimum", -Infinity, "minimum"],
    ["minLength", 0, "minimum length"],
    ["minItems", 0, "minimum item count"],
    ["maximum", Infinity, "maximum"],
    ["maxLength", Infinity, "maximum length"],
    ["maxItems", Infinity, "maximum item count"],
  ];
  for (const [keyword, fallback, description] of ranges) {
    const oldValue = oldSchema[keyword] ?? fallback;
    const newValue = newSchema[keyword] ?? fallback;
    const lowerBound = keyword.startsWith("min");
    const breaks = direction === "input"
      ? (lowerBound ? newValue > oldValue : newValue < oldValue)
      : (lowerBound ? newValue < oldValue : newValue > oldValue);
    if (breaks) errors.push(`${label} changed ${description} from ${oldValue} to ${newValue}`);
  }
  if (direction === "input" && oldSchema.pattern !== newSchema.pattern && newSchema.pattern) {
    errors.push(`${label} added or changed its accepted pattern`);
  }
  if (direction === "output" && oldSchema.pattern && oldSchema.pattern !== newSchema.pattern) {
    errors.push(`${label} weakened its returned pattern guarantee`);
  }
}

function unwrapNullable(document, raw) {
  const schema = resolveSchema(document, raw);
  const alternatives = schema.anyOf ?? schema.oneOf;
  if (!Array.isArray(alternatives)) return { nullable: schema.type === "null", schema };
  const nonNull = alternatives.filter((item) => resolveSchema(document, item).type !== "null");
  const nullable = nonNull.length !== alternatives.length;
  return { nullable, schema: nonNull.length === 1 ? nonNull[0] : schema };
}

function resolveSchema(document, schema) {
  let current = schema ?? {};
  const seen = new Set();
  while (current?.$ref) {
    if (seen.has(current.$ref)) throw new Error(`cyclic schema reference ${current.$ref}`);
    seen.add(current.$ref);
    const prefix = "#/components/schemas/";
    if (!current.$ref.startsWith(prefix)) throw new Error(`unsupported schema reference ${current.$ref}`);
    const name = current.$ref.slice(prefix.length).replaceAll("~1", "/").replaceAll("~0", "~");
    current = document.components?.schemas?.[name];
    if (!current) throw new Error(`unresolved schema reference ${name}`);
  }
  return current;
}

function kindCompatible(oldKind, newKind, direction) {
  if (oldKind === newKind || oldKind === "any") return true;
  if (direction === "input") return newKind === "any" || (oldKind === "integer" && newKind === "number");
  return oldKind === "number" && newKind === "integer";
}

function schemaKind(schema) {
  if (schema.type) return schema.type;
  if (schema.properties || schema.additionalProperties !== undefined) return "object";
  return "any";
}

function allowsAdditionalProperty(schema) {
  return schema.additionalProperties === true
    || (schema.additionalProperties && typeof schema.additionalProperties === "object");
}

function operations(document) {
  const result = new Map();
  for (const [routePath, pathItem] of Object.entries(document.paths ?? {})) {
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (HTTP_METHODS.has(method)) result.set(`${method.toUpperCase()} ${routePath}`, operation);
    }
  }
  return new Map([...result].sort(([left], [right]) => left.localeCompare(right)));
}

function parameterMap(parameters = []) {
  return new Map(parameters.map((parameter) => [`${parameter.in}:${parameter.name}`, parameter]));
}

function isSuccessStatus(status) {
  return status === "101" || /^2\d\d$/.test(status);
}

function pick(value, keys) {
  return Object.fromEntries(keys.filter((key) => value?.[key] !== undefined).map((key) => [key, clone(value[key])]));
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

async function main() {
  const current = await readJson(currentPath);
  if (process.argv.includes("--write-baseline")) {
    await mkdir(path.dirname(baselinePath), { recursive: true });
    await writeFile(baselinePath, `${JSON.stringify(createCompatibilitySnapshot(current), null, 2)}\n`);
    console.log(`Wrote API compatibility baseline for ${current.info.version}.`);
    return;
  }
  const result = await checkRepositoryCompatibility();
  console.log(
    `API compatibility passed against ${result.baselineVersion}: ${result.operations} operations remain compatible with ${result.candidateVersion}.`,
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
