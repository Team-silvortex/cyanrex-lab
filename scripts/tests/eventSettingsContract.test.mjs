import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("event settings expose private unavailable reads and unconfirmed transactional updates", async () => {
  const doc = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  for (const method of ["get", "post"]) {
    const op = doc.paths["/settings/events"][method];
    assert.equal(op["x-cyanrex-access"], "authenticated");
    assert.match(op.description, /503/);
    assert.match(op.description, /10.second/);
    for (const code of ["200", "503"]) assert.equal(op.responses[code].headers["Cache-Control"].schema.const, "no-store");
  }
  const op = doc.paths["/settings/events"].post;
  assert.match(op.description, /not rollback/);
  assert.match(op.description, /transaction/);
  assert.match(op["x-cyanrex-csrf"], /Origin/);
  for (const code of ["400", "413", "415", "422"]) assert.equal(op.responses[code].headers["Cache-Control"].schema.const, "no-store");
  assert.equal(op.responses["200"].content["application/json"].schema.$ref, "#/components/schemas/UpdateEventSettingsResponse");
});
