import { useMemo } from "react";
import Link from "next/link";
import { useRouter } from "next/router";

import SidebarLayout from "../src/components/SidebarLayout";
import { useConfirmedAction } from "../src/components/useConfirmedAction";
import { getEngineUrl } from "../src/config/runtime";
import { DOCS_LINK_STYLE, DOCS_QUICK_LINKS } from "../src/config/settings";
import RunnerAgentAdminPanel from "../src/features/runner/RunnerAgentAdminPanel";
import PerformanceMetricsPanel from "../src/features/settings/PerformanceMetricsPanel";
import { usePerformanceMetrics } from "../src/features/settings/usePerformanceMetrics";
import { useSettingsForm } from "../src/features/settings/useSettingsForm";
import type { EventSettings } from "../src/features/settings/settingsRequest";
import { useI18n } from "../src/i18n/context";

export default function SettingsPage() {
  const { t } = useI18n();
  const router = useRouter();
  const safety = useConfirmedAction();
  const engineUrl = useMemo(getEngineUrl, []);
  const performance = usePerformanceMetrics(engineUrl, router.asPath);
  const form = useSettingsForm(engineUrl, router.asPath, t);
  const controlsDisabled = !form.ready || safety.busy;

  const confirmSave = () => {
    const snapshot = form.review();
    if (!snapshot) return;
    safety.request({
      action: t("settings.save"), description: t("safety.settings"), details: [
        { label: t("settings.maxRecords"), value: String(snapshot.events.max_records) },
        { label: t("settings.overflowPolicy"), value: snapshot.events.overflow_policy },
        ...(snapshot.compiler ? [{ label: t("settings.residentCompiler"), value: String(snapshot.compiler.resident) }] : []),
      ],
    }, signal => form.save(snapshot, signal));
  };
  const reload = () => {
    if (form.dirty) safety.request({ action: t("settings.reload"), description: t("settings.reloadHint"), dangerous: false }, async () => form.reload());
    else form.reload();
  };

  return (
    <SidebarLayout title={t("settings.title")}>
      {safety.dialog}
      <section className="panel">
        <h2>{t("settings.title")}</h2>
        <p className="meta">{t("settings.subtitle")}</p>

        <div className="grid cols-2" style={{ marginTop: 12 }}>
          <label className="meta">
            {t("settings.maxRecords")}
            <input
              type="number" min={50} max={50000} step={1}
              value={Number.isNaN(form.draft.max_records) ? "" : form.draft.max_records}
              disabled={controlsDisabled}
              onChange={(event) => form.changeEvents({ ...form.draft, max_records: event.target.value === "" ? Number.NaN : Number(event.target.value) })}
              style={{ marginTop: 6, width: "100%" }}
            />
          </label>
          <label className="meta">
            {t("settings.overflowPolicy")}
            <select
              value={form.draft.overflow_policy}
              disabled={controlsDisabled}
              onChange={(event) => form.changeEvents({ ...form.draft, overflow_policy: event.target.value as EventSettings["overflow_policy"] })}
              style={{ marginTop: 6, width: "100%" }}
            >
              <option value="drop_oldest">{t("settings.dropOldest")}</option>
              <option value="drop_new">{t("settings.dropNew")}</option>
            </select>
          </label>
        </div>

        <p className="meta" style={{ marginTop: 10 }}>
          {form.draft.overflow_policy === "drop_oldest" ? t("settings.dropOldestHint") : t("settings.dropNewHint")}
        </p>
        {form.compiler && (
          <label className="row" style={{ marginTop: 16, alignItems: "flex-start" }}>
            <input
              type="checkbox" checked={form.resident} disabled={controlsDisabled}
              onChange={(event) => form.changeResident(event.target.checked)}
              style={{ marginTop: 3 }}
            />
            <span>
              <strong>{t("settings.residentCompiler")}</strong>
              <span className="meta" style={{ display: "block", marginTop: 4 }}>
                {form.resident ? t("settings.residentCompilerEnabledHint") : t("settings.residentCompilerDisabledHint")}
              </span>
            </span>
          </label>
        )}
        {!form.loading && form.events && !form.compiler && <p role="status" className="meta">{t("settings.compilerUnavailable")}</p>}

        <DocumentationLinks />
        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" disabled={controlsDisabled} onClick={confirmSave}>
            {form.saving ? t("settings.saving") : t("settings.save")}
          </button>
          <button type="button" disabled={form.loading || form.saving || safety.busy} onClick={reload} title={t("settings.reloadHint")}>
            {t("settings.reload")}
          </button>
          <button type="button" onClick={() => { void performance.refresh(); }} disabled={performance.refreshing}>
            {performance.refreshing ? t("settings.metricsRefreshing") : t("settings.refreshMetrics")}
          </button>
          {form.loading && <span className="meta">{t("settings.loading")}</span>}
        </div>

        {form.messageKey && <p role="status" className="meta" style={{ color: "#9cd67a" }}>{t(form.messageKey)}</p>}
        {form.errorKey && <p role="alert" className="error">{t(form.errorKey)}</p>}

        <PerformanceMetricsPanel metrics={performance.metrics} summary={performance.hotspotSummary}
          refreshing={performance.refreshing} stale={performance.stale} error={performance.error} message={performance.message} />
        <RunnerAgentAdminPanel engineUrl={engineUrl} />
      </section>
    </SidebarLayout>
  );
}

function DocumentationLinks() {
  const { t } = useI18n();
  return (
    <section className="panel" style={{ marginTop: 12 }}>
      <strong>{t("settings.docsPanelTitle")}</strong>
      <p className="meta" style={{ marginTop: 4 }}>{t("settings.docsPanelDescription")}</p>
      <div className="row" style={{ marginTop: 10 }}>
        <Link href="/learn" style={DOCS_LINK_STYLE}>{t("layout.nav.learn")}</Link>
        <Link href="/learn/troubleshooting" style={DOCS_LINK_STYLE}>
          {t("settings.docsTroubleshoot")}
        </Link>
      </div>
      <p className="meta" style={{ marginTop: 14 }}>{t("settings.docsQuickTitle")}</p>
      <p className="meta" style={{ marginTop: 4 }}>{t("settings.docsQuickHint")}</p>
      <div className="grid cols-2" style={{ marginTop: 8 }}>
        {DOCS_QUICK_LINKS.map((item) => (
          <Link href={item.href} key={item.href} style={DOCS_LINK_STYLE}>{t(item.titleKey)}</Link>
        ))}
      </div>
    </section>
  );
}
