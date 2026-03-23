import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";

export default function DashboardPage() {
  const { t } = useI18n();
  return (
    <SidebarLayout title={t("dashboard.title")}>
      <section className="panel">
        <h2>{t("dashboard.title")}</h2>
        <p className="meta">{t("dashboard.subtitle")}</p>
      </section>

      <section className="grid cols-2" style={{ marginTop: 16 }}>
        <article className="panel">
          <h3>{t("dashboard.systemHealth")}</h3>
          <p className="meta">Engine API: <code>/health</code></p>
          <p>Use the sidebar to run eBPF programs and inspect responses.</p>
        </article>
        <article className="panel">
          <h3>{t("dashboard.quickActions")}</h3>
          <p className="meta">Open <code>eBPF Runner</code> to upload C code and execute loader pipeline.</p>
        </article>
      </section>
    </SidebarLayout>
  );
}
