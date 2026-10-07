import { invalid, record } from "./json.js";

// Deliberately bounded subset of expanded generated OpenAPI input schemas.
const supported = new Set(["type", "enum", "const", "properties", "required", "additionalProperties",
  "items", "minItems", "maxItems", "uniqueItems", "minLength", "maxLength", "pattern",
  "minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "multipleOf", "oneOf", "anyOf",
  "allOf", "nullable", "description", "title", "default", "format", "examples", "example",
  "minProperties", "maxProperties", "readOnly", "writeOnly", "deprecated"]);

export function validateToolArguments(schema: unknown, value: unknown, depth = 0): void {
  if (depth > 32) invalid();
  const rule = record(schema);
  if (Object.keys(rule).some(key => !supported.has(key))) invalid();
  if (value === null && rule.nullable === true) return;
  const check = (child: unknown, item = value) => validateToolArguments(child, item, depth + 1);
  for (const mode of ["oneOf", "anyOf", "allOf"] as const) {
    if (rule[mode] === undefined) continue;
    if (!Array.isArray(rule[mode]) || !rule[mode].length) invalid();
    let matches = 0;
    for (const child of rule[mode]) { try { check(child); matches++; } catch { /* union branch */ } }
    if (mode === "oneOf" ? matches !== 1 : mode === "anyOf" ? matches === 0 : matches !== rule[mode].length) invalid();
  }
  if (rule.const !== undefined && JSON.stringify(rule.const) !== JSON.stringify(value)) invalid();
  if (rule.enum !== undefined && (!Array.isArray(rule.enum)
    || !rule.enum.some(item => JSON.stringify(item) === JSON.stringify(value)))) invalid();
  const kinds = Array.isArray(rule.type) ? rule.type : rule.type === undefined ? [] : [rule.type];
  if (kinds.length && !kinds.some(kind => kind === "null" ? value === null
    : kind === "object" ? !!value && typeof value === "object" && !Array.isArray(value)
      : kind === "array" ? Array.isArray(value)
        : kind === "integer" ? Number.isSafeInteger(value)
          : kind === "number" ? typeof value === "number" && Number.isFinite(value)
            : kind === "string" || kind === "boolean" ? typeof value === kind : false)) invalid();
  if (typeof value === "string") {
    const length = [...value].length;
    if (typeof rule.minLength === "number" && length < rule.minLength
      || typeof rule.maxLength === "number" && length > rule.maxLength
      || typeof rule.pattern === "string" && !new RegExp(rule.pattern, "u").test(value)) invalid();
  }
  if (typeof value === "number") {
    if (typeof rule.minimum === "number" && value < rule.minimum
      || typeof rule.maximum === "number" && value > rule.maximum
      || typeof rule.exclusiveMinimum === "number" && value <= rule.exclusiveMinimum
      || typeof rule.exclusiveMaximum === "number" && value >= rule.exclusiveMaximum
      || rule.exclusiveMinimum === true && value === rule.minimum
      || rule.exclusiveMaximum === true && value === rule.maximum
      || typeof rule.multipleOf === "number" && value / rule.multipleOf % 1 !== 0) invalid();
  }
  if (Array.isArray(value)) {
    if (typeof rule.minItems === "number" && value.length < rule.minItems
      || typeof rule.maxItems === "number" && value.length > rule.maxItems
      || rule.uniqueItems === true && new Set(value.map(item => JSON.stringify(item))).size !== value.length) invalid();
    if (rule.items !== undefined) for (const item of value) check(rule.items, item);
  } else if (value && typeof value === "object") {
    const item = record(value);
    const properties = rule.properties === undefined ? {} : record(rule.properties);
    if (rule.required !== undefined && (!Array.isArray(rule.required)
      || rule.required.some(key => typeof key !== "string" || !Object.hasOwn(item, key)))) invalid();
    if (typeof rule.minProperties === "number" && Object.keys(item).length < rule.minProperties
      || typeof rule.maxProperties === "number" && Object.keys(item).length > rule.maxProperties) invalid();
    for (const [key, child] of Object.entries(item)) {
      if (Object.hasOwn(properties, key)) check(properties[key], child);
      else if (rule.additionalProperties && typeof rule.additionalProperties === "object") check(rule.additionalProperties, child);
      else invalid(); // No implicit open objects, including additionalProperties: true.
    }
  }
}
