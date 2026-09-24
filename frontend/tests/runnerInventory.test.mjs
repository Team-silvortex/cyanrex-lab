import assert from "node:assert/strict";
import test from "node:test";
import { runnerSafetyMessages } from "../src/i18n/locales/runnerSafety.ts";
import { parseAgentInventory, parseJobInventory, parseProbeAcknowledgement, parseCancelAcknowledgement } from "../src/features/runner/runnerInventory.ts";
import { agentInventory, jobInventory, runnerAgent, runnerJob, stamp } from "./fixtures/runnerData.mjs";

test("inventories accept Engine DTOs, disabled registries and retained terminal jobs", () => {
  assert.deepEqual(parseAgentInventory(agentInventory()), agentInventory());
  assert.deepEqual(parseAgentInventory(agentInventory([], { enabled: false })), agentInventory([], { enabled: false }));
  const expected = runnerJob({ state: "cancelled", completed_at: stamp }); delete expected.output;
  assert.deepEqual(parseJobInventory(jobInventory([runnerJob({ state: "cancelled", completed_at: stamp })])), jobInventory([expected]));
});
test("inventories reject missing containers, wrong counts and duplicate identities", () => {
  for (const bad of [null, [], {}, { agents: null }, agentInventory([], { total_agents: 1 }), agentInventory([], { online_agents: 1 }),
    agentInventory([runnerAgent(), runnerAgent()]), agentInventory([], { enabled: "true" })]) assert.throws(() => parseAgentInventory(bad));
  for (const bad of [null, [], {}, { jobs: null }, jobInventory([], { total_jobs: -1 }), jobInventory([], { total_jobs: "0" }),
    jobInventory([runnerJob(), runnerJob()])]) assert.throws(() => parseJobInventory(bad));
});
test("all displayed nested fields are validated before rendering", () => {
  for (const changes of [{ labels: null }, { labels: [] }, { labels: { room: {} } }, { capabilities: null }, { capabilities: [true] },
    { state: "unknown" }, { isolation: "unknown" }, { agent_version: null }, { agent_id: "" }, { message: {} }, { kernel_release: [] }])
    assert.throws(() => parseAgentInventory(agentInventory([runnerAgent(changes)])));
  for (const changes of [{ state: "unknown" }, { job_id: "" }, { kind: null }, { owner_username: {} }, { message: [] },
    { source_bytes: -1 }, { target_agent_id: [] }, { program_name: {} }, { result_message: 42 }])
    assert.throws(() => parseJobInventory(jobInventory([runnerJob(changes)])));
});
test("counts and capacities reject coercion, impossible sums and unsafe arithmetic", () => {
  for (const key of ["protocol_version", "max_concurrent", "active_jobs", "available_slots"])
    for (const value of [null, "1", NaN, Infinity, -1, 0.5, Number.MAX_SAFE_INTEGER + 1])
      assert.throws(() => parseAgentInventory(agentInventory([runnerAgent({ [key]: value })])));
  assert.throws(() => parseAgentInventory(agentInventory([runnerAgent({ active_jobs: 2, available_slots: 2 })])));
  for (const value of [0, -1, "30", null, Infinity, 0.5]) assert.throws(() => parseJobInventory(jobInventory([runnerJob({ timeout_seconds: value })])));
});
test("timestamps require explicit time zones and real parseable instants", () => {
  for (const value of [null, "", "tomorrow", "2026-09-13", "2026-09-13T00:00:00", "2026-99-13T00:00:00Z", "2026-02-30T00:00:00Z"]) {
    assert.throws(() => parseAgentInventory(agentInventory([], { generated_at: value })));
    assert.throws(() => parseJobInventory(jobInventory([runnerJob({ created_at: value })])));
  }
  assert.throws(() => parseJobInventory(jobInventory([runnerJob({ deadline: {} })])));
  assert.equal(parseAgentInventory(agentInventory([], { generated_at: "2026-09-13T08:00:00.123456+08:00" })).agents.length, 0);
});

test("inventory bounds prevent unbounded retained lists and nested labels", () => {
  assert.throws(() => parseAgentInventory(agentInventory(Array.from({ length: 257 }, (_, n) => runnerAgent({ agent_id: `agent-${n}` })))));
  assert.throws(() => parseJobInventory(jobInventory(Array.from({ length: 513 }, (_, n) => runnerJob({ job_id: `job-${n}` })))));
  assert.throws(() => parseAgentInventory(agentInventory([runnerAgent({ labels: Object.fromEntries(Array.from({ length: 17 }, (_, n) => [`key-${n}`, "value"])) })])));
});
test("all locales include readable, matching Runner safety messages", () => {
  const keys = Object.keys(runnerSafetyMessages.en).sort();
  for (const messages of Object.values(runnerSafetyMessages)) {
    assert.deepEqual(Object.keys(messages).sort(), keys);
    for (const message of Object.values(messages)) assert.ok(typeof message === "string" && message.length > 20);
  }
});
test("validated display DTOs are copied and ignore output or future extension fields", () => {
  const source = agentInventory(); const parsed = parseAgentInventory(source);
  source.agents[0].labels.room = "changed"; source.agents[0].capabilities.push("future");
  assert.deepEqual(parsed, agentInventory());
  const jobs = parseJobInventory(jobInventory([runnerJob({ output: "not displayed", extra: { ignored: true } })]));
  assert.equal("output" in jobs.jobs[0], false); assert.equal("extra" in jobs.jobs[0], false);
});
const probe = (changes = {}) => runnerJob({ kind: "control_probe", message: "teacher health probe", timeout_seconds: 30,
  owner_username: null, program_name: null, source_bytes: null, ...changes });
test("probe acknowledgement binds the submitted agent, kind, state and request", () => {
  assert.equal(parseProbeAcknowledgement(probe(), "fixture-agent").job_id, "fixture-job");
  for (const bad of [{ ok: true }, { ok: false }, null, probe({ target_agent_id: "other" }), probe({ state: "succeeded" }),
    probe({ kind: "ebpf_compile_check" }), probe({ message: "different" }), probe({ timeout_seconds: 20 }), probe({ owner_username: "someone" })])
    assert.throws(() => parseProbeAcknowledgement(bad, "fixture-agent"));
});
test("cancel acknowledgements accept both Engine transitions but bind the reviewed job", () => {
  for (const state of ["cancelled", "cancel_requested"])
    assert.equal(parseCancelAcknowledgement(runnerJob({ state }), runnerJob()).state, state);
  for (const bad of [{ ok: true }, runnerJob(), runnerJob({ state: "succeeded" }), runnerJob({ job_id: "other", state: "cancelled" }),
    runnerJob({ state: "cancelled", owner_username: "other" }), runnerJob({ state: "cancelled", target_agent_id: "other" })])
    assert.throws(() => parseCancelAcknowledgement(bad, runnerJob()));
});
