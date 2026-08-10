export type CompilerOperationMetrics = {
  total_requests: number;
  cache_hits: number;
  cache_misses: number;
  errors: number;
  rejected: number;
  in_flight: number;
  in_flight_peak: number;
  avg_duration_ms: number;
};

export type PerformanceMetrics = {
  check: CompilerOperationMetrics;
  completion: CompilerOperationMetrics;
};

export type HotspotSeverity = "safe" | "warning" | "critical";

export type OperationHotspot = {
  name: string;
  severity: HotspotSeverity;
  cacheHitRate: number;
  rejectRate: number;
  avgDurationMs: number;
  inFlightPeak: number;
  notes: string[];
};

export type HotspotSummary = {
  overall: {
    severity: HotspotSeverity;
    cacheHitRate: number;
    rejectRate: number;
    avgDurationMs: number;
    inFlightPeak: number;
    totalRequests: number;
  };
  operationHotspots: OperationHotspot[];
};
