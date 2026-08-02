import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import Link from "next/link";

import { DOCS_LINK_STYLE, DOCS_QUICK_LINKS } from "../src/config/settings";
import { getEngineUrl } from "../src/config/runtime";
import SidebarLayout from "../src/components/SidebarLayout";
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

type CompilerOperationMetricsResponse = {
  total_requests: number;
  cache_hits: number;
  cache_misses: number;
  errors: number;
  rejected: number;
  in_flight: number;
  in_flight_peak: number;
  avg_duration_ms: number;
};

type PerformanceMetricsResponse = {
  check: CompilerOperationMetricsResponse;
  completion: CompilerOperationMetricsResponse;
};

type HotspotSeverity = "safe" | "warning" | "critical";

export default function SettingsPage() {
  const { t } = useI18n();
  const [maxRecords, setMaxRecords] = useState(
    () => loadPageState<number>("settings_event_max_records_v1") ?? 500,
  );
  const [overflowPolicy, setOverflowPolicy] = useState<EventOverflowPolicy>(
    () => loadPageState<EventOverflowPolicy>("settings_event_overflow_policy_v1") ?? "drop_oldest",
  );
  const [residentCompiler, setResidentCompiler] = useState(false);
  const [compilerSettingsAvailable, setCompilerSettingsAvailable] = useState(false);
  const [performanceMetrics, setPerformanceMetrics] = useState<PerformanceMetricsResponse | null>(
    null,
  );
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [refreshingPerformance, setRefreshingPerformance] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const isMounted = useRef(true);
  const performancePollTimer = useRef<number | null>(null);

  const engineUrl = useMemo(getEngineUrl, []);

  const formatPercent = (value: number, digits = 1) => `${(value * 100).toFixed(digits)}%`;

  const severityColor = (severity: HotspotSeverity) => {
    if (severity === "critical") return "#f05f5f";
    if (severity === "warning") return "#f3b33d";
    return "#8ad66a";
  };

  const severityLabel = (severity: HotspotSeverity) => {
    if (severity === "critical") return t("settings.hotspotCritical");
    if (severity === "warning") return t("settings.hotspotWarning");
    return t("settings.hotspotSafe");
  };

  const evaluateOperationHotspot = useCallback(
    (name: string, metrics: CompilerOperationMetricsResponse) => {
      const cacheTotal = metrics.cache_hits + metrics.cache_misses;
      const cacheHitRate = cacheTotal > 0 ? metrics.cache_hits / cacheTotal : 0;
      const rejectRate = metrics.total_requests > 0 ? metrics.rejected / metrics.total_requests : 0;
      const notes: string[] = [];

      let severity: HotspotSeverity = "safe";
      if (cacheHitRate < 0.2 || rejectRate > 0.2 || metrics.avg_duration_ms > 300) {
        severity = "critical";
      } else if (
        cacheHitRate < 0.4 ||
        rejectRate > 0.05 ||
        metrics.avg_duration_ms > 150
      ) {
        severity = "warning";
      }

      if (cacheTotal > 0 && cacheHitRate < 0.4) {
        notes.push(
          t("settings.hotspotCacheLow", {
            value: formatPercent(cacheHitRate),
            threshold: "40%",
          }),
        );
      }

      if (metrics.total_requests > 0 && rejectRate > 0.05) {
        notes.push(
          t("settings.hotspotRejectHigh", {
            value: formatPercent(rejectRate),
            threshold: "5%",
          }),
        );
      }

      if (metrics.avg_duration_ms > 150) {
        notes.push(
          t("settings.hotspotLatencyHigh", {
            value: metrics.avg_duration_ms.toFixed(1),
            threshold: "150",
          }),
        );
      }

      if (notes.length === 0) {
        notes.push(t("settings.hotspotNoAlert"));
      }

      return {
        name,
        severity,
        cacheHitRate,
        rejectRate,
        avgDurationMs: metrics.avg_duration_ms,
        inFlightPeak: metrics.in_flight_peak,
        notes,
      };
    },
    [formatPercent, t],
  );

  const refreshPerformance = useCallback(
    async ({ silent = false }: { silent?: boolean } = {}) => {
      if (!silent) {
        setRefreshingPerformance(true);
        setError(null);
      }
      try {
        const response = await fetch(`${engineUrl}/settings/performance`, {
          credentials: "include",
        });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        const json = (await response.json()) as PerformanceMetricsResponse;
        if (isMounted.current) {
          setPerformanceMetrics(json);
        }
        if (!silent) {
          setMessage(t("settings.metricsUpdated"));
        }
      } catch (err) {
        if (!silent && isMounted.current) {
          setError((err as Error).message);
        }
      } finally {
        if (!silent && isMounted.current) {
          setRefreshingPerformance(false);
        }
      }
    },
    [engineUrl, t],
  );

  useEffect(() => {
    isMounted.current = true;

    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const [eventsResponse, compilerResponse] = await Promise.all([
          fetch(`${engineUrl}/settings/events`, { credentials: "include" }),
          fetch(`${engineUrl}/settings/compiler`, { credentials: "include" }),
        ]);
        if (!eventsResponse.ok) throw new Error(`HTTP ${eventsResponse.status}`);
        const json = (await eventsResponse.json()) as EventSettingsResponse;
        if (!isMounted.current) return;
        setMaxRecords(json.max_records);
        setOverflowPolicy(json.overflow_policy);
        if (compilerResponse.ok) {
          const compiler = (await compilerResponse.json()) as CompilerSettingsResponse;
          setResidentCompiler(compiler.resident);
          setCompilerSettingsAvailable(true);
        }
        await refreshPerformance({ silent: true });
      } catch (err) {
        if (isMounted.current) {
          setError((err as Error).message);
        }
      } finally {
        if (isMounted.current) {
          setLoading(false);
        }
      }
    };

    load();

    return () => {
      isMounted.current = false;
      if (performancePollTimer.current !== null) {
        clearInterval(performancePollTimer.current);
        performancePollTimer.current = null;
      }
    };
  }, [engineUrl, refreshPerformance]);

  useEffect(() => {
    savePageState("settings_event_max_records_v1", maxRecords);
    savePageState("settings_event_overflow_policy_v1", overflowPolicy);
  }, [maxRecords, overflowPolicy]);

  useEffect(() => {
    performancePollTimer.current = window.setInterval(() => {
      void refreshPerformance({ silent: true });
    }, 10_000);

    return () => {
      if (performancePollTimer.current !== null) {
        clearInterval(performancePollTimer.current);
        performancePollTimer.current = null;
      }
    };
  }, [refreshPerformance]);

  const hotspotSummary = useMemo(() => {
    if (!performanceMetrics) return null;

    const checkHotspot = evaluateOperationHotspot(t("settings.metricsCheck"), performanceMetrics.check);
    const completionHotspot = evaluateOperationHotspot(
      t("settings.metricsCompletion"),
      performanceMetrics.completion,
    );

    const operationHotspots = [checkHotspot, completionHotspot];

    const totalRequests =
      performanceMetrics.check.total_requests + performanceMetrics.completion.total_requests;
    const totalCacheHits =
      performanceMetrics.check.cache_hits + performanceMetrics.completion.cache_hits;
    const totalCacheMisses =
      performanceMetrics.check.cache_misses + performanceMetrics.completion.cache_misses;
    const totalRejected =
      performanceMetrics.check.rejected + performanceMetrics.completion.rejected;

    const allRequests = totalCacheHits + totalCacheMisses;
    const cacheHitRate = allRequests > 0 ? totalCacheHits / allRequests : 0;
    const rejectRate = totalRequests > 0 ? totalRejected / totalRequests : 0;
    const avgDurationMs =
      (performanceMetrics.check.avg_duration_ms * performanceMetrics.check.total_requests +
        performanceMetrics.completion.avg_duration_ms * performanceMetrics.completion.total_requests) /
      Math.max(1, totalRequests);

    let overallSeverity: HotspotSeverity = "safe";
    const hasCriticalOperation = operationHotspots.some((entry) => entry.severity === "critical");
    const hasWarningOperation = operationHotspots.some((entry) => entry.severity === "warning");

    if (cacheHitRate < 0.2 || rejectRate > 0.2 || avgDurationMs > 300 || hasCriticalOperation) {
      overallSeverity = "critical";
    } else if (
      cacheHitRate < 0.4 ||
      rejectRate > 0.05 ||
      avgDurationMs > 150 ||
      hasWarningOperation
    ) {
      overallSeverity = "warning";
    }

    return {
      overall: {
        severity: overallSeverity,
        cacheHitRate,
        rejectRate,
        avgDurationMs,
        inFlightPeak: Math.max(
          performanceMetrics.check.in_flight_peak,
          performanceMetrics.completion.in_flight_peak,
        ),
        totalRequests,
      },
      operationHotspots,
    };
  }, [performanceMetrics, evaluateOperationHotspot, t]);

  const save = async () => {
    setSaving(true);
    setError(null);
    setMessage(null);
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
      const json = (await response.json()) as {
        ok: boolean;
        message: string;
        settings?: EventSettingsResponse;
      };
      if (!response.ok || !json.ok) {
        throw new Error(json.message || `HTTP ${response.status}`);
      }
      if (json.settings) {
        setMaxRecords(json.settings.max_records);
        setOverflowPolicy(json.settings.overflow_policy);
      }

      if (compilerSettingsAvailable) {
        const compilerResponse = await fetch(`${engineUrl}/settings/compiler`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
          body: JSON.stringify({ resident: residentCompiler }),
        });
        const compilerJson = (await compilerResponse.json()) as {
          ok: boolean;
          message: string;
          settings: CompilerSettingsResponse;
        };
        if (!compilerResponse.ok || !compilerJson.ok) {
          throw new Error(compilerJson.message || `HTTP ${compilerResponse.status}`);
        }
        setResidentCompiler(compilerJson.settings.resident);
      }
      setMessage(t("settings.saved"));
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setSaving(false);
    }
  };

  const formatOpsRow = (key: keyof CompilerOperationMetricsResponse, value: number) => {
    if (Number.isFinite(value) && key === "avg_duration_ms") {
      return `${value.toFixed(2)} ms`;
    }
    return String(value);
  };

  const renderOperationMetrics = (title: string, metrics: CompilerOperationMetricsResponse) => {
    return (
      <div className="panel" style={{ marginTop: 10 }}>
        <strong>{title}</strong>
        <div className="grid cols-2" style={{ marginTop: 8 }}>
          <p className="meta">
            {t("settings.metricsTotal")}：{formatOpsRow("total_requests", metrics.total_requests)}
          </p>
          <p className="meta">
            {t("settings.metricsCacheHits")}：{formatOpsRow("cache_hits", metrics.cache_hits)}
          </p>
          <p className="meta">
            {t("settings.metricsCacheMisses")}：{formatOpsRow("cache_misses", metrics.cache_misses)}
          </p>
          <p className="meta">
            {t("settings.metricsErrors")}：{formatOpsRow("errors", metrics.errors)}
          </p>
          <p className="meta">
            {t("settings.metricsRejected")}：{formatOpsRow("rejected", metrics.rejected)}
          </p>
          <p className="meta">
            {t("settings.metricsInFlight")}：{formatOpsRow("in_flight", metrics.in_flight)}
          </p>
          <p className="meta">
            {t("settings.metricsInFlightPeak")}：{formatOpsRow("in_flight_peak", metrics.in_flight_peak)}
          </p>
          <p className="meta">
            {t("settings.metricsAvgDuration")}：{formatOpsRow("avg_duration_ms", metrics.avg_duration_ms)}
          </p>
        </div>
      </div>
    );
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

        <section className="panel" style={{ marginTop: 12 }}>
          <strong>{t("settings.docsPanelTitle")}</strong>
          <p className="meta" style={{ marginTop: 4 }}>
            {t("settings.docsPanelDescription")}
          </p>
          <div className="row" style={{ marginTop: 10 }}>
            <Link href="/learn" style={DOCS_LINK_STYLE}>
              {t("layout.nav.learn")}
            </Link>
            <Link href="/learn/troubleshooting" style={DOCS_LINK_STYLE}>
              {t("settings.docsTroubleshoot")}
            </Link>
          </div>
          <p className="meta" style={{ marginTop: 14 }}>
            {t("settings.docsQuickTitle")}
          </p>
          <p className="meta" style={{ marginTop: 4 }}>
            {t("settings.docsQuickHint")}
          </p>
          <div className="grid cols-2" style={{ marginTop: 8 }}>
            {DOCS_QUICK_LINKS.map((item) => (
              <Link href={item.href} key={item.href} style={DOCS_LINK_STYLE}>
                {t(item.titleKey)}
              </Link>
            ))}
          </div>
        </section>

        <div className="row" style={{ marginTop: 12 }}>
          <button type="button" onClick={save} disabled={saving || loading}>
            {saving ? t("settings.saving") : t("settings.save")}
          </button>
          <button
            type="button"
            onClick={() => void refreshPerformance()}
            disabled={loading || refreshingPerformance}
          >
            {refreshingPerformance ? t("settings.metricsRefreshing") : t("settings.refreshMetrics")}
          </button>
          {loading && <span className="meta">{t("settings.loading")}</span>}
        </div>

        {message && <p className="meta" style={{ color: "#9cd67a" }}>{message}</p>}
        {error && <p className="error">{error}</p>}

        <section className="panel" style={{ marginTop: 14 }}>
          <h3>{t("settings.performanceTitle")}</h3>
          <p className="meta">{t("settings.performanceSubtitle")}</p>

          {!performanceMetrics && (
            <p className="meta" style={{ marginTop: 10 }}>
              {t("settings.metricsUnavailable")}
            </p>
          )}

          {performanceMetrics && hotspotSummary && (
            <>
              <div
                className="panel"
                style={{
                  marginTop: 10,
                  borderLeft: `4px solid ${severityColor(hotspotSummary.overall.severity)}`,
                }}
              >
                <strong>{t("settings.hotspotSummary")}</strong>
                <p
                  className="meta"
                  style={{
                    color: severityColor(hotspotSummary.overall.severity),
                    marginTop: 6,
                  }}
                >
                  {severityLabel(hotspotSummary.overall.severity)}
                </p>
                <div className="grid cols-2" style={{ marginTop: 8 }}>
                  <p className="meta">
                    {t("settings.hotspotRequests")}: {hotspotSummary.overall.totalRequests}
                  </p>
                  <p className="meta">
                    {t("settings.hotspotCacheRate")}：{formatPercent(hotspotSummary.overall.cacheHitRate)}
                  </p>
                  <p className="meta">
                    {t("settings.hotspotRejectRate")}：{formatPercent(hotspotSummary.overall.rejectRate)}
                  </p>
                  <p className="meta">
                    {t("settings.hotspotAvgLatency")}：{hotspotSummary.overall.avgDurationMs.toFixed(2)} ms
                  </p>
                  <p className="meta">
                    {t("settings.metricsInFlightPeak")}：{hotspotSummary.overall.inFlightPeak}
                  </p>
                </div>
              </div>

              <div className="grid cols-2" style={{ marginTop: 10 }}>
                {hotspotSummary.operationHotspots.map((entry) => (
                  <div
                    key={entry.name}
                    style={{
                      border: `1px solid ${severityColor(entry.severity)}`,
                      padding: 10,
                    }}
                  >
                    <strong>{entry.name}</strong>
                    <p
                      className="meta"
                      style={{
                        color: severityColor(entry.severity),
                        marginTop: 4,
                      }}
                    >
                      {severityLabel(entry.severity)}
                    </p>
                    <p className="meta">
                      {t("settings.hotspotCacheRate")}：{formatPercent(entry.cacheHitRate)}
                    </p>
                    <p className="meta">
                      {t("settings.hotspotRejectRate")}：{formatPercent(entry.rejectRate)}
                    </p>
                    <p className="meta">
                      {t("settings.hotspotAvgLatency")}：{entry.avgDurationMs.toFixed(2)} ms
                    </p>
                    <p className="meta">
                      {t("settings.metricsInFlightPeak")}：{entry.inFlightPeak}
                    </p>
                    {entry.notes.map((note) => (
                      <p className="meta" key={note}>
                        - {note}
                      </p>
                    ))}
                  </div>
                ))}
              </div>

              <h4 style={{ marginTop: 12 }}>{t("settings.hotspotDetailTitle")}</h4>
              {renderOperationMetrics(t("settings.metricsCheck"), performanceMetrics.check)}
              {renderOperationMetrics(t("settings.metricsCompletion"), performanceMetrics.completion)}
            </>
          )}
        </section>
      </section>
    </SidebarLayout>
  );
}
