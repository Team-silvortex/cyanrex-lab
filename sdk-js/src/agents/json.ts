export const MAX_AGENT_ARGUMENT_BYTES = 64 * 1024;
export const MAX_AGENT_CALLS = 32;
export const MAX_AGENT_RESULT_BYTES = 1024 * 1024;
const dangerous = new Set(["__proto__", "prototype", "constructor"]);
const encoder = new TextEncoder();

export function invalid(): never { throw new TypeError("Invalid agent tool input"); }
export function bytes(value: string): number { return encoder.encode(value).length; }
export function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)
    || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) invalid();
  return value as Record<string, unknown>;
}

// Copies only JSON data. No getters, custom prototypes, toJSON, cycles or unsafe keys.
export function jsonCopy(value: unknown, maxBytes = MAX_AGENT_ARGUMENT_BYTES): unknown {
  const active = new Set<object>();
  let budget = maxBytes;
  const spend = (amount: number) => { budget -= amount; if (budget < 0) invalid(); };
  function visit(item: unknown, depth: number): unknown {
    if (depth > 32) invalid();
    if (item === null || typeof item === "boolean") { spend(JSON.stringify(item).length); return item; }
    if (typeof item === "string") { spend(bytes(JSON.stringify(item))); return item; }
    if (typeof item === "number" && Number.isFinite(item)) { spend(JSON.stringify(item).length); return item; }
    if (!item || typeof item !== "object" || active.has(item)) invalid();
    active.add(item); spend(2);
    let output: unknown;
    if (Array.isArray(item)) {
      if (Object.getPrototypeOf(item) !== Array.prototype || item.length > maxBytes) invalid();
      const entries = Object.getOwnPropertyDescriptors(item);
      if (Object.keys(entries).length !== item.length + 1 || Object.getOwnPropertySymbols(item).length) invalid();
      output = Array.from({ length: item.length }, (_, index) => {
        const descriptor = entries[String(index)];
        if (!descriptor || !("value" in descriptor)) invalid();
        if (index > 0) spend(1); return visit(descriptor.value, depth + 1);
      });
    } else {
      record(item);
      if (Object.getOwnPropertySymbols(item).length) invalid();
      const copy: Record<string, unknown> = {};
      let count = 0;
      for (const [key, descriptor] of Object.entries(Object.getOwnPropertyDescriptors(item))) {
        if (dangerous.has(key) || !descriptor.enumerable || !("value" in descriptor)) invalid();
        spend(bytes(JSON.stringify(key)) + 1 + (count++ > 0 ? 1 : 0)); copy[key] = visit(descriptor.value, depth + 1);
      }
      output = copy;
    }
    active.delete(item); return output;
  }
  const copy = visit(value, 0);
  if (bytes(JSON.stringify(copy)) > maxBytes) invalid();
  return copy;
}

// JSON.parse alone silently accepts duplicate object keys; scan decoded names first.
export function parseArguments(raw: unknown): Record<string, unknown> {
  if (typeof raw !== "string" || bytes(raw) > MAX_AGENT_ARGUMENT_BYTES) invalid();
  const text = raw;
  let index = 0;
  const ws = () => { while (index < text.length && /\s/.test(text[index])) index++; };
  function string(): string {
    const start = index++;
    while (index < text.length) {
      const char = text[index++];
      if (char === "\\") index++;
      else if (char === '"') return JSON.parse(text.slice(start, index));
    }
    return invalid();
  }
  function value(depth: number): void {
    if (depth > 32) invalid();
    ws();
    if (text[index] === '"') { string(); return; }
    const open = text[index];
    if (open === "{" || open === "[") {
      index++; ws();
      const close = open === "{" ? "}" : "]";
      const keys = new Set<string>();
      if (text[index] === close) { index++; return; }
      while (index < text.length) {
        if (open === "{") {
          ws(); if (text[index] !== '"') invalid();
          const key = string();
          if (keys.has(key)) invalid(); keys.add(key);
          ws(); if (text[index++] !== ":") invalid();
        }
        value(depth + 1); ws();
        if (text[index] === close) { index++; return; }
        if (text[index++] !== ",") invalid();
      }
      invalid();
    }
    const start = index;
    while (index < text.length && !/[\s,}\]]/.test(text[index])) index++;
    if (index === start) invalid();
  }
  try {
    value(0); ws(); if (index !== text.length) invalid();
    return record(jsonCopy(JSON.parse(text)));
  } catch { return invalid(); }
}

export function exact(value: unknown, required: readonly string[], optional: readonly string[] = []): Record<string, unknown> {
  const item = record(value);
  if (required.some(key => !Object.hasOwn(item, key))
    || Object.keys(item).some(key => !required.includes(key) && !optional.includes(key))) invalid();
  return item;
}

export function freezeJson<T>(value: T): T {
  if (value && typeof value === "object") {
    for (const child of Object.values(value)) freezeJson(child);
    Object.freeze(value);
  }
  return value;
}
