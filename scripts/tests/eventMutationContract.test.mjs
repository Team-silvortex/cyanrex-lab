import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("event mutations document unconfirmed writes, cancellation and private acknowledgements", async () => {
  const doc = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  for (const path of ["/events/mark-read", "/events/delete"]) {
    const op = doc.paths[path].post;
    assert.equal(op["x-cyanrex-access"], "authenticated");
    assert.match(op["x-cyanrex-csrf"], /Origin/);
    assert.match(op.description, /503/);
    assert.match(op.description, /10.second/);
    assert.match(op.description, /not rollback/);
    assert.match(op.description, /verify state/);
    for (const code of ["200", "503"]) {
      assert.equal(op.responses[code].headers["Cache-Control"].schema.const, "no-store");
    }
    assert.equal(op.responses["200"].content["application/json"].schema.properties.ok.type, "boolean");
  }
  assert.equal(doc.paths["/events/delete"].post.responses["400"].headers["Cache-Control"].schema.const, "no-store");
});
