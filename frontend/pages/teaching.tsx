import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
import type {
  LabAttempt,
  TeacherLearningOverview,
  TeacherStudentAttempts,
} from "../src/features/learning/models";
import { buildTeacherAttemptsUrl } from "../src/features/learning/teacherReview";
import { useI18n } from "../src/i18n/context";

export default function TeachingPage() {
  const { t } = useI18n();
  const engineUrl = useMemo(getEngineUrl, []);
  const [overview, setOverview] = useState<TeacherLearningOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [selectedUsername, setSelectedUsername] = useState("");
  const [attemptReview, setAttemptReview] = useState<TeacherStudentAttempts | null>(null);
  const [attemptLoading, setAttemptLoading] = useState(false);
  const [attemptError, setAttemptError] = useState("");
  const attemptRequestRef = useRef(0);

  const loadOverview = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const response = await fetch(`${engineUrl}/learning/teacher/overview`, {
        credentials: "include",
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      setOverview((await response.json()) as TeacherLearningOverview);
    } catch (cause) {
      setError(`${t("teaching.loadFailed")}: ${(cause as Error).message}`);
    } finally {
      setLoading(false);
    }
  }, [engineUrl, t]);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  const loadAttempts = useCallback(async (username: string) => {
    const requestId = attemptRequestRef.current + 1;
    attemptRequestRef.current = requestId;
    setSelectedUsername(username);
    setAttemptReview(null);
    setAttemptLoading(true);
    setAttemptError("");
    try {
      const response = await fetch(buildTeacherAttemptsUrl(engineUrl, username), {
        credentials: "include",
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const payload = (await response.json()) as TeacherStudentAttempts;
      if (requestId !== attemptRequestRef.current) return;
      setAttemptReview(payload);
    } catch (cause) {
      if (requestId !== attemptRequestRef.current) return;
      setAttemptError(`${t("teaching.reviewFailed")}: ${(cause as Error).message}`);
    } finally {
      if (requestId === attemptRequestRef.current) setAttemptLoading(false);
    }
  }, [engineUrl, t]);

  return (
    <SidebarLayout title={t("teaching.title")}>
      <section className="panel">
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <p className="brand-kicker">CYANREX CLASSROOM</p>
            <h2 style={{ marginTop: 4 }}>{t("teaching.title")}</h2>
            <p className="meta">{t("teaching.subtitle")}</p>
          </div>
          <button type="button" onClick={loadOverview} disabled={loading}>
            {loading ? t("common.checking") : t("common.refresh")}
          </button>
        </div>
        {error && <p className="error">{error}</p>}
      </section>

      <section className="grid cols-2" style={{ marginTop: 16 }}>
        <article className="panel">
          <p className="meta">{t("teaching.activeStudents")}</p>
          <strong className="metric-value">{overview?.active_students ?? 0}</strong>
        </article>
        <article className="panel">
          <p className="meta">{t("teaching.totalLabs")}</p>
          <strong className="metric-value">{overview?.total_labs ?? 0}</strong>
        </article>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <h3>{t("teaching.studentProgress")}</h3>
        {!loading && overview?.students.length === 0 && (
          <p className="meta">{t("teaching.noActivity")}</p>
        )}
        <div style={{ overflowX: "auto" }}>
          <table>
            <thead>
              <tr>
                <th>{t("teaching.student")}</th>
                <th>{t("teaching.completed")}</th>
                <th>{t("teaching.attempts")}</th>
                <th>{t("teaching.lastActivity")}</th>
                <th>{t("teaching.labStates")}</th>
                <th>{t("teaching.action")}</th>
              </tr>
            </thead>
            <tbody>
              {overview?.students.map((student) => (
                <tr key={student.username}>
                  <td><strong>{student.username}</strong></td>
                  <td>{student.completed_labs}/{student.total_labs}</td>
                  <td>{student.total_attempts}</td>
                  <td>{formatTime(student.last_activity_at)}</td>
                  <td>
                    <div className="learning-status-row">
                      {student.labs.map((progress) => (
                        <span
                          className={`learning-status ${progress.status}`}
                          key={progress.lab.id}
                          title={`${progress.lab.title}: ${progress.status}`}
                        >
                          {progress.lab.position}
                        </span>
                      ))}
                    </div>
                  </td>
                  <td>
                    <button
                      type="button"
                      onClick={() => void loadAttempts(student.username)}
                      disabled={attemptLoading && selectedUsername === student.username}
                    >
                      {attemptLoading && selectedUsername === student.username
                        ? t("teaching.reviewLoading")
                        : t("teaching.reviewAction")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {selectedUsername && (
        <AttemptReviewPanel
          username={selectedUsername}
          review={attemptReview}
          loading={attemptLoading}
          error={attemptError}
          t={t}
        />
      )}
    </SidebarLayout>
  );
}

type Translate = (key: string, vars?: Record<string, string | number>) => string;

function AttemptReviewPanel({
  username,
  review,
  loading,
  error,
  t,
}: {
  username: string;
  review: TeacherStudentAttempts | null;
  loading: boolean;
  error: string;
  t: Translate;
}) {
  return (
    <section className="panel" style={{ marginTop: 16 }}>
      <h3>{t("teaching.reviewTitle")} · {username}</h3>
      {loading && <p className="meta">{t("teaching.reviewLoading")}</p>}
      {error && <p className="error">{error}</p>}
      {!loading && !error && review?.attempts.length === 0 && (
        <p className="meta">{t("teaching.reviewNoAttempts")}</p>
      )}
      <div className="grid" style={{ gap: 12 }}>
        {review?.attempts.map((attempt) => (
          <AttemptCard key={attempt.id} attempt={attempt} t={t} />
        ))}
      </div>
    </section>
  );
}

function AttemptCard({ attempt, t }: { attempt: LabAttempt; t: Translate }) {
  const attachment = !attempt.attach_expected
    ? t("teaching.attachmentNotExpected")
    : attempt.attach_verified
      ? t("teaching.attachmentVerified")
      : t("teaching.attachmentMissing");
  return (
    <article style={{ borderTop: "1px solid var(--line)", paddingTop: 12 }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
        <strong>{attempt.lab_id}</strong>
        <span className={`learning-status ${attempt.completed ? "completed" : "in_progress"}`}>
          {attempt.completed ? t("teaching.attemptCompleted") : t("teaching.attemptIncomplete")}
        </span>
      </div>
      <p className="meta">
        {formatTime(attempt.created_at)} · {t("teaching.stage")}: {attempt.stage} · {attachment}
      </p>
      <p className="meta">
        {t("teaching.runResult")}: {attempt.run_success
          ? t("teaching.runSucceeded")
          : t("teaching.runFailed")}
        {attempt.template_id ? ` · ${t("teaching.template")}: ${attempt.template_id}` : ""}
      </p>
      {attempt.feedback.length > 0 && (
        <div>
          <strong>{t("teaching.feedback")}</strong>
          <ul>{attempt.feedback.map((item) => <li key={item}>{item}</li>)}</ul>
        </div>
      )}
      <details>
        <summary>{t("teaching.source")}</summary>
        <pre style={{ maxHeight: 360, overflow: "auto" }}>{attempt.source}</pre>
      </details>
    </article>
  );
}

function formatTime(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString();
}
