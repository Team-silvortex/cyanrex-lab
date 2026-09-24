import type { CompilerOperationMetrics, PerformanceMetrics } from "./models";

const record = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const invalid = () => new Error("Invalid performance metrics");

function numeric(value: unknown, integral = true): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > Number.MAX_SAFE_INTEGER
    || (integral && !Number.isSafeInteger(value))) throw invalid();
  return value;
}

function operation(value: unknown): CompilerOperationMetrics {
  if (!record(value)) throw invalid();
  return {
    total_requests: numeric(value.total_requests),
    cache_hits: numeric(value.cache_hits),
    cache_misses: numeric(value.cache_misses),
    errors: numeric(value.errors),
    rejected: numeric(value.rejected),
    in_flight: numeric(value.in_flight),
    in_flight_peak: numeric(value.in_flight_peak),
    avg_duration_ms: numeric(value.avg_duration_ms, false),
  };
}

export function parsePerformanceMetrics(value: unknown): PerformanceMetrics {
  if (!record(value)) throw invalid();
  const check = operation(value.check), completion = operation(value.completion);
  // Protect the displayed aggregate arithmetic, without imposing cross-counter equality:
  // Engine counters are sampled independently and may change while a snapshot is read.
  numeric(check.total_requests + completion.total_requests);
  numeric(check.rejected + completion.rejected);
  numeric(check.cache_hits + check.cache_misses + completion.cache_hits + completion.cache_misses);
  return { check, completion };
}
