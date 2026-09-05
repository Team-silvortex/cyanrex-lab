import assert from "node:assert/strict";
import test from "node:test";
import { CyanrexApiError, CyanrexClient } from "../dist/index.js";

test("teacher feedback convenience and generated APIs preserve revisions and CSRF", async () => {
  const calls = [];
  const feedback = { reviewer: "teacher", comment: "Check bounds", revision: 1, updated_at: "2026-09-05T00:00:00Z" };
  const client = new CyanrexClient("https://lab.example/api", {
    csrfOrigin: "https://lab.example", sessionCookie: "cyanrex_session=test",
    fetch: async (url, init) => { calls.push({ url, init }); return Response.json(feedback); },
  });
  const body = { username: "student", attempt_id: "attempt-1", comment: "Check bounds", expected_revision: 0 };
  const controller = new AbortController();
  assert.deepEqual(await client.learning.saveTeacherFeedback(body, { signal: controller.signal }), feedback);
  assert.deepEqual(await client.operation("postLearningTeacherFeedback", { body }), feedback);
  for (const call of calls) {
    assert.equal(call.url, "https://lab.example/api/learning/teacher/feedback");
    assert.equal(call.init.method, "POST");
    assert.equal(call.init.credentials, "include");
    assert.equal(call.init.headers.Origin, "https://lab.example");
    assert.equal(call.init.headers.Cookie, "cyanrex_session=test");
    assert.deepEqual(JSON.parse(call.init.body), body);
  }
  assert.equal(calls[0].init.signal, controller.signal);
});

test("teacher feedback conflicts are exposed without retrying an overwrite", async () => {
  let calls = 0;
  const client = new CyanrexClient("/engine", {
    fetch: async () => { calls += 1; return Response.json({ ok: false, message: "feedback changed; reload before editing" }, { status: 409 }); },
  });
  await assert.rejects(() => client.learning.saveTeacherFeedback({ username: "student", attempt_id: "attempt-1", comment: "Review", expected_revision: 0 }),
    (error) => error instanceof CyanrexApiError && error.status === 409);
  assert.equal(calls, 1);
});
