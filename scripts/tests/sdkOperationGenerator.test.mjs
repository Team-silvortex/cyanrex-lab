import assert from "node:assert/strict";
import test from "node:test";

import {
  collectSdkOperations,
  renderSdkOperations,
} from "../generate-sdk-operations.mjs";

test("operation generation covers SDK transports and excludes signed Agent protocol", () => {
  const operations = collectSdkOperations(sampleDocument());

  assert.deepEqual(
    operations.map(({ operationId, transport }) => [operationId, transport]),
    [
      ["getExport", "download"],
      ["getThing", "json"],
      ["getUpdates", "websocket"],
      ["postThing", "json"],
    ],
  );
  assert.equal(operations.some(({ operationId }) => operationId === "postAgentHeartbeat"), false);
});

test("rendered operations preserve required path, query, body, and response types", () => {
  const output = renderSdkOperations(sampleDocument());

  assert.match(output, /"getThing": \{[\s\S]*?"path": \{[\s\S]*?"id": string;/);
  assert.match(output, /"query"\?: \{[\s\S]*?"verbose"\?: boolean;/);
  assert.match(output, /"postThing": \{[\s\S]*?"body": OpenApiSchemas\["ThingInput"\];/);
  assert.match(output, /response: OpenApiSchemas\["Thing"\];/);
  assert.match(output, /"getExport": \{"method":"GET"[\s\S]*?"transport":"download"\}/);
  assert.match(output, /"getUpdates": \{"method":"GET"[\s\S]*?"transport":"websocket"\}/);
});

test("operation generation rejects unsupported parameters and mismatched path templates", () => {
  const invalidHeader = sampleDocument();
  invalidHeader.paths["/things/{id}"].get.parameters.push({
    name: "x-token",
    in: "header",
    required: true,
    schema: { type: "string" },
  });
  assert.throws(() => collectSdkOperations(invalidHeader), /unsupported header parameter x-token/);

  const invalidPath = sampleDocument();
  invalidPath.paths["/things/{id}"].parameters = [];
  assert.throws(
    () => collectSdkOperations(invalidPath),
    /path template and parameters do not match/,
  );
});

function sampleDocument() {
  return {
    paths: {
      "/things/{id}": {
        parameters: [
          { name: "id", in: "path", required: true, schema: { type: "string" } },
        ],
        get: {
          operationId: "getThing",
          "x-cyanrex-access": "public",
          parameters: [
            { name: "verbose", in: "query", required: false, schema: { type: "boolean" } },
          ],
          responses: {
            200: {
              content: {
                "application/json": { schema: { $ref: "#/components/schemas/Thing" } },
              },
            },
          },
        },
      },
      "/things": {
        post: {
          operationId: "postThing",
          "x-cyanrex-access": "admin",
          requestBody: {
            required: true,
            content: {
              "application/json": { schema: { $ref: "#/components/schemas/ThingInput" } },
            },
          },
          responses: {
            200: {
              content: {
                "application/json": { schema: { $ref: "#/components/schemas/Thing" } },
              },
            },
          },
        },
      },
      "/export": {
        get: {
          operationId: "getExport",
          "x-cyanrex-access": "authenticated",
          responses: { 200: { content: { "text/csv": { schema: { type: "string" } } } } },
        },
      },
      "/updates": {
        get: {
          operationId: "getUpdates",
          "x-cyanrex-access": "authenticated",
          responses: { 101: { description: "upgrade" } },
        },
      },
      "/agent/heartbeat": {
        post: {
          operationId: "postAgentHeartbeat",
          "x-cyanrex-access": "runner-agent-signed",
          responses: { 200: { content: {} } },
        },
      },
    },
  };
}
