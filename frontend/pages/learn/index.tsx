import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

import SidebarLayout from "../../src/components/SidebarLayout";
import { getEngineUrl } from "../../src/config/runtime";
import { useI18n } from "../../src/i18n/context";
import { LabProgress, labTitleKey } from "../../src/features/learning/models";

const sections = [
  { href: "/learn/teacher-guide", titleKey: "learn.sectionTeacherGuideTitle", detailKey: "learn.sectionTeacherGuideDetail" },
  { href: "/learn/architecture", titleKey: "learn.sectionArchitectureTitle", detailKey: "learn.sectionArchitectureDetail" },
  { href: "/learn/student-guide", titleKey: "learn.sectionStudentGuideTitle", detailKey: "learn.sectionStudentGuideDetail" },
  { href: "/learn/concepts", titleKey: "learn.sectionConceptsTitle", detailKey: "learn.sectionConceptsDetail" },
  { href: "/learn/troubleshooting", titleKey: "learn.sectionTroubleshootingTitle", detailKey: "learn.sectionTroubleshootingDetail" },
  { href: "/learn/security", titleKey: "learn.sectionSecurityTitle", detailKey: "learn.sectionSecurityDetail" },
];

export default function LearnIndexPage() {
  const { t } = useI18n();
  const [labs, setLabs] = useState<LabProgress[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const engineUrl = useMemo(getEngineUrl, []);

  useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const response = await fetch(`${engineUrl}/learning/labs`, { credentials: "include" });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const payload = (await response.json()) as LabProgress[];
        if (active) setLabs(payload);
      } catch (cause) {
        if (active) setError(`${t("learn.progressLoadFailed")}: ${(cause as Error).message}`);
      } finally {
        if (active) setLoading(false);
      }
    };
    void load();
    return () => { active = false; };
  }, [engineUrl, t]);

  const completed = labs.filter((lab) => lab.status === "completed").length;

  return (
    <SidebarLayout title={t("learn.title")}>
      <section className="panel">
        <p className="brand-kicker">CYANREX COURSE</p>
        <h2 style={{ marginTop: 4 }}>{t("learn.heading")}</h2>
        <p className="meta">{t("learn.subtitle")}</p>
        <div className="grid cols-2" style={{ marginTop: 16 }}>
          {sections.map((section) => (
            <Link key={section.href} href={section.href} className="panel" style={{ display: "block", textDecoration: "none", background: "#0b1425" }}>
              <strong>{t(section.titleKey)}</strong>
              <p className="meta" style={{ marginBottom: 0 }}>{t(section.detailKey)}</p>
            </Link>
          ))}
        </div>
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <h2 style={{ marginBottom: 4 }}>{t("learn.progressTitle")}</h2>
            <p className="meta" style={{ margin: 0 }}>
              {t("learn.progressSummary", { completed, total: labs.length })}
            </p>
          </div>
          {labs.length > 0 && <strong>{Math.round((completed / labs.length) * 100)}%</strong>}
        </div>
        {loading && <p className="meta">{t("learn.progressLoading")}</p>}
        {error && <p className="error">{error}</p>}
        <div className="grid cols-2" style={{ marginTop: 16 }}>
          {labs.map((progress) => (
            <article className="panel" key={progress.lab.id} style={{ background: "#0b1425" }}>
              <div className="row" style={{ justifyContent: "space-between" }}>
                <strong>{t(labTitleKey(progress.lab.id))}</strong>
                <span className={`learning-status ${progress.status}`}>
                  {t(`learn.status.${progress.status}`)}
                </span>
              </div>
              <p className="meta">{progress.lab.summary}</p>
              <p className="meta">
                {t("learn.attemptCount", { count: progress.attempts })}
                {progress.latest_stage ? ` · ${progress.latest_stage}` : ""}
              </p>
              {progress.latest_feedback.length > 0 && (
                <p className="meta">{progress.latest_feedback[0]}</p>
              )}
              <div className="row">
                <Link href={`/learn/${progress.lab.doc_slug}`}>{t("learn.openLab")}</Link>
                <Link href={`/ebpf?lab=${encodeURIComponent(progress.lab.id)}`}>
                  {t("learn.openEditor")}
                </Link>
              </div>
            </article>
          ))}
        </div>
      </section>
    </SidebarLayout>
  );
}
