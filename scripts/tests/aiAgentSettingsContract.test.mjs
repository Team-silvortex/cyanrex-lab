import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { aiAgentSchemas } from "../openapi-ai-agent-schemas.mjs";

test("AI Agent metadata is bounded, revision-fenced and cannot carry API keys", () => {
  assert.equal(aiAgentSchemas.AiAgentProfile.additionalProperties, false);
  assert.equal(aiAgentSchemas.UpdateAiAgentSettingsRequest.additionalProperties, false);
  assert.equal(aiAgentSchemas.AiAgentProfile.properties.api_key, undefined);
  assert.equal(aiAgentSchemas.AiAgentProfile.properties.credential_ref.anyOf[0].pattern, "^[A-Z][A-Z0-9_]{0,63}$");
  assert.equal(aiAgentSchemas.UpdateAiAgentSettingsRequest.properties.profiles.maxItems, 16);
  assert.equal(aiAgentSchemas.UpdateAiAgentSettingsRequest.properties.expected_revision.maximum, 4294967295);
  assert.equal(aiAgentSchemas.AiAgentProtocol.enum.length, 5);
});

test("AI Agent configuration requires teacher authority and stays outside model-executable tools", async () => {
  const doc = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  for (const method of ["get", "post"]) {
    const op = doc.paths["/settings/ai-agents"][method];
    assert.equal(op["x-cyanrex-access"], "admin");
    assert.deepEqual(op["x-cyanrex-roles"], ["admin", "teacher"]);
    assert.equal(op.responses[200].headers["Cache-Control"].schema.const, "no-store");
    assert.match(op.description, /not Runner configuration|not a model proxy|no provider requests/);
  }
  const op = doc.paths["/settings/ai-agents"].post;
  assert.equal(op.requestBody.content["application/json"].schema.$ref, "#/components/schemas/UpdateAiAgentSettingsRequest");
  assert.ok(op.responses[409]); assert.ok(op.responses[503]); assert.ok(op.responses[413]);
});
