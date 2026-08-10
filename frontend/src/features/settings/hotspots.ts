import type {
  CompilerOperationMetrics,
  HotspotSeverity,
  HotspotSummary,
  OperationHotspot,
  PerformanceMetrics,
} from "./models";

type Translate = (key: string, vars?: Record<string, string | number>) => string;

const formatPercent = (value: number) => `${(value * 100).toFixed(1)}%`;

export function buildHotspotSummary(
  metrics: PerformanceMetrics,
  t: Translate,
): HotspotSummary {
  const operationHotspots = [
    evaluateOperation(t("settings.metricsCheck"), metrics.check, t),
    evaluateOperation(t("settings.metricsCompletion"), metrics.completion, t),
  ];
  const totalRequests = metrics.check.total_requests + metrics.completion.total_requests;
  const totalCacheHits = metrics.check.cache_hits + metrics.completion.cache_hits;
  const totalCacheMisses = metrics.check.cache_misses + metrics.completion.cache_misses;
  const totalRejected = metrics.check.rejected + metrics.completion.rejected;
  const cacheRequests = totalCacheHits + totalCacheMisses;
  const cacheHitRate = cacheRequests > 0 ? totalCacheHits / cacheRequests : 0;
  const rejectRate = totalRequests > 0 ? totalRejected / totalRequests : 0;
  const avgDurationMs = (
    metrics.check.avg_duration_ms * metrics.check.total_requests
    + metrics.completion.avg_duration_ms * metrics.completion.total_requests
  ) / Math.max(1, totalRequests);
  const hasCritical = operationHotspots.some((entry) => entry.severity === "critical");
  const hasWarning = operationHotspots.some((entry) => entry.severity === "warning");
  let severity: HotspotSeverity = "safe";

  if (
    (cacheRequests > 0 && cacheHitRate < 0.2)
    || rejectRate > 0.2
    || avgDurationMs > 300
    || hasCritical
  ) {
    severity = "critical";
  } else if (
    (cacheRequests > 0 && cacheHitRate < 0.4)
    || rejectRate > 0.05
    || avgDurationMs > 150
    || hasWarning
  ) {
    severity = "warning";
  }

  return {
    overall: {
      severity,
      cacheHitRate,
      rejectRate,
      avgDurationMs,
      inFlightPeak: Math.max(metrics.check.in_flight_peak, metrics.completion.in_flight_peak),
      totalRequests,
    },
    operationHotspots,
  };
}

function evaluateOperation(
  name: string,
  operation: CompilerOperationMetrics,
  t: Translate,
): OperationHotspot {
  const cacheTotal = operation.cache_hits + operation.cache_misses;
  const cacheHitRate = cacheTotal > 0 ? operation.cache_hits / cacheTotal : 0;
  const rejectRate = operation.total_requests > 0
    ? operation.rejected / operation.total_requests
    : 0;
  const notes: string[] = [];
  let severity: HotspotSeverity = "safe";

  if (
    (cacheTotal > 0 && cacheHitRate < 0.2)
    || rejectRate > 0.2
    || operation.avg_duration_ms > 300
  ) {
    severity = "critical";
  } else if (
    (cacheTotal > 0 && cacheHitRate < 0.4)
    || rejectRate > 0.05
    || operation.avg_duration_ms > 150
  ) {
    severity = "warning";
  }
  if (cacheTotal > 0 && cacheHitRate < 0.4) {
    notes.push(t("settings.hotspotCacheLow", {
      value: formatPercent(cacheHitRate),
      threshold: "40%",
    }));
  }
  if (operation.total_requests > 0 && rejectRate > 0.05) {
    notes.push(t("settings.hotspotRejectHigh", {
      value: formatPercent(rejectRate),
      threshold: "5%",
    }));
  }
  if (operation.avg_duration_ms > 150) {
    notes.push(t("settings.hotspotLatencyHigh", {
      value: operation.avg_duration_ms.toFixed(1),
      threshold: "150",
    }));
  }
  if (notes.length === 0) notes.push(t("settings.hotspotNoAlert"));

  return {
    name,
    severity,
    cacheHitRate,
    rejectRate,
    avgDurationMs: operation.avg_duration_ms,
    inFlightPeak: operation.in_flight_peak,
    notes,
  };
}
