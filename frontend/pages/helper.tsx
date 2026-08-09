import { useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
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
  runtime_mode: "native-linux" | "wsl2" | "docker";
  runtime_guidance: string;
  checks: EnvironmentCheckItem[];
};

type RunnerStatus = {
  mode: string;
  isolation: string;
  instance_id: string;
  max_concurrent: number;
  max_per_user: number;
  active_total: number;
  active_for_current_user: number;
  available_slots: number;
  execution_timeout_seconds: number;
};

export default function HelperPage() {
  const { t } = useI18n();
  const [report, setReport] = useState<EnvironmentReport | null>(() =>
    loadPageState<EnvironmentReport>("helper_report_v1"),
  );
  const [runner, setRunner] = useState<RunnerStatus | null>(() =>
    loadPageState<RunnerStatus>("helper_runner_status_v1"),
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(() =>
    loadPageState<string>("helper_error_v1"),
  );

  const engineUrl = getEngineUrl();

  const runCheck = async () => {
    setLoading(true);
    setError(null);

    try {
      const [environmentResponse, runnerResponse] = await Promise.all([
        fetch(`${engineUrl}/helper/environment`, { credentials: "include" }),
        fetch(`${engineUrl}/runner/status`, { credentials: "include" }),
      ]);
      if (!environmentResponse.ok || !runnerResponse.ok) {
        throw new Error(`HTTP ${environmentResponse.status}/${runnerResponse.status}`);
      }
      const environmentJson = (await environmentResponse.json()) as EnvironmentReport;
      const runnerJson = (await runnerResponse.json()) as RunnerStatus;
      setReport(environmentJson);
      setRunner(runnerJson);
      savePageState("helper_report_v1", environmentJson);
      savePageState("helper_runner_status_v1", runnerJson);
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
            <p>
              <strong>{t("helper.backend")}:</strong> <code>{report.runtime_mode}</code>
            </p>
            <p className="meta">{report.runtime_guidance}</p>
            <p className="meta">
              {t("helper.generatedAt")}: {new Date(report.generated_at).toLocaleString()}
            </p>

            <div className="grid" style={{ marginTop: 10 }}>
              {report.checks.map((check) => (
                <article key={check.name} className="panel" style={{ background: "#0b1425" }}>
                  <p>
                    <strong>{check.name}</strong>: {check.ok ? t("helper.ok") : t("helper.fail")}
                  </p>
                  <p className="meta" style={{ margin: 0 }}>{check.detail}</p>
                </article>
              ))}
            </div>
          </div>
        )}
      </section>

      {runner && (
        <section className="panel">
          <h2>{t("helper.runnerStatus")}</h2>
          <div className="grid" style={{ marginTop: 10 }}>
            <article className="panel" style={{ background: "#0b1425" }}>
              <strong>{t("helper.runnerMode")}</strong>
              <p><code>{runner.mode}</code></p>
              <p className="meta">{runner.instance_id}</p>
            </article>
            <article className="panel" style={{ background: "#0b1425" }}>
              <strong>{t("helper.runnerCapacity")}</strong>
              <p>{runner.active_total}/{runner.max_concurrent}</p>
              <p className="meta">{t("helper.runnerAvailable")}: {runner.available_slots}</p>
            </article>
            <article className="panel" style={{ background: "#0b1425" }}>
              <strong>{t("helper.runnerUserCapacity")}</strong>
              <p>{runner.active_for_current_user}/{runner.max_per_user}</p>
              <p className="meta">
                {t("helper.runnerTimeout")}: {runner.execution_timeout_seconds}s
              </p>
            </article>
          </div>
          <p className="error" style={{ marginTop: 12 }}>
            {t("helper.runnerIsolation")}: <code>{runner.isolation}</code> — {t("helper.runnerSharedKernelWarning")}
          </p>
        </section>
      )}
    </SidebarLayout>
  );
}
