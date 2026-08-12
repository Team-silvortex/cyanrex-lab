import assert from "node:assert/strict";
import test from "node:test";

import {
  buildTeacherAttemptsUrl,
  TEACHER_ATTEMPT_LIMIT,
} from "../src/features/learning/teacherReview.ts";

test("teacher review URL encodes the student and uses the bounded default", () => {
  assert.equal(TEACHER_ATTEMPT_LIMIT, 20);
  assert.equal(
    buildTeacherAttemptsUrl("http://localhost:8080/", " student&limit=50 "),
    "http://localhost:8080/learning/teacher/attempts?username=student%26limit%3D50&limit=20",
  );
});

test("teacher review URL clamps requested history size", () => {
  assert.match(buildTeacherAttemptsUrl("/engine", "student", 999), /limit=50$/);
  assert.match(buildTeacherAttemptsUrl("/engine", "student", 0), /limit=1$/);
  assert.match(buildTeacherAttemptsUrl("/engine", "student", Number.NaN), /limit=20$/);
});
