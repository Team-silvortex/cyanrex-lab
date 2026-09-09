type Filters = {
  categoryFilter: string; severityFilter: string; rangePreset: string; startTime: string; endTime: string;
};

// The server deletes all matches, not the bounded 200-row browser snapshot. Freeze the time scope.
export function buildEventDeleteParams(input: Filters, now = new Date()): URLSearchParams {
  if (!["all", "kernel", "platform"].includes(input.categoryFilter) ||
      !["all", "success", "warning", "error"].includes(input.severityFilter) ||
      !["all", "10m", "1h", "24h", "custom"].includes(input.rangePreset) || !Number.isFinite(now.getTime())) {
    throw new Error("Invalid event deletion scope");
  }
  const params = new URLSearchParams();
  if (input.categoryFilter !== "all") params.set("category", input.categoryFilter);
  if (input.severityFilter !== "all") params.set("severity", input.severityFilter);
  let start: Date | undefined;
  let end = now;
  const minutes: Record<string, number> = { "10m": 10, "1h": 60, "24h": 1440 };
  if (minutes[input.rangePreset]) start = new Date(now.getTime() - minutes[input.rangePreset] * 60000);
  if (input.rangePreset === "custom") {
    if (input.startTime) start = new Date(input.startTime);
    if (input.endTime) {
      const requestedEnd = new Date(input.endTime);
      if (!Number.isFinite(requestedEnd.getTime())) throw new Error("Invalid event deletion scope");
      end = new Date(Math.min(now.getTime(), requestedEnd.getTime()));
    }
  }
  if (start && (!Number.isFinite(start.getTime()) || start > end)) throw new Error("Invalid event deletion scope");
  if (start) params.set("start", start.toISOString());
  params.set("end", end.toISOString());
  return params;
}
