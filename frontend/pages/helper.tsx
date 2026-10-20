import { useRouter } from "next/router";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
import { useI18n } from "../src/i18n/context";
import { localizeLegacyText } from "../src/i18n/legacyText";
import { useEnvironmentCheck } from "../src/features/helper/useEnvironmentCheck";

export default function HelperPage() {
  const { t } = useI18n();
  const router = useRouter();
  const { snapshot, loading, errorKey, runCheck } = useEnvironmentCheck(getEngineUrl(), router.asPath);
  const report = snapshot?.report, runner = snapshot?.runner;

  return (
    <SidebarLayout title={t("helper.title")}>
      <section className="panel">
        <h2>{t("helper.title")}</h2>
        <p className="meta">{t("helper.subtitle")}</p>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={() => void runCheck()} disabled={loading}>
            {loading ? t("helper.checking") : t("helper.runCheck")}
          </button>
        </div>

        {!snapshot && !loading && !errorKey && <p className="meta" role="status">{t("helper.explicitCheckHint")}</p>}
        {loading && <p className="meta" role="status">{t("helper.checking")}</p>}
        {errorKey && <p className="error" role="alert" style={{ marginTop: 12 }}>{t(errorKey)}</p>}

        {report && (
            <div style={{ marginTop: 16 }}>
            <p>
              <strong>{t("helper.overall")}:</strong> {report.overall_ok ? t("helper.ok") : t("helper.notReady")}
            </p>
            <p>
              <strong>{t("helper.backend")}:</strong> <code>{report.runtime_mode}</code>
            </p>
            <p className="meta">{localizeLegacyText(report.runtime_guidance, t)}</p>
            <p className="meta">
              {t("helper.generatedAt")}: {new Date(report.generated_at).toLocaleString()}
            </p>

            <div className="grid" style={{ marginTop: 10 }}>
              {report.checks.map((check) => (
                <article key={check.name} className="panel" style={{ background: "#0b1425" }}>
                  <p>
                    <strong>{localizeLegacyText(check.name, t)}</strong>: {check.ok ? t("helper.ok") : t("helper.fail")}
                  </p>
                  <p className="meta" style={{ margin: 0 }}>{localizeLegacyText(check.detail, t)}</p>
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
