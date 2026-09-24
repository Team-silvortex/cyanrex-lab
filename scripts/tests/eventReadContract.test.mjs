import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("event reads document private unavailable results without changing session authority or success shapes", async () => {
  const doc = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  for (const path of ["/events", "/events/export", "/events/unread-count"]) {
    const operation = doc.paths[path].get;
    assert.equal(operation["x-cyanrex-access"], "authenticated");
    assert.match(operation.description, /503/);
    assert.match(operation.description, /10.second/);
    for (const code of ["200", "503"]) {
      assert.equal(operation.responses[code].headers["Cache-Control"].schema.const, "no-store");
    }
    if (path !== "/events/unread-count") {
      assert.equal(operation.responses["400"].headers["Cache-Control"].schema.const, "no-store");
      assert.match(operation.description, /invalid|Invalid/);
    }
  }
  assert.equal(doc.paths["/events"].get.responses["200"].content["application/json"].schema.type, "array");
  assert.ok(doc.paths["/events/export"].get.responses["200"].content["text/csv"]);
  assert.equal(doc.paths["/events/unread-count"].get.responses["200"].content["application/json"].schema.properties.unread.minimum, 0);
});
