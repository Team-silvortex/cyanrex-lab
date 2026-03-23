import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";

export default function TerminalPage() {
  const { t } = useI18n();
  return (
    <SidebarLayout title={t("terminal.title")}>
      <section className="panel">
        <h2>{t("terminal.title")}</h2>
        <p className="meta">{t("terminal.subtitle")}</p>
      </section>
    </SidebarLayout>
  );
}
