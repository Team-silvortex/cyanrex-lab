import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
import type {
  TeacherLearningOverview,
  TeacherStudentAttempts,
} from "../src/features/learning/models";
import { buildTeacherAttemptsUrl } from "../src/features/learning/teacherReview";
import { AttemptCard } from "../src/features/learning/AttemptCard";
import { TeacherFeedbackEditor } from "../src/features/learning/TeacherFeedback";
import { useI18n } from "../src/i18n/context";
import { InvitationPanel } from "../src/features/classroom/InvitationPanel";

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
      <header className="page-header">
        <div>
          <p className="brand-kicker">CYANREX CLASSROOM</p>
          <h2 style={{ marginTop: 4 }}>{t("teaching.title")}</h2>
          <p className="meta">{t("teaching.subtitle")}</p>
        </div>
        <button type="button" className="button-secondary" onClick={loadOverview} disabled={loading}>
          {loading ? t("common.checking") : t("common.refresh")}
        </button>
      </header>
      {error && <p className="error" role="alert">{error}</p>}
      <InvitationPanel />

      <section className="teaching-summary">
        <article className="panel">
          <p className="meta">{t("teaching.activeStudents")}</p>
          <strong className="metric-value">{overview?.active_students ?? 0}</strong>
        </article>
        <article className="panel">
          <p className="meta">{t("teaching.totalLabs")}</p>
          <strong className="metric-value">{overview?.total_labs ?? 0}</strong>
        </article>
      </section>

      <section className="panel" id="student-progress" tabIndex={-1}>
        <h3>{t("teaching.studentProgress")}</h3>
        {!loading && overview?.students.length === 0 && (
          <p className="meta">{t("teaching.noActivity")}</p>
        )}
        <table className="teaching-table" role="table" aria-label={t("teaching.studentProgress")}>
          <thead role="rowgroup">
            <tr role="row">
              <th role="columnheader" scope="col">{t("teaching.student")}</th>
              <th role="columnheader" scope="col">{t("teaching.completed")}</th>
              <th role="columnheader" scope="col">{t("teaching.attempts")}</th>
              <th role="columnheader" scope="col">{t("teaching.lastActivity")}</th>
              <th role="columnheader" scope="col">{t("teaching.labStates")}</th>
              <th role="columnheader" scope="col">{t("teaching.action")}</th>
            </tr>
          </thead>
          <tbody role="rowgroup">
            {overview?.students.map((student) => (
              <tr key={student.username} role="row" className={selectedUsername === student.username ? "selected" : undefined}>
                <td role="cell" data-label={t("teaching.student")}><strong>{student.username}</strong></td>
                <td role="cell" data-label={t("teaching.completed")}>{student.completed_labs}/{student.total_labs}</td>
                <td role="cell" data-label={t("teaching.attempts")}>{student.total_attempts}</td>
                <td role="cell" data-label={t("teaching.lastActivity")}>{formatTime(student.last_activity_at)}</td>
                <td role="cell" data-label={t("teaching.labStates")}>
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
                <td role="cell" data-label={t("teaching.action")}>
                  <button
                    type="button"
                    className="button-secondary"
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
      </section>

      {selectedUsername && (
        <AttemptReviewPanel
          username={selectedUsername}
          review={attemptReview}
          loading={attemptLoading}
          error={attemptError}
          engineUrl={engineUrl}
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
  engineUrl,
  t,
}: {
  username: string;
  review: TeacherStudentAttempts | null;
  loading: boolean;
  error: string;
  engineUrl: string;
  t: Translate;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // Focus the selected review, not a disappearing loading control; feedback edits stay mounted.
    heading.current?.focus({ preventScroll: true });
    heading.current?.scrollIntoView({ block: "start" });
  }, [username, loading]);

  return (
    <section className="panel attempt-review" aria-labelledby="attempt-review-title">
      <div className="section-heading">
        <h3 id="attempt-review-title" tabIndex={-1} ref={heading}>{t("teaching.reviewTitle")} · {username}</h3>
        <a href="#student-progress">{t("teaching.backToStudents")}</a>
      </div>
      {loading && <p className="meta">{t("teaching.reviewLoading")}</p>}
      {error && <p className="error">{error}</p>}
      {!loading && !error && review?.attempts.length === 0 && (
        <p className="meta">{t("teaching.reviewNoAttempts")}</p>
      )}
      <div className="grid" style={{ gap: 12 }}>
        {review?.attempts.map((attempt) => (
          <AttemptCard key={attempt.id} attempt={attempt}>
            <TeacherFeedbackEditor attempt={attempt} engineUrl={engineUrl} />
          </AttemptCard>
        ))}
      </div>
    </section>
  );
}

function formatTime(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString();
}
