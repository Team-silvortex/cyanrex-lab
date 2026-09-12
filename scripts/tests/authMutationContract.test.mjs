import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("auth mutations document unconfirmed storage writes without weakening access or success schemas", async () => {
  const document = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  for (const path of ["/auth/logout", "/auth/password/change", "/auth/delete"]) {
    const operation = document.paths[path].post;
    assert.ok(operation.responses[503], `${path} needs an explicit storage-failure response`);
    assert.match(operation.responses[503].description, /unconfirmed/i);
    assert.match(operation.description, /memory-only/i);
    assert.equal(operation.responses[200].content["application/json"].schema.$ref, "#/components/schemas/ApiMessage");
    assert.equal(operation["x-cyanrex-access"], path === "/auth/logout" ? "optional-session-csrf" : "authenticated");
    assert.ok(operation["x-cyanrex-csrf"]);
  }
});

test("login and destructive event filters expose failure behavior without changing success contracts", async () => {
  const document = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  const login = document.paths["/auth/login"].post;
  assert.match(login.responses[503].description, /No new session cookie/i);
  assert.match(login.description, /same transaction/i);
  assert.equal(login["x-cyanrex-access"], "public");
  assert.equal(login.responses[200].content["application/json"].schema.$ref, "#/components/schemas/LoginResponse");
  const deletion = document.paths["/events/delete"].post;
  assert.equal(deletion["x-cyanrex-access"], "authenticated");
  assert.ok(deletion["x-cyanrex-csrf"]);
  assert.match(deletion.responses[400].description, /no events are deleted/i);
  assert.ok(deletion.responses[400].content["text/plain"]);
  assert.match(deletion.description, /unknown query keys/i);
  assert.match(deletion.description, /No filters explicitly means delete all/i);
  assert.equal(deletion.parameters.find(({ name }) => name === "since_minutes").schema.minimum, 0);
});
