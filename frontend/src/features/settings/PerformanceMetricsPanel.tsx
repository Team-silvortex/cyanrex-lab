import { useI18n } from "../../i18n/context";
import type {
  CompilerOperationMetrics,
  HotspotSeverity,
  HotspotSummary,
  PerformanceMetrics,
} from "./models";

type Props = {
  metrics: PerformanceMetrics | null;
  summary: HotspotSummary | null;
  refreshing: boolean;
  stale: boolean;
  error: string;
  message: string;
};

const formatPercent = (value: number) => `${(value * 100).toFixed(1)}%`;

const severityColor = (severity: HotspotSeverity) => {
  if (severity === "critical") return "#f05f5f";
  if (severity === "warning") return "#f3b33d";
  return "#8ad66a";
};

export default function PerformanceMetricsPanel({ metrics, summary, refreshing, stale, error, message }: Props) {
  const { t } = useI18n();
  const color = (severity: HotspotSeverity) => stale ? "#9a9a9a" : severityColor(severity);
  const severityLabel = (severity: HotspotSeverity) => {
    if (stale) return t("settings.metricsStaleLabel");
    if (severity === "critical") return t("settings.hotspotCritical");
    if (severity === "warning") return t("settings.hotspotWarning");
    return t("settings.hotspotSafe");
  };

  return (
    <section className="panel" style={{ marginTop: 14 }}>
      <h3>{t("settings.performanceTitle")}</h3>
      <p className="meta">{t("settings.performanceSubtitle")}</p>

      <div role="status">
        {error && <p className="error">{error}</p>}
        {stale && <p className="meta">{t("settings.metricsStale")}</p>}
        {message && <p className="meta" style={{ color: "#9cd67a" }}>{message}</p>}
        {refreshing && <p className="meta">{t("settings.metricsRefreshing")}</p>}
        {!metrics && !refreshing && <p className="meta" style={{ marginTop: 10 }}>{t("settings.metricsUnavailable")}</p>}
      </div>
      {metrics && summary && (
        <>
          <div
            className="panel"
            style={{
              marginTop: 10,
              borderLeft: `4px solid ${color(summary.overall.severity)}`,
            }}
          >
            <strong>{t("settings.hotspotSummary")}</strong>
            <p
              className="meta"
              style={{ color: color(summary.overall.severity), marginTop: 6 }}
            >
              {severityLabel(summary.overall.severity)}
            </p>
            <div className="grid cols-2" style={{ marginTop: 8 }}>
              <p className="meta">{t("settings.hotspotRequests")}: {summary.overall.totalRequests}</p>
              <p className="meta">
                {t("settings.hotspotCacheRate")}：{formatPercent(summary.overall.cacheHitRate)}
              </p>
              <p className="meta">
                {t("settings.hotspotRejectRate")}：{formatPercent(summary.overall.rejectRate)}
              </p>
              <p className="meta">
                {t("settings.hotspotAvgLatency")}：{summary.overall.avgDurationMs.toFixed(2)} ms
              </p>
              <p className="meta">
                {t("settings.metricsInFlightPeak")}：{summary.overall.inFlightPeak}
              </p>
            </div>
          </div>

          <div className="grid cols-2" style={{ marginTop: 10 }}>
            {summary.operationHotspots.map((entry) => (
              <div
                key={entry.name}
                style={{ border: `1px solid ${color(entry.severity)}`, padding: 10 }}
              >
                <strong>{entry.name}</strong>
                <p
                  className="meta"
                  style={{ color: color(entry.severity), marginTop: 4 }}
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
                <p className="meta">{t("settings.metricsInFlightPeak")}：{entry.inFlightPeak}</p>
                {entry.notes.map((note) => <p className="meta" key={note}>- {note}</p>)}
              </div>
            ))}
          </div>

          <h4 style={{ marginTop: 12 }}>{t("settings.hotspotDetailTitle")}</h4>
          <OperationMetrics title={t("settings.metricsCheck")} metrics={metrics.check} />
          <OperationMetrics title={t("settings.metricsCompletion")} metrics={metrics.completion} />
        </>
      )}
    </section>
  );
}

function OperationMetrics({
  title,
  metrics,
}: {
  title: string;
  metrics: CompilerOperationMetrics;
}) {
  const { t } = useI18n();
  const rows: Array<[string, keyof CompilerOperationMetrics]> = [
    ["metricsTotal", "total_requests"],
    ["metricsCacheHits", "cache_hits"],
    ["metricsCacheMisses", "cache_misses"],
    ["metricsErrors", "errors"],
    ["metricsRejected", "rejected"],
    ["metricsInFlight", "in_flight"],
    ["metricsInFlightPeak", "in_flight_peak"],
    ["metricsAvgDuration", "avg_duration_ms"],
  ];
  return (
    <div className="panel" style={{ marginTop: 10 }}>
      <strong>{title}</strong>
      <div className="grid cols-2" style={{ marginTop: 8 }}>
        {rows.map(([label, key]) => (
          <p className="meta" key={key}>
            {t(`settings.${label}`)}：{formatMetric(key, metrics[key])}
          </p>
        ))}
      </div>
    </div>
  );
}

function formatMetric(key: keyof CompilerOperationMetrics, value: number) {
  return key === "avg_duration_ms" && Number.isFinite(value)
    ? `${value.toFixed(2)} ms`
    : String(value);
}
