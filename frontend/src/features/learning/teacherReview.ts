import type { LabAttempt, SaveTeacherFeedbackRequest } from "./models";

export const TEACHER_ATTEMPT_LIMIT = 20;
export const TEACHER_FEEDBACK_MAX_LENGTH = 2000;

export function buildTeacherAttemptsUrl(
  engineUrl: string,
  username: string,
  limit = TEACHER_ATTEMPT_LIMIT,
): string {
  const normalizedLimit = Number.isFinite(limit)
    ? Math.min(50, Math.max(1, Math.trunc(limit)))
    : TEACHER_ATTEMPT_LIMIT;
  const params = new URLSearchParams({
    username: username.trim(),
    limit: String(normalizedLimit),
  });
  return `${engineUrl.replace(/\/$/, "")}/learning/teacher/attempts?${params}`;
}

export function teacherFeedbackRequest(
  attempt: Pick<LabAttempt, "id" | "username" | "teacher_feedback">,
  comment: string,
): SaveTeacherFeedbackRequest {
  const normalized = comment.trim();
  if (!normalized || Array.from(normalized).length > TEACHER_FEEDBACK_MAX_LENGTH) {
    throw new Error("comment must contain 1 to 2000 characters");
  }
  return {
    username: attempt.username,
    attempt_id: attempt.id,
    comment: normalized,
    expected_revision: attempt.teacher_feedback?.revision ?? 0,
  };
}
