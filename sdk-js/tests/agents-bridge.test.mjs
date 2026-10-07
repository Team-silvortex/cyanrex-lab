import assert from "node:assert/strict";
import test from "node:test";
import { CyanrexClient } from "../dist/index.js";
import { createAgentBridge, formatAgentResults, MAX_AGENT_RESULT_BYTES } from "../dist/agents/index.js";
import { agentToolCatalog } from "../dist/generated/agent-tools.js";

const call = (id, name = "getHealth", arguments_ = {}) => ({ id, name, arguments: arguments_ });
function fake() {
  const calls = [];
  const client = { async operation(...args) { calls.push(args); return { ok: true }; } };
  return { calls, client };
}
test("explicit finite allowlist excludes privileged and non-JSON platform surfaces", () => {
  const { client } = fake();
  assert.throws(() => createAgentBridge(client), TypeError);
  assert.deepEqual(createAgentBridge(client, { operations: [] }).tools("custom"), []);
  for (const name of ["getAuthMe", "postAuthLogin", "getSettingsAiAgents", "postCommand", "postEbpfRun", "postEbpfDetach",
    "postRunnerAgentHeartbeat", "postRunnerJobsProbe", "getClassroomInvitations", "getEventsExport", "getWsEvents", "constructor"]) {
    assert.throws(() => createAgentBridge(client, { operations: [name], allowMutations: true, approveMutation: () => true }), TypeError, name);
  }
  assert.equal(Object.keys(agentToolCatalog).length, 28);
  assert.equal(Object.values(agentToolCatalog).filter(tool => tool.mutating).length, 5);
  assert.throws(() => createAgentBridge(client, { operations: ["getHealth", "getHealth"] }), TypeError);
});

test("provider definitions preserve explicit selection, schemas and non-strict OpenAI functions", () => {
  const bridge = createAgentBridge(fake().client, { operations: ["getEvents"] });
  const native = bridge.tools("custom")[0];
  assert.equal(native.name, "getEvents");
  assert.equal(native.inputSchema.additionalProperties, false);
  assert.deepEqual(bridge.tools("mcp"), [native]);
  assert.deepEqual(bridge.tools("openai_responses"), [{ type: "function", name: native.name, description: native.description, parameters: native.inputSchema, strict: false }]);
  assert.deepEqual(bridge.tools("openai_chat_completions"), [{ type: "function", function: { name: native.name, description: native.description, parameters: native.inputSchema, strict: false } }]);
  assert.deepEqual(bridge.tools("anthropic_messages"), [{ name: native.name, description: native.description, input_schema: native.inputSchema }]);
  assert.deepEqual(bridge.tools("gemini_generate_content"), [{ functionDeclarations: [{ name: native.name, description: native.description, parametersJsonSchema: native.inputSchema }] }]);
  native.inputSchema.additionalProperties = true;
  assert.equal(bridge.tools("custom")[0].inputSchema.additionalProperties, false);
});

test("whole batch validation occurs before any network call or approval", async () => {
  const f = fake(); let approved = 0;
  const bridge = createAgentBridge(f.client, { operations: ["getEvents", "postScriptsDelete"], allowMutations: true, approveMutation: () => { approved++; return true; } });
  for (const calls of [[call("a", "postScriptsDelete", { body: { id: "one" } }), call("b", "getEvents", { query: { limit: 0 } })],
    [call("a", "getEvents"), call("a", "getEvents")], [call("c", "getAuthMe")], [call("d", "getEvents", { approved: true })]]) {
    await assert.rejects(bridge.executeCalls("custom", calls), TypeError);
  }
  assert.equal(approved, 0); assert.deepEqual(f.calls, []);
});

test("default is read only and each mutation needs separate trusted immutable approval", async () => {
  const f = fake(); const approvals = [];
  assert.throws(() => createAgentBridge(f.client, { operations: ["postScriptsDelete"] }), TypeError);
  assert.throws(() => createAgentBridge(f.client, { operations: ["postScriptsDelete"], allowMutations: true }), TypeError);
  const bridge = createAgentBridge(f.client, { operations: ["postScriptsDelete"], allowMutations: true, approveMutation(request) {
    approvals.push(request);
    assert.throws(() => { request.call.arguments.body.id = "changed"; }, TypeError);
    return request.call.id === "allow";
  } });
  const results = await bridge.executeCalls("custom", [call("deny", "postScriptsDelete", { body: { id: "one" } }), call("allow", "postScriptsDelete", { body: { id: "two" } })]);
  assert.equal(approvals.length, 2); assert.equal(f.calls.length, 1);
  assert.deepEqual(f.calls[0][1], { body: { id: "two" } });
  assert.deepEqual(results[0].error, { code: "approval_denied", outcome: "not_attempted" });
  assert.equal(results[1].ok, true);
});

test("lost response is unconfirmed and the bridge never repeats a call ID", async () => {
  let dispatched = 0;
  const bridge = createAgentBridge({ async operation() { dispatched++; throw new Error("secret cookie and sensitive URL"); } }, { operations: ["getHealth"] });
  const results = await bridge.executeCalls("custom", [call("lost")]);
  assert.deepEqual(results[0].error, { code: "operation_failed", outcome: "unconfirmed" });
  assert.equal(JSON.stringify(results).includes("secret"), false);
  await assert.rejects(bridge.executeCalls("custom", [call("lost")]), TypeError);
  assert.equal(dispatched, 1);
});

test("pending batches cannot run concurrently and mutations run serially", async () => {
  let release; const barrier = new Promise(resolve => { release = resolve; });
  const events = [];
  const bridge = createAgentBridge({ async operation(_name, input) {
    events.push(input.body.id); if (input.body.id === "one") await barrier; return {};
  } }, { operations: ["postScriptsDelete"], allowMutations: true, approveMutation: () => true });
  const pending = bridge.executeCalls("custom", [call("one", "postScriptsDelete", { body: { id: "one" } }), call("two", "postScriptsDelete", { body: { id: "two" } })]);
  await Promise.resolve();
  assert.deepEqual(events, ["one"]);
  await assert.rejects(bridge.executeCalls("custom", []), /pending batch/);
  release(); await pending;
  assert.deepEqual(events, ["one", "two"]);
});

test("cancellation during approval prevents dispatch and callback errors are private", async () => {
  const f = fake(); const abort = new AbortController();
  const bridge = createAgentBridge(f.client, { operations: ["postScriptsDelete"], allowMutations: true,
    approveMutation: () => { abort.abort(); return true; } });
  const [result] = await bridge.executeCalls("custom", [call("a", "postScriptsDelete", { body: { id: "one" } })], { signal: abort.signal });
  assert.deepEqual(result.error, { code: "cancelled", outcome: "not_attempted" });
  assert.equal(f.calls.length, 0);
  const broken = createAgentBridge(f.client, { operations: ["postScriptsDelete"], allowMutations: true, approveMutation: () => { throw Error("private host secret"); } });
  const denied = await broken.executeCalls("custom", [call("b", "postScriptsDelete", { body: { id: "one" } })]);
  assert.equal(JSON.stringify(denied).includes("secret"), false); assert.equal(f.calls.length, 0);
});

test("dispatch uses current SDK cookies and CSRF without calling a model endpoint", async () => {
  const seen = []; const signal = new AbortController().signal;
  const client = new CyanrexClient("https://engine.example", { sessionCookie: "synthetic-session", csrfOrigin: "https://teacher.example",
    fetch: async (url, init) => { seen.push({ url, init }); return Response.json({ ok: true }); } });
  const bridge = createAgentBridge(client, { operations: ["postScriptsDelete"], allowMutations: true, approveMutation: () => true });
  await bridge.executeCalls("custom", [call("a", "postScriptsDelete", { body: { id: "script-1" } })], { signal });
  assert.equal(seen[0].url, "https://engine.example/scripts/delete");
  assert.equal(seen[0].init.headers.Cookie, "cyanrex_session=synthetic-session");
  assert.equal(seen[0].init.headers.Origin, "https://teacher.example");
  assert.equal(seen[0].init.signal, signal); assert.equal(seen.length, 1);
});

test("oversized or non-JSON results produce generic unconfirmed errors", async () => {
  for (const output of [{ huge: "a".repeat(MAX_AGENT_RESULT_BYTES) }, { bigint: 1n }]) {
    const bridge = createAgentBridge({ operation: async () => output }, { operations: ["getHealth"] });
    assert.deepEqual((await bridge.executeCalls("custom", [call("a")]))[0].error, { code: "operation_failed", outcome: "unconfirmed" });
  }
});

test("result formatting emits provider-native correlation and generic error envelopes", () => {
  const result = { id: "a", name: "getHealth", ok: false, error: { code: "operation_failed", outcome: "unconfirmed" } };
  for (const format of ["openai_responses", "openai_chat_completions", "anthropic_messages", "gemini_generate_content", "mcp", "custom"]) {
    const [formatted] = formatAgentResults(format, [result]);
    assert.ok(JSON.stringify(formatted).includes("operation_failed"));
  }
  assert.equal(formatAgentResults("openai_responses", [result])[0].call_id, "a");
  assert.equal(formatAgentResults("anthropic_messages", [result])[0].is_error, true);
  assert.equal(formatAgentResults("mcp", [result])[0].isError, true);
});

test("call ledger is bounded and denial or pre-dispatch cancellation cannot be replayed", async () => {
  const f = fake(); const bridge = createAgentBridge(f.client, { operations: ["getHealth"] });
  for (let batch = 0; batch < 32; batch++) {
    await bridge.executeCalls("custom", Array.from({ length: 32 }, (_, index) => call(`${batch}-${index}`)));
  }
  assert.equal(f.calls.length, 1024);
  await assert.rejects(bridge.executeCalls("custom", [call("beyond-budget")]), /budget exhausted/);
  assert.equal(f.calls.length, 1024);
  const controller = new AbortController(); controller.abort();
  const stopped = createAgentBridge(f.client, { operations: ["getHealth"] });
  const [result] = await stopped.executeCalls("custom", [call("cancelled")], { signal: controller.signal });
  assert.deepEqual(result.error, { code: "cancelled", outcome: "not_attempted" });
  await assert.rejects(stopped.executeCalls("custom", [call("cancelled")]), TypeError);
  assert.equal(f.calls.length, 1024);
});

test("pending approval uses a detached snapshot and aborted dispatch is never retried", async () => {
  let release;
  const barrier = new Promise(resolve => { release = resolve; });
  const f = fake();
  const bridge = createAgentBridge(f.client, { operations: ["postScriptsDelete"], allowMutations: true, approveMutation: () => barrier });
  const original = [call("one", "postScriptsDelete", { body: { id: "original" } })];
  const pending = bridge.executeCalls("custom", original);
  original[0].arguments.body.id = "changed-after-admission";
  release(true); await pending;
  assert.deepEqual(f.calls[0][1], { body: { id: "original" } });
  let count = 0; const controller = new AbortController();
  const aborting = createAgentBridge({ operation: async (_name, _input, { signal }) => {
    count++; controller.abort(); throw signal.reason;
  } }, { operations: ["getHealth"] });
  const results = await aborting.executeCalls("custom", [call("a"), call("b")], { signal: controller.signal });
  assert.deepEqual(results.map(item => item.error.outcome), ["unconfirmed", "not_attempted"]);
  assert.equal(count, 1);
});

test("an abort-ignoring late success cannot acknowledge a cancelled batch", async () => {
  const controller = new AbortController(); let dispatched = 0;
  const bridge = createAgentBridge({ async operation() {
    dispatched++; controller.abort(); return { sensitive: "late result" };
  } }, { operations: ["getHealth"] });
  const results = await bridge.executeCalls("custom", [call("late"), call("next")], { signal: controller.signal });
  assert.deepEqual(results[0].error, { code: "operation_failed", outcome: "unconfirmed" });
  assert.deepEqual(results[1].error, { code: "cancelled", outcome: "not_attempted" });
  assert.equal(JSON.stringify(results).includes("late result"), false);
  assert.equal(dispatched, 1);
  await assert.rejects(bridge.executeCalls("custom", [call("late")]), TypeError);
});
