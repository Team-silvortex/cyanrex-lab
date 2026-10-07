import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { collectAgentTools, renderAgentTools } from "../generate-agent-tools.mjs";

const document = () => ({
  components: { schemas: { Script: {
    type: "object", properties: { name: { type: "string" }, code: { type: "string" } },
    required: ["name", "code"], additionalProperties: false,
  } } },
  paths: {
    "/health": { get: operation("getHealth", "public") },
    "/scripts/save": { post: { ...operation("postScriptsSave", "authenticated"),
      requestBody: { required: true, content: { "application/json": { schema: { $ref: "#/components/schemas/Script" } } } },
    } },
    "/auth/login": { post: operation("postAuthLogin", "public") },
    "/ebpf/run": { post: operation("postEbpfRun", "authenticated") },
    "/settings/ai-agents": { post: operation("postSettingsAiAgents", "admin") },
    "/future": { get: operation("getFutureSecret", "authenticated") },
  },
});
function operation(operationId, access) {
  return { operationId, "x-cyanrex-access": access,
    responses: { 200: { content: { "application/json": { schema: { type: "object" } } } } } };
}

test("agent tools require a reviewed allowlist rather than exposing all browser operations", () => {
  const tools = collectAgentTools(document());
  assert.deepEqual(Object.keys(tools), ["getHealth", "postScriptsSave"]);
  assert.equal(tools.getHealth.mutating, false);
  assert.equal(tools.postScriptsSave.mutating, true);
  assert.equal(tools.postScriptsSave.operationName, "postScriptsSave");
});

test("agent tool schemas expand local references while preserving required and closed inputs", () => {
  const tools = collectAgentTools(document());
  const schema = tools.postScriptsSave.inputSchema;
  assert.equal(schema.additionalProperties, false);
  assert.deepEqual(schema.required, ["body"]);
  assert.deepEqual(schema.properties.body.required, ["name", "code"]);
  assert.equal(schema.properties.body.additionalProperties, false);
  assert.doesNotMatch(JSON.stringify(tools), /\$ref/);
  assert.equal(renderAgentTools(document()), renderAgentTools(document()));
});

test("tool generation rejects policy path/access drift, recursive and external schema references", () => {
  const moved = document(); moved.paths["/health"].get["x-cyanrex-access"] = "admin";
  assert.throws(() => collectAgentTools(moved), /policy drift/);
  const external = document();
  external.components.schemas.Script.properties.code = { $ref: "https://example.invalid/schema" };
  assert.throws(() => collectAgentTools(external), /local schema reference/);
  const recursive = document();
  recursive.components.schemas.Script.properties.code = { $ref: "#/components/schemas/Script" };
  assert.throws(() => collectAgentTools(recursive), /recursive schema/);
});

test("committed tool catalogue excludes auth, secrets, kernel writes and configuration changes", async () => {
  const source = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  const tools = collectAgentTools(source);
  assert.equal(Object.keys(tools).length, 28);
  for (const name of ["postAuthLogin", "postAuthDelete", "postClassroomJoin", "postCommand",
    "postEbpfRun", "postEbpfDetach", "postSettingsAiAgents", "getSettingsAiAgents", "postModulesStart"]) {
    assert.equal(tools[name], undefined, name);
  }
});
