import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/context";
import { MAX_UPLOAD_BYTES } from "../ebpf/models";
import type { LabAttempt } from "./models";
import { loadResumeAttempt, parseResumeTarget } from "./resumeAttempt";
import { TeacherFeedbackDisplay } from "./TeacherFeedback";

type LoadState =
  | { status: "loading" | "invalid" | "kept" | "applied" }
  | { status: "failed"; message: string }
  | { status: "ready"; attempt: LabAttempt };

// The parent keys this component by the complete target, so navigation cannot display an old confirmation.
export function AttemptResumePanel({ engineUrl, attemptId, labId, running, onApply, onKeep }: {
  engineUrl: string; attemptId: unknown; labId: unknown; running: boolean;
  onApply: (attempt: LabAttempt) => boolean; onKeep: () => void;
}) {
  const { t } = useI18n();
  const [state, setState] = useState<LoadState>({ status: "loading" });
  const [retry, setRetry] = useState(0);
  const request = useRef<AbortController | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    request.current = controller;
    setState({ status: "loading" });
    let target;
    try { target = parseResumeTarget(attemptId, labId); }
    catch { setState({ status: "invalid" }); return () => controller.abort(); }
    if (!target) return () => controller.abort();
    void loadResumeAttempt(engineUrl, target, controller.signal, fetch, MAX_UPLOAD_BYTES).then(attempt => {
      if (!controller.signal.aborted) setState({ status: "ready", attempt });
    }).catch(error => {
      if (!controller.signal.aborted) setState({ status: "failed", message: (error as Error).message });
    });
    return () => controller.abort();
  }, [engineUrl, attemptId, labId, retry]);

  const keep = () => {
    request.current?.abort();
    setState({ status: "kept" });
    onKeep();
  };
  if (state.status === "kept" || state.status === "applied") {
    return <div className="learning-context" role="status">{t(`learn.resume${state.status === "kept" ? "Kept" : "Applied"}`)}</div>;
  }
  return <section className="learning-resume" aria-labelledby="resume-title">
    <h3 id="resume-title">{t("learn.resumeTitle")}</h3>
    <p className="meta">{t("learn.resumeHint")}</p>
    {state.status === "loading" && <p role="status">{t("learn.resumeLoading")}</p>}
    {state.status === "invalid" && <p className="error" role="alert">{t("learn.resumeInvalid")}</p>}
    {state.status === "failed" && <p className="error" role="alert">{t("learn.resumeFailed")}: {state.message}</p>}
    {state.status === "ready" && <>
      <p className="meta">{new Date(state.attempt.created_at).toLocaleString()} · {t("teaching.stage")}: {state.attempt.stage}</p>
      <TeacherFeedbackDisplay feedback={state.attempt.teacher_feedback} />
      <details><summary>{t("teaching.source")}</summary><pre>{state.attempt.source}</pre></details>
    </>}
    <div className="row" style={{ marginTop: 12, flexWrap: "wrap" }}>
      {state.status === "ready" && <button type="button" className="button-primary" disabled={running} onClick={() => {
        if (onApply(state.attempt)) setState({ status: "applied" });
      }}>{t("learn.resumeApply")}</button>}
      {state.status === "failed" && <button type="button" onClick={() => setRetry(value => value + 1)}>{t("learn.resumeRetry")}</button>}
      <button type="button" className="button-secondary" onClick={keep}>{t("learn.resumeKeep")}</button>
    </div>
  </section>;
}
