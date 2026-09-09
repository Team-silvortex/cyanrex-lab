import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("the legacy admin API tier truthfully includes teacher deployment authority", async () => {
  const document = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  let checked = 0;
  for (const path of Object.values(document.paths)) {
    for (const operation of Object.values(path)) {
      if (operation["x-cyanrex-access"] !== "admin") continue;
      assert.deepEqual(operation["x-cyanrex-roles"], ["admin", "teacher"]);
      assert.deepEqual(operation.security, [{ cookieAuth: [] }]);
      checked++;
    }
  }
  assert.equal(checked, 18);
});

test("supported launchers pass explicit teacher and legacy administrator allowlists", async () => {
  for (const name of ["start.sh", "docker/docker-compose.yml", "docker/docker-compose.distribution.yml"]) {
    const source = await readFile(new URL(`../../${name}`, import.meta.url), "utf8");
    for (const variable of ["CYANREX_TEACHER_USERNAMES", "CYANREX_ADMIN_USERNAMES"]) {
      assert.ok(source.includes(`${variable}:-`), `${name} must forward ${variable}`);
    }
  }
});
