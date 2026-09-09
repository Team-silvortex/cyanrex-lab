import assert from "node:assert/strict";
import test from "node:test";
import { attemptEditorUrl, loadResumeAttempt, parseResumeTarget } from "../src/features/learning/resumeAttempt.ts";

const id = "00000000-0000-4000-8000-000000000001";
const lab = "01-first-program";
const attempt = { id, lab_id: lab, template_id: "xdp-pass", source: "// 中文\n", feedback: [], teacher_feedback: null };

test("resume links carry only the lab and attempt identifiers, not submitted source or usernames", () => {
  assert.equal(attemptEditorUrl({ ...attempt, username: "private", source: "secret code" }), `/ebpf?lab=${lab}&attempt=${id}`);
});

test("resume targets distinguish absent, valid and malformed queries", () => {
  assert.equal(parseResumeTarget(undefined, lab), null);
  assert.deepEqual(parseResumeTarget(id, lab), { attemptId: id, labId: lab });
  for (const value of ["", [id, id], "../escape", "not-an-id"]) assert.throws(() => parseResumeTarget(value, lab));
  for (const value of [undefined, [lab], "../bad"]) assert.throws(() => parseResumeTarget(id, value));
});

test("resume reads one owner-bound attempt with credentials, no-store and cancellation", async () => {
  const controller = new AbortController();
  let call;
  const result = await loadResumeAttempt("http://localhost:8080/", { attemptId: id, labId: lab }, controller.signal,
    async (...args) => { call = args; return Response.json(attempt); });
  assert.deepEqual(result, attempt);
  assert.equal(call[0], `http://localhost:8080/learning/attempt?attempt_id=${id}`);
  assert.equal(call[1].credentials, "include");
  assert.equal(call[1].cache, "no-store");
  assert.equal(call[1].signal, controller.signal);
});

test("resume rejects missing, cross-lab, mismatched and oversized records without applying them", async () => {
  const target = { attemptId: id, labId: lab };
  for (const payload of [null, {}, { ...attempt, id: "another" }, { ...attempt, lab_id: "02-trace-execve" },
    { ...attempt, source: "😀".repeat(65537) }, { ...attempt, source: 1 }, { ...attempt, template_id: {} }]) {
    await assert.rejects(loadResumeAttempt("/engine", target, new AbortController().signal, async () => Response.json(payload)));
  }
  for (const status of [401, 404, 500]) {
    await assert.rejects(loadResumeAttempt("/engine", target, new AbortController().signal,
      async () => Response.json({ message: "private server detail" }, { status })), new RegExp(`HTTP ${status}`));
  }
});

test("an aborted resume discards even a late successful response", async () => {
  const controller = new AbortController();
  await assert.rejects(loadResumeAttempt("/engine", { attemptId: id, labId: lab }, controller.signal, async () => {
    controller.abort();
    return Response.json(attempt);
  }), error => error.name === "AbortError");
});
