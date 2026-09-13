import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const document = JSON.parse(await readFile(new URL("../../engine/openapi/openapi.json", import.meta.url), "utf8"));

test("user remote checks document bounded queue waiting without changing success or access", () => {
  for (const [path, method, status] of [
    ["/ebpf/check/remote", "post", 202],
    ["/ebpf/check/remote", "get", 200],
    ["/ebpf/check/remote/cancel", "post", 200],
  ]) {
    const operation = document.paths[path][method];
    assert.equal(operation["x-cyanrex-access"], "authenticated");
    assert.match(operation.description, /35-second queue wait/);
    assert.match(operation.description, /next queue interaction/);
    assert.match(operation.description, /claimed execution deadline is unchanged/);
    assert.equal(operation.responses[status].content["application/json"].schema.$ref, "#/components/schemas/EbpfRemoteCheckResponse");
    if (method === "post") assert.ok(operation["x-cyanrex-csrf"]);
  }
  assert.doesNotMatch(document.paths["/runner/jobs/compile-check"].post.description, /35-second queue wait/);
});

test("Agent claim and sync retain signed authority and explain capacity and lost leases", () => {
  const claim = document.paths["/runner/agent/jobs/claim"].post;
  const sync = document.paths["/runner/agent/jobs/sync"].post;
  assert.match(claim.description, /active_jobs \+ available_slots/);
  assert.match(claim.description, /cancel-requested leases/);
  assert.match(sync.description, /lost_job_ids/);
  assert.match(sync.description, /before execution/);
  for (const operation of [claim, sync]) assert.equal(operation["x-cyanrex-access"], "runner-agent-signed");
  assert.equal(claim.responses[200].content["application/json"].schema.$ref, "#/components/schemas/RunnerJobClaimResponse");
  assert.equal(sync.responses[200].content["application/json"].schema.$ref, "#/components/schemas/RunnerJobSyncResponse");
});
