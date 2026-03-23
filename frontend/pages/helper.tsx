import { useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { useI18n } from "../src/i18n/context";
import { loadPageState, savePageState } from "../src/utils/pageState";

type EnvironmentCheckItem = {
  name: string;
  ok: boolean;
  detail: string;
};

type EnvironmentReport = {
  overall_ok: boolean;
  generated_at: string;
  checks: EnvironmentCheckItem[];
};

export default function HelperPage() {
  const { t } = useI18n();
  const [report, setReport] = useState<EnvironmentReport | null>(() =>
    loadPageState<EnvironmentReport>("helper_report_v1"),
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(() =>
    loadPageState<string>("helper_error_v1"),
  );

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  const runCheck = async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await fetch(`${engineUrl}/helper/environment`, {
        credentials: "include",
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const json = (await response.json()) as EnvironmentReport;
      setReport(json);
      savePageState("helper_report_v1", json);
      savePageState("helper_error_v1", "");
    } catch (err) {
      const msg = (err as Error).message;
      setError(msg);
      savePageState("helper_error_v1", msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <SidebarLayout title={t("helper.title")}>
      <section className="panel">
        <h2>{t("helper.title")}</h2>
        <p className="meta">{t("helper.subtitle")}</p>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={runCheck} disabled={loading}>
            {loading ? t("helper.checking") : t("helper.runCheck")}
          </button>
        </div>

        {error && <p className="error" style={{ marginTop: 12 }}>{error}</p>}

        {report && (
          <div style={{ marginTop: 16 }}>
            <p>
              <strong>{t("helper.overall")}:</strong> {report.overall_ok ? t("helper.ok") : t("helper.notReady")}
            </p>
            <p className="meta">
              {t("helper.generatedAt")}: {new Date(report.generated_at).toLocaleString()}
            </p>

            <div className="grid" style={{ marginTop: 10 }}>
              {report.checks.map((check) => (
                <article key={check.name} className="panel" style={{ background: "#0b1425" }}>
                  <p>
                    <strong>{check.name}</strong>: {check.ok ? t("helper.ok") : "FAIL"}
                  </p>
                  <p className="meta" style={{ margin: 0 }}>{check.detail}</p>
                </article>
              ))}
            </div>
          </div>
        )}
      </section>
    </SidebarLayout>
  );
}
