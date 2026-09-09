import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("classroom endpoints preserve separate discovery, invitation and join trust boundaries", async () => {
  const spec = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));
  assert.equal(spec.paths["/.well-known/cyanrex-classroom"].get["x-cyanrex-access"], "public");
  assert.equal(spec.paths["/classroom/join"].post["x-cyanrex-access"], "optional-session-csrf");
  assert.ok(spec.paths["/classroom/join"].post.responses["426"]);
  for (const [path, method] of [["/classroom/invitations", "get"], ["/classroom/invitations", "post"], ["/classroom/invitations/revoke", "post"]]) {
    assert.deepEqual(spec.paths[path][method]["x-cyanrex-roles"], ["admin", "teacher"]);
  }
  assert.equal(spec.components.schemas.ClassroomJoinRequest.additionalProperties, false);
  assert.equal(spec.components.schemas.ClassroomJoinRequest.properties.invite_token.writeOnly, true);
});

test("launchers forward opt-in classroom config and explicitly build the frontend API address", async () => {
  for (const file of ["start.sh", "docker/docker-compose.yml", "docker/docker-compose.distribution.yml"]) {
    const source = await readFile(new URL(`../../${file}`, import.meta.url), "utf8");
    for (const variable of ["CYANREX_CLASSROOM_PUBLIC_URL", "CYANREX_CLASSROOM_ID", "CYANREX_CLASSROOM_NAME", "CYANREX_CORS_ORIGINS", "CYANREX_SECURE_COOKIES"]) {
      assert.ok(source.includes(`${variable}:-`), `${file}: ${variable}`);
    }
  }
  const dockerfile = await readFile(new URL("../../frontend/Dockerfile", import.meta.url), "utf8");
  assert.match(dockerfile, /ARG NEXT_PUBLIC_ENGINE_URL=http:\/\/localhost:8080/);
  assert.match(dockerfile, /ENV NEXT_PUBLIC_ENGINE_URL=\$\{NEXT_PUBLIC_ENGINE_URL\}/);
  const packager = await readFile(new URL("../package-distribution.sh", import.meta.url), "utf8");
  assert.match(packager, /--build-arg NEXT_PUBLIC_ENGINE_URL=/);
});
