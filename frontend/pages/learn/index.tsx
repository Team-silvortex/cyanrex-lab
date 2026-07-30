import { useI18n } from "../../src/i18n/context";
import Link from "next/link";

import SidebarLayout from "../../src/components/SidebarLayout";

const sections = [
  { href: "/learn/teacher-guide", titleKey: "learn.sectionTeacherGuideTitle", detailKey: "learn.sectionTeacherGuideDetail" },
  { href: "/learn/student-guide", titleKey: "learn.sectionStudentGuideTitle", detailKey: "learn.sectionStudentGuideDetail" },
  { href: "/learn/concepts", titleKey: "learn.sectionConceptsTitle", detailKey: "learn.sectionConceptsDetail" },
  { href: "/learn/labs/01-first-program", titleKey: "learn.sectionLab01Title", detailKey: "learn.sectionLab01Detail" },
  { href: "/learn/labs/02-trace-execve", titleKey: "learn.sectionLab02Title", detailKey: "learn.sectionLab02Detail" },
  { href: "/learn/labs/03-map-counter", titleKey: "learn.sectionLab03Title", detailKey: "learn.sectionLab03Detail" },
  { href: "/learn/labs/04-ring-buffer", titleKey: "learn.sectionLab04Title", detailKey: "learn.sectionLab04Detail" },
  { href: "/learn/labs/05-verifier-debugging", titleKey: "learn.sectionLab05Title", detailKey: "learn.sectionLab05Detail" },
  { href: "/learn/troubleshooting", titleKey: "learn.sectionTroubleshootingTitle", detailKey: "learn.sectionTroubleshootingDetail" },
  { href: "/learn/security", titleKey: "learn.sectionSecurityTitle", detailKey: "learn.sectionSecurityDetail" },
];

export default function LearnIndexPage() {
  const { t } = useI18n();
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
    </SidebarLayout>
  );
}
