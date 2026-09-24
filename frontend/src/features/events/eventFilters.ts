import type { EngineEvent } from "./eventStream";

export type EventFilters = {
  categoryFilter: "all" | "kernel" | "platform";
  severityFilter: "all" | "success" | "warning" | "error";
  rangePreset: "all" | "10m" | "1h" | "24h" | "custom";
  startTime: string;
  endTime: string;
};
export type ExportFormat = "json" | "csv";
export const presetMinutes = (preset: string): number | undefined =>
  ({ "10m": 10, "1h": 60, "24h": 1440 } as Record<string, number>)[preset];

export function buildEventFilterParams(filters: EventFilters, limit?: number, format?: ExportFormat): URLSearchParams {
  if (!["all", "kernel", "platform"].includes(filters.categoryFilter)
    || !["all", "success", "warning", "error"].includes(filters.severityFilter)
    || !["all", "10m", "1h", "24h", "custom"].includes(filters.rangePreset)
    || typeof filters.startTime !== "string" || typeof filters.endTime !== "string"
    || (format !== undefined && !["json", "csv"].includes(format))) throw new Error("Invalid event filters");
  const params = new URLSearchParams();
  if (filters.categoryFilter !== "all") params.set("category", filters.categoryFilter);
  if (filters.severityFilter !== "all") params.set("severity", filters.severityFilter);
  if (limit !== undefined) {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 500) throw new Error("Invalid event limit");
    params.set("limit", String(limit));
  }
  if (format) params.set("format", format);
  const minutes = presetMinutes(filters.rangePreset);
  if (minutes) params.set("since_minutes", String(minutes));
  if (filters.rangePreset === "custom") {
    const start = filters.startTime ? Date.parse(filters.startTime) : null;
    const end = filters.endTime ? Date.parse(filters.endTime) : null;
    if ((start !== null && !Number.isFinite(start)) || (end !== null && !Number.isFinite(end))
      || (start !== null && end !== null && start > end)) throw new Error("Invalid event time range");
    if (start !== null) params.set("start", new Date(start).toISOString());
    if (end !== null) params.set("end", new Date(end).toISOString());
  }
  return params;
}

export function matchesEventFilters(event: EngineEvent, filters: EventFilters, now = Date.now()): boolean {
  if (filters.categoryFilter !== "all" && event.category !== filters.categoryFilter) return false;
  if (filters.severityFilter !== "all" && event.severity !== filters.severityFilter) return false;
  const time = Date.parse(event.timestamp);
  if (!Number.isFinite(time)) return false;
  const minutes = presetMinutes(filters.rangePreset);
  if (minutes) return time >= now - minutes * 60_000;
  if (filters.rangePreset === "custom") {
    if (filters.startTime && !(time >= Date.parse(filters.startTime))) return false;
    if (filters.endTime && !(time <= Date.parse(filters.endTime))) return false;
  }
  return true;
}
