import { useEffect, useMemo, useRef, useState } from "react";
import Link from "next/link";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl } from "../src/config/runtime";
import { DOCS_LINK_STYLE, DOCS_QUICK_LINKS } from "../src/config/settings";
import RunnerAgentAdminPanel from "../src/features/runner/RunnerAgentAdminPanel";
import PerformanceMetricsPanel from "../src/features/settings/PerformanceMetricsPanel";
import { usePerformanceMetrics } from "../src/features/settings/usePerformanceMetrics";
import { useI18n } from "../src/i18n/context";
import { loadPageState, savePageState } from "../src/utils/pageState";

type EventOverflowPolicy = "drop_oldest" | "drop_new";

type EventSettingsResponse = {
  max_records: number;
  overflow_policy: EventOverflowPolicy;
};

type CompilerSettingsResponse = {
  resident: boolean;
  strategy: "resident_cache" | "on_demand";
};

export default function SettingsPage() {
  const { t } = useI18n();
  const engineUrl = useMemo(getEngineUrl, []);
  const performance = usePerformanceMetrics(engineUrl);
  const [maxRecords, setMaxRecords] = useState(
    () => loadPageState<number>("settings_event_max_records_v1") ?? 500,
  );
  const [overflowPolicy, setOverflowPolicy] = useState<EventOverflowPolicy>(
    () => loadPageState<EventOverflowPolicy>("settings_event_overflow_policy_v1") ?? "drop_oldest",
  );
  const [residentCompiler, setResidentCompiler] = useState(false);
  const [compilerSettingsAvailable, setCompilerSettingsAvailable] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    const load = async () => {
      setLoading(true);
      setError("");
      try {
        const [eventsResponse, compilerResponse] = await Promise.all([
          fetch(`${engineUrl}/settings/events`, { credentials: "include" }),
          fetch(`${engineUrl}/settings/compiler`, { credentials: "include" }),
        ]);
        if (!eventsResponse.ok) throw new Error(`HTTP ${eventsResponse.status}`);
        const events = (await eventsResponse.json()) as EventSettingsResponse;
        if (!mounted.current) return;
        setMaxRecords(events.max_records);
        setOverflowPolicy(events.overflow_policy);
        if (compilerResponse.ok) {
          const compiler = (await compilerResponse.json()) as CompilerSettingsResponse;
          setResidentCompiler(compiler.resident);
          setCompilerSettingsAvailable(true);
        }
      } catch (err) {
        if (mounted.current) setError((err as Error).message);
      } finally {
        if (mounted.current) setLoading(false);
      }
    };
    void load();
    return () => {
      mounted.current = false;
    };
  }, [engineUrl]);

  useEffect(() => {
    savePageState("settings_event_max_records_v1", maxRecords);
    savePageState("settings_event_overflow_policy_v1", overflowPolicy);
  }, [maxRecords, overflowPolicy]);

  const save = async () => {
    setSaving(true);
    setError("");
    setMessage("");
    performance.clearFeedback();
    try {
      const response = await fetch(`${engineUrl}/settings/events`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify({
          max_records: Math.max(50, Math.min(50000, Number(maxRecords) || 500)),
          overflow_policy: overflowPolicy,
        }),
      });
      const payload = (await response.json()) as {
        ok: boolean;
        message: string;
        settings?: EventSettingsResponse;
      };
      if (!response.ok || !payload.ok) {
        throw new Error(payload.message || `HTTP ${response.status}`);
      }
      if (payload.settings) {
        setMaxRecords(payload.settings.max_records);
        setOverflowPolicy(payload.settings.overflow_policy);
      }
      if (compilerSettingsAvailable) {
        const compilerResponse = await fetch(`${engineUrl}/settings/compiler`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          body: JSON.stringify({ resident: residentCompiler }),
        });
        const compiler = (await compilerResponse.json()) as {
          ok: boolean;
          message: string;
          settings: CompilerSettingsResponse;
        };
        if (!compilerResponse.ok || !compiler.ok) {
          throw new Error(compiler.message || `HTTP ${compilerResponse.status}`);
        }
        setResidentCompiler(compiler.settings.resident);
      }
      setMessage(t("settings.saved"));
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <SidebarLayout title={t("settings.title")}>
      <section className="panel">
        <h2>{t("settings.title")}</h2>
        <p className="meta">{t("settings.subtitle")}</p>

        <div className="grid cols-2" style={{ marginTop: 12 }}>
          <label className="meta">
            {t("settings.maxRecords")}
            <input
              type="number"
              min={50}
              max={50000}
              value={maxRecords}
              onChange={(event) => setMaxRecords(Number(event.target.value) || 500)}
              style={{ marginTop: 6, width: "100%" }}
            />
          </label>
          <label className="meta">
            {t("settings.overflowPolicy")}
            <select
              value={overflowPolicy}
              onChange={(event) => setOverflowPolicy(event.target.value as EventOverflowPolicy)}
              style={{ marginTop: 6, width: "100%" }}
            >
              <option value="drop_oldest">{t("settings.dropOldest")}</option>
              <option value="drop_new">{t("settings.dropNew")}</option>
            </select>
          </label>
        </div>

        <p className="meta" style={{ marginTop: 10 }}>
          {overflowPolicy === "drop_oldest" ? t("settings.dropOldestHint") : t("settings.dropNewHint")}
        </p>
        {compilerSettingsAvailable && (
          <label className="row" style={{ marginTop: 16, alignItems: "flex-start" }}>
            <input
              type="checkbox"
              checked={residentCompiler}
              onChange={(event) => setResidentCompiler(event.target.checked)}
              style={{ marginTop: 3 }}
            />
            <span>
              <strong>{t("settings.residentCompiler")}</strong>
              <span className="meta" style={{ display: "block", marginTop: 4 }}>
                {residentCompiler
                  ? t("settings.residentCompilerEnabledHint")
                  : t("settings.residentCompilerDisabledHint")}
              </span>
            </span>
          </label>
        )}

        <DocumentationLinks />
        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={save} disabled={saving || loading}>
            {saving ? t("settings.saving") : t("settings.save")}
          </button>
          <button
            type="button"
            onClick={() => {
              setMessage("");
              setError("");
              void performance.refresh();
            }}
            disabled={loading || performance.refreshing}
          >
            {performance.refreshing ? t("settings.metricsRefreshing") : t("settings.refreshMetrics")}
          </button>
          {loading && <span className="meta">{t("settings.loading")}</span>}
        </div>

        {message && <p className="meta" style={{ color: "#9cd67a" }}>{message}</p>}
        {performance.message && (
          <p className="meta" style={{ color: "#9cd67a" }}>{performance.message}</p>
        )}
        {error && <p className="error">{error}</p>}
        {performance.error && <p className="error">{performance.error}</p>}

        <PerformanceMetricsPanel metrics={performance.metrics} summary={performance.hotspotSummary} />
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
