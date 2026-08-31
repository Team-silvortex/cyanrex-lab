export const TEACHER_ATTEMPT_LIMIT = 20;

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
