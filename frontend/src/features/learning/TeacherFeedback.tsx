import { useId, useState } from "react";

import { useI18n } from "../../i18n/context";
import type { LabAttempt, LabTeacherFeedback, TeacherStudentAttempts } from "./models";
import { buildTeacherAttemptsUrl, teacherFeedbackRequest, TEACHER_FEEDBACK_MAX_LENGTH } from "./teacherReview";

export function TeacherFeedbackDisplay({ feedback }: { feedback?: LabTeacherFeedback | null }) {
  const { t } = useI18n();
  if (!feedback) return null;
  return (
    <div style={{ marginTop: 12, borderLeft: "3px solid var(--accent)", paddingLeft: 12 }}>
      <strong>{t("teaching.teacherFeedback")}</strong>
      <p className="meta" style={{ overflowWrap: "anywhere" }}>{feedback.reviewer} · {new Date(feedback.updated_at).toLocaleString()}</p>
      <p style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{feedback.comment}</p>
    </div>
  );
}

export function TeacherFeedbackEditor({ attempt, engineUrl }: { attempt: LabAttempt; engineUrl: string }) {
  const { t } = useI18n();
  const inputId = useId();
  const [feedback, setFeedback] = useState(attempt.teacher_feedback);
  const [comment, setComment] = useState(feedback?.comment ?? "");
  const [busy, setBusy] = useState(false);
  const [conflict, setConflict] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState(false);
  const length = Array.from(comment.trim()).length;

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    if (busy || conflict) return;
    setBusy(true);
    setError("");
    setSaved(false);
    try {
      const body = teacherFeedbackRequest({ ...attempt, teacher_feedback: feedback }, comment);
      const response = await fetch(`${engineUrl}/learning/teacher/feedback`, {
        method: "POST", credentials: "include",
        headers: { "Content-Type": "application/json" }, body: JSON.stringify(body),
      });
      if (response.status === 409) { setConflict(true); return; }
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const updated = await response.json() as LabTeacherFeedback;
      setFeedback(updated);
      setComment(updated.comment);
      setSaved(true);
    } catch (cause) {
      setError(`${t("teaching.feedbackSaveFailed")}: ${(cause as Error).message}`);
    } finally { setBusy(false); }
  };

  const reloadFeedback = async () => {
    setBusy(true);
    setError("");
    try {
      const response = await fetch(buildTeacherAttemptsUrl(engineUrl, attempt.username, 50), { credentials: "include" });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const payload = await response.json() as TeacherStudentAttempts;
      const current = payload.attempts.find((item) => item.id === attempt.id);
      if (!current) throw new Error(t("teaching.feedbackAttemptUnavailable"));
      setFeedback(current.teacher_feedback);
      setConflict(false);
      // Keep the draft. The teacher can compare it with the latest saved feedback before resubmitting.
    } catch (cause) {
      setError(`${t("teaching.reviewFailed")}: ${(cause as Error).message}`);
    } finally { setBusy(false); }
  };

  return (
    <form onSubmit={(event) => void save(event)} style={{ marginTop: 16 }}>
      <TeacherFeedbackDisplay feedback={feedback} />
      <label htmlFor={inputId}>{t("teaching.feedbackEdit")}</label>
      <p className="meta" id={`${inputId}-hint`}>{t("teaching.feedbackHint")}</p>
      <textarea id={inputId} rows={3} value={comment} disabled={busy}
        aria-describedby={`${inputId}-hint`} aria-invalid={length > TEACHER_FEEDBACK_MAX_LENGTH}
        onChange={(event) => { setComment(event.target.value); setSaved(false); setError(""); }}
        style={{ width: "100%", minHeight: 96, boxSizing: "border-box", resize: "vertical", fontFamily: "inherit" }} />
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginTop: 8 }}>
        <span className="meta">{length}/{TEACHER_FEEDBACK_MAX_LENGTH}</span>
        <button type="submit" disabled={busy || conflict || !length || length > TEACHER_FEEDBACK_MAX_LENGTH || comment.trim() === feedback?.comment}>
          {busy ? t("teaching.feedbackSaving") : t("teaching.feedbackSave")}
        </button>
      </div>
      {conflict && <div role="alert">
        <p className="error">{t("teaching.feedbackConflict")}</p>
        <button type="button" onClick={() => void reloadFeedback()} disabled={busy}>{t("teaching.feedbackReload")}</button>
      </div>}
      {error && <p className="error" role="alert">{error}</p>}
      {saved && <p className="meta" role="status">{t("teaching.feedbackSaved")}</p>}
    </form>
  );
}
