import { useCallback, useEffect, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
import type { TeacherLearningOverview } from "../src/features/learning/models";
import { useI18n } from "../src/i18n/context";

export default function TeachingPage() {
  const { t } = useI18n();
  const engineUrl = useMemo(getEngineUrl, []);
  const [overview, setOverview] = useState<TeacherLearningOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

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
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </SidebarLayout>
  );
}

function formatTime(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "—" : date.toLocaleString();
}
