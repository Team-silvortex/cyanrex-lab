import assert from "node:assert/strict";
import test from "node:test";
import { normalizeAgentCalls } from "../dist/agents/calls.js";
import { jsonCopy, parseArguments, MAX_AGENT_ARGUMENT_BYTES } from "../dist/agents/json.js";
import { validateToolArguments } from "../dist/agents/schema.js";

test("model-supplied approval and unexpected call fields are rejected", () => {
  assert.throws(() => normalizeAgentCalls("custom", [{
    id: "call-1", name: "getHealth", arguments: {}, approved: true,
  }]), TypeError);
});

const custom = arguments_ => [{ id: "call-1", name: "getHealth", arguments: arguments_ }];

test("all six formats normalize explicit stable call IDs without changing arguments", () => {
  const expected = [{ id: "call-1", name: "getEvents", arguments: { query: { limit: 1 } } }];
  const args = expected[0].arguments;
  const formats = {
    openai_responses: [{ type: "function_call", call_id: "call-1", name: "getEvents", arguments: JSON.stringify(args) }],
    openai_chat_completions: [{ id: "call-1", type: "function", function: { name: "getEvents", arguments: JSON.stringify(args) } }],
    anthropic_messages: [{ type: "tool_use", id: "call-1", name: "getEvents", input: args }],
    gemini_generate_content: [{ functionCall: { id: "call-1", name: "getEvents", args } }],
    mcp: [{ jsonrpc: "2.0", id: "call-1", method: "tools/call", params: { name: "getEvents", arguments: args } }],
    custom: expected,
  };
  for (const [format, calls] of Object.entries(formats)) assert.deepEqual(normalizeAgentCalls(format, calls), expected);
});

test("malformed envelopes, missing IDs and duplicate calls are rejected before dispatch", () => {
  for (const calls of [null, {}, custom(null), custom([]), custom({}) .concat(custom({})),
    [{ id: "", name: "getHealth", arguments: {} }], [{ id: "x", name: "../getHealth", arguments: {} }],
    Array.from({ length: 33 }, (_, i) => ({ id: String(i), name: "getHealth", arguments: {} }))]) {
    assert.throws(() => normalizeAgentCalls("custom", calls), TypeError);
  }
  assert.throws(() => normalizeAgentCalls("gemini_generate_content", [{ functionCall: { name: "getHealth", args: {} } }]), TypeError);
  assert.throws(() => normalizeAgentCalls("openai_responses", [{ type: "function_call", call_id: "x", name: "getHealth", arguments: "{}", status: "in_progress" }]), TypeError);
  assert.throws(() => normalizeAgentCalls("mcp", [{ id: "x", method: "tools/call", params: { name: "getHealth", approved: true } }]), TypeError);
  assert.equal(normalizeAgentCalls("custom", Array.from({ length: 32 }, (_, i) => ({ id: String(i), name: "getHealth", arguments: {} }))).length, 32);
});

test("JSON arguments reject duplicate decoded keys, prototype keys and invalid literals", () => {
  for (const text of ['{"a":1,"a":2}', '{"a":1,"\\u0061":2}', '{"a":{"x":1,"x":1}}',
    '{"__proto__":{}}', '{"a":{"constructor":{}}}', '{"prototype":null}', '{"x":1e999}',
    'null', '[]', '{"x":undefined}', '{"x":1,}', '{} {}', '\ufeff{}']) {
    assert.throws(() => parseArguments(text), TypeError, text);
  }
  assert.deepEqual(parseArguments(' { "a": [true, false, null, 1.5, "\\\"中\\\\"] } '), { a: [true, false, null, 1.5, '"中\\'] });
});

test("argument limit is UTF-8 bytes and accepts the exact JSON boundary", () => {
  const json = JSON.stringify({ text: "a".repeat(MAX_AGENT_ARGUMENT_BYTES - 11) });
  assert.equal(Buffer.byteLength(json), MAX_AGENT_ARGUMENT_BYTES);
  assert.equal(parseArguments(json).text.length, MAX_AGENT_ARGUMENT_BYTES - 11);
  assert.throws(() => parseArguments(json.replace('"}', 'a"}')), TypeError);
  assert.throws(() => normalizeAgentCalls("custom", custom({ text: "中".repeat(22000) })), TypeError);
});

test("host objects cannot execute getters, toJSON or prototypes during normalization", () => {
  let touched = false;
  const getter = Object.defineProperty({}, "name", { enumerable: true, get() { touched = true; return "getHealth"; } });
  const cycle = {}; cycle.child = cycle;
  for (const args of [getter, { toJSON() { touched = true; return {}; } }, new Date(), Object.create({ hidden: true }), cycle, { x: undefined }, { x: 1n }, { x: NaN }]) {
    assert.throws(() => normalizeAgentCalls("custom", custom(args)), TypeError);
  }
  assert.equal(touched, false);
  assert.deepEqual(jsonCopy(Object.assign(Object.create(null), { safe: "text" })), { safe: "text" });
  const envelope = [];
  Object.defineProperty(envelope, "0", { enumerable: true, get() { touched = true; return custom({})[0]; } });
  assert.throws(() => normalizeAgentCalls("custom", envelope), TypeError);
  assert.equal(touched, false);
  assert.throws(() => normalizeAgentCalls("unknown", []), TypeError);
});

test("runtime schemas enforce type, enum, bounds, required and closed nested properties", () => {
  const schema = { type: "object", properties: { query: { type: "object", properties: {
    limit: { type: "integer", minimum: 1, maximum: 500 }, category: { type: "string", enum: ["platform", "kernel"] },
  }, required: ["limit"], additionalProperties: false } }, required: ["query"], additionalProperties: false };
  validateToolArguments(schema, { query: { limit: 1, category: "platform" } });
  for (const args of [{}, { query: {} }, { query: { limit: 0 } }, { query: { limit: 1.5 } },
    { query: { limit: 1, category: "bad" } }, { query: { limit: "1" } }, { query: { limit: 2, approved: true } },
    { query: { limit: 2 }, body: {} }]) assert.throws(() => validateToolArguments(schema, args), TypeError);
  assert.throws(() => validateToolArguments({ type: "string", unknownConstraint: true }, "ok"), TypeError);
});
