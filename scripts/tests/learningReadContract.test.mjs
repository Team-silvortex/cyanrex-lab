import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("learning projections distinguish failed storage reads from empty success and remain private", async () => {
  const document = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  const cases = [
    ["/learning/attempts", "authenticated", { type: "array", items: { $ref: "#/components/schemas/LabAttempt" } }],
    ["/learning/labs", "authenticated", { type: "array", items: { $ref: "#/components/schemas/LabProgress" } }],
    ["/learning/teacher/attempts", "staff", { $ref: "#/components/schemas/TeacherStudentAttempts" }],
    ["/learning/teacher/overview", "staff", { $ref: "#/components/schemas/TeacherLearningOverview" }],
  ];
  for (const [path, access, schema] of cases) {
    const operation = document.paths[path].get;
    assert.ok(operation.responses[500], `${path} needs an explicit read-failure response`);
    assert.match(operation.responses[500].description, /not empty/i);
    assert.match(operation.description, /selected PostgreSQL/i);
    assert.match(operation.description, /no silent local fallback/i);
    assert.equal(operation["x-cyanrex-access"], access);
    assert.deepEqual(operation.responses[200].content["application/json"].schema, schema);
    assert.equal(operation.responses[500].content["application/json"].schema.$ref, "#/components/schemas/ApiMessage");
    for (const status of [200, 500]) {
      assert.equal(operation.responses[status].headers["Cache-Control"].schema.const, "no-store");
    }
  }
});
