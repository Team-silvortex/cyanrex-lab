import type { ReactNode } from "react";

import { useI18n } from "../../i18n/context";
import { labTitleKey, type LabAttempt } from "./models";
import { TeacherFeedbackDisplay } from "./TeacherFeedback";

export function AttemptCard({ attempt, children }: { attempt: LabAttempt; children?: ReactNode }) {
  const { t } = useI18n();
  const attachment = !attempt.attach_expected
    ? t("teaching.attachmentNotExpected")
    : attempt.attach_verified ? t("teaching.attachmentVerified") : t("teaching.attachmentMissing");
  return (
    <article style={{ borderTop: "1px solid var(--line)", paddingTop: 12 }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <strong>{t(labTitleKey(attempt.lab_id))}</strong>
        <span className={`learning-status ${attempt.completed ? "completed" : "in_progress"}`}>
          {attempt.completed ? t("teaching.attemptCompleted") : t("teaching.attemptIncomplete")}
        </span>
      </div>
      <p className="meta">
        {new Date(attempt.created_at).toLocaleString()} · {t("teaching.stage")}: {attempt.stage} · {attachment}
      </p>
      <p className="meta">
        {t("teaching.runResult")}: {attempt.run_success ? t("teaching.runSucceeded") : t("teaching.runFailed")}
        {attempt.template_id ? ` · ${t("teaching.template")}: ${attempt.template_id}` : ""}
      </p>
      {attempt.feedback.length > 0 && <div>
        <strong>{t("teaching.feedback")}</strong>
        <ul>{attempt.feedback.map((item, index) => <li key={index}>{item}</li>)}</ul>
      </div>}
      <details>
        <summary>{t("teaching.source")}</summary>
        <pre style={{ maxHeight: 360, overflow: "auto" }}>{attempt.source}</pre>
      </details>
      {children ?? <TeacherFeedbackDisplay feedback={attempt.teacher_feedback} />}
    </article>
  );
}
