import type { LabAttempt } from "./models";

export type ResumeTarget = { attemptId: string; labId: string };
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function attemptEditorUrl(attempt: Pick<LabAttempt, "id" | "lab_id">): string {
  return `/ebpf?lab=${encodeURIComponent(attempt.lab_id)}&attempt=${encodeURIComponent(attempt.id)}`;
}

export function parseResumeTarget(attempt: unknown, lab: unknown): ResumeTarget | null {
  if (attempt === undefined) return null;
  if (typeof attempt !== "string" || !uuid.test(attempt) || typeof lab !== "string" || !/^\d{2}-[a-z0-9-]+$/.test(lab)) {
    throw new Error("Invalid resume link");
  }
  return { attemptId: attempt, labId: lab };
}

export async function loadResumeAttempt(
  engineUrl: string,
  target: ResumeTarget,
  signal: AbortSignal,
  fetcher: typeof fetch = fetch,
  sourceLimit = 256 * 1024,
): Promise<LabAttempt> {
  const url = `${engineUrl.replace(/\/+$/, "")}/learning/attempt?attempt_id=${encodeURIComponent(target.attemptId)}`;
  const response = await fetcher(url, { credentials: "include", cache: "no-store", signal });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const attempt = await response.json() as LabAttempt;
  signal.throwIfAborted();
  if (!attempt || attempt.id !== target.attemptId || attempt.lab_id !== target.labId ||
      typeof attempt.source !== "string" || new TextEncoder().encode(attempt.source).length > sourceLimit ||
      (attempt.template_id != null && typeof attempt.template_id !== "string")) {
    throw new Error("Invalid attempt response");
  }
  return attempt;
}
