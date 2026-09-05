import assert from "node:assert/strict";
import test from "node:test";

import {
  buildTeacherAttemptsUrl,
  TEACHER_ATTEMPT_LIMIT,
  teacherFeedbackRequest,
} from "../src/features/learning/teacherReview.ts";

test("teacher review URL encodes the student and uses the bounded default", () => {
  assert.equal(TEACHER_ATTEMPT_LIMIT, 20);
  assert.equal(
    buildTeacherAttemptsUrl("http://localhost:8080/", " student&limit=50 "),
    "http://localhost:8080/learning/teacher/attempts?username=student%26limit%3D50&limit=20",
  );
});

test("teacher feedback request binds the attempt and the displayed revision", () => {
  const attempt = { id: "attempt-1", username: "student" };
  assert.deepEqual(teacherFeedbackRequest(attempt, "  请检查边界。  "), {
    attempt_id: "attempt-1", username: "student", comment: "请检查边界。", expected_revision: 0,
  });
  assert.equal(teacherFeedbackRequest({ ...attempt, teacher_feedback: { revision: 3 } }, "Update").expected_revision, 3);
});

test("teacher feedback rejects empty and oversized comments but accepts Unicode", () => {
  const attempt = { id: "attempt-1", username: "student" };
  assert.throws(() => teacherFeedbackRequest(attempt, " \n\t "), /comment/);
  assert.throws(() => teacherFeedbackRequest(attempt, "😀".repeat(2001)), /comment/);
  assert.equal(teacherFeedbackRequest(attempt, "😀".repeat(2000)).comment, "😀".repeat(2000));
});

test("teacher review URL clamps requested history size", () => {
  assert.match(buildTeacherAttemptsUrl("/engine", "student", 999), /limit=50$/);
  assert.match(buildTeacherAttemptsUrl("/engine", "student", 0), /limit=1$/);
  assert.match(buildTeacherAttemptsUrl("/engine", "student", Number.NaN), /limit=20$/);
});
