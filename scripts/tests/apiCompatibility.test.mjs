import assert from "node:assert/strict";
import test from "node:test";

import { compareApiCompatibility } from "../api-compatibility.mjs";

test("compatibility rejects removed operations and changed access tiers", () => {
  const baseline = contract();
  const removed = structuredClone(baseline);
  delete removed.paths["/items"].post;
  assert.deepEqual(compareApiCompatibility(baseline, removed), [
    "operation removed: POST /items",
  ]);

  const restricted = structuredClone(baseline);
  restricted.paths["/items"].post["x-cyanrex-access"] = "admin";
  assert.deepEqual(compareApiCompatibility(baseline, restricted), [
    "access changed for POST /items: authenticated -> admin",
  ]);
});

test("compatibility rejects request requirements and enum narrowing", () => {
  const baseline = contract();
  const candidate = structuredClone(baseline);
  const request = requestSchema(candidate);
  request.properties.note = { type: "string" };
  request.required.push("note");
  request.properties.mode.enum = ["fast"];

  assert.deepEqual(compareApiCompatibility(baseline, candidate), [
    "POST /items request.mode no longer accepts enum value \"safe\"",
    "POST /items request added required property note",
  ]);
});

test("compatibility rejects weakened response guarantees", () => {
  const baseline = contract();
  const missing = structuredClone(baseline);
  delete responseSchema(missing).properties.status;
  responseSchema(missing).required = [];
  assert.deepEqual(compareApiCompatibility(baseline, missing), [
    "POST /items response[200] removed property status",
  ]);

  const widened = structuredClone(baseline);
  responseSchema(widened).properties.status.enum.push("pending");
  assert.deepEqual(compareApiCompatibility(baseline, widened), [
    "POST /items response[200].status may now return enum value \"pending\"",
  ]);
});

test("compatibility permits additive operations and compatible object growth", () => {
  const baseline = contract();
  const candidate = structuredClone(baseline);
  requestSchema(candidate).properties.note = { type: "string" };
  responseSchema(candidate).properties.request_id = { type: "string" };
  responseSchema(candidate).required.push("request_id");
  candidate.paths["/health"] = {
    get: operation(undefined, { type: "object", properties: { status: { type: "string" } } }),
  };

  assert.deepEqual(compareApiCompatibility(baseline, candidate), []);
});

test("compatibility treats nullable input and output in opposite directions", () => {
  const baseline = contract();
  requestSchema(baseline).properties.note = nullable({ type: "string" });

  const inputNarrowed = structuredClone(baseline);
  requestSchema(inputNarrowed).properties.note = { type: "string" };
  assert.deepEqual(compareApiCompatibility(baseline, inputNarrowed), [
    "POST /items request.note no longer accepts null",
  ]);

  const outputWidened = structuredClone(baseline);
  responseSchema(outputWidened).properties.status = nullable(
    responseSchema(outputWidened).properties.status,
  );
  assert.deepEqual(compareApiCompatibility(baseline, outputWidened), [
    "POST /items response[200].status may now return null",
  ]);
});

function contract() {
  return {
    openapi: "3.1.0",
    info: { version: "0.3.0" },
    paths: {
      "/items": {
        post: operation(
          {
            type: "object",
            properties: {
              mode: { type: "string", enum: ["fast", "safe"] },
            },
            required: ["mode"],
            additionalProperties: false,
          },
          {
            type: "object",
            properties: {
              status: { type: "string", enum: ["ok", "error"] },
            },
            required: ["status"],
            additionalProperties: false,
          },
        ),
      },
    },
    components: { schemas: {} },
  };
}

function operation(request, response) {
  return {
    operationId: "postItems",
    "x-cyanrex-access": "authenticated",
    ...(request ? {
      requestBody: {
        required: true,
        content: { "application/json": { schema: request } },
      },
    } : {}),
    responses: {
      200: {
        description: "ok",
        content: { "application/json": { schema: response } },
      },
    },
  };
}

function requestSchema(document) {
  return document.paths["/items"].post.requestBody.content["application/json"].schema;
}

function responseSchema(document) {
  return document.paths["/items"].post.responses[200].content["application/json"].schema;
}

function nullable(schema) {
  return { anyOf: [schema, { type: "null" }] };
}
