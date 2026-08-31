import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
import { getEngineUrl, toWebSocketUrl } from "../src/config/runtime";
import { useI18n } from "../src/i18n/context";
import { loadPageState, savePageState } from "../src/utils/pageState";

type EngineEvent = {
  username: string;
  timestamp: string;
  source: string;
  event_type: string;
  category: "kernel" | "platform";
  severity: "success" | "warning" | "error";
  color: "green" | "yellow" | "red";
  payload: Record<string, unknown>;
};

type SafetyTone = "ok" | "warn";

const EVENT_LIST_LIMIT = 200;
const MARK_READ_DEBOUNCE_MS = 1200;

type EventFilterState = {
  categoryFilter: "all" | "kernel" | "platform";
  severityFilter: "all" | "success" | "warning" | "error";
  rangePreset: "all" | "10m" | "1h" | "24h" | "custom";
  startTime: string;
  endTime: string;
};

export default function EventsPage() {
  const { t } = useI18n();
  const [events, setEvents] = useState<EngineEvent[]>([]);
  const [connection, setConnection] = useState<"connecting" | "open" | "closed">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [categoryFilter, setCategoryFilter] = useState<"all" | "kernel" | "platform">(
    () => loadPageState<"all" | "kernel" | "platform">("events_category_v1") ?? "all",
  );
  const [severityFilter, setSeverityFilter] = useState<"all" | "success" | "warning" | "error">(
    () => loadPageState<"all" | "success" | "warning" | "error">("events_severity_v1") ?? "all",
  );
  const [rangePreset, setRangePreset] = useState<"all" | "10m" | "1h" | "24h" | "custom">(
    () => loadPageState<"all" | "10m" | "1h" | "24h" | "custom">("events_range_v1") ?? "all",
  );
  const [startTime, setStartTime] = useState(() => loadPageState<string>("events_start_v1") ?? "");
  const [endTime, setEndTime] = useState(() => loadPageState<string>("events_end_v1") ?? "");
  const [exportFormat, setExportFormat] = useState<"json" | "csv">(
    () => loadPageState<"json" | "csv">("events_export_v1") ?? "json",
  );
  const filterRef = useRef<EventFilterState>({
    categoryFilter: "all",
    severityFilter: "all",
    rangePreset: "all",
    startTime: "",
    endTime: "",
  });

  const engineUrl = useMemo(getEngineUrl, []);
  const markReadTimer = useRef<number | null>(null);

  useEffect(() => {
    filterRef.current = {
      categoryFilter,
      severityFilter,
      rangePreset,
      startTime,
      endTime,
    };
  }, [categoryFilter, endTime, rangePreset, severityFilter, startTime]);

  const scheduleMarkRead = useCallback(() => {
    if (markReadTimer.current !== null) {
      return;
    }

    markReadTimer.current = window.setTimeout(() => {
      markReadTimer.current = null;
      void fetch(`${engineUrl}/events/mark-read`, {
        method: "POST",
        credentials: "include",
      }).catch(() => undefined);
    }, MARK_READ_DEBOUNCE_MS);
  }, [engineUrl]);

  const loadSnapshot = useCallback(async () => {
    const params = buildFilterParams({
      categoryFilter,
      severityFilter,
      rangePreset,
      startTime,
      endTime,
      limit: EVENT_LIST_LIMIT,
    });

    try {
      const response = await fetch(`${engineUrl}/events?${params.toString()}`, {
        credentials: "include",
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const snapshot = (await response.json()) as EngineEvent[];
      setEvents(snapshot);
      scheduleMarkRead();
      setError(null);
    } catch (err) {
      setError((err as Error).message);
    }
  }, [categoryFilter, engineUrl, endTime, rangePreset, scheduleMarkRead, severityFilter, startTime]);

  useEffect(() => {
    let ws: WebSocket | null = null;
    let alive = true;

    const openWs = () => {
      const wsUrl = toWebSocketUrl(engineUrl, "/ws/events");
      ws = new WebSocket(wsUrl);
      setConnection("connecting");

      ws.onopen = () => {
        if (!alive) return;
        setConnection("open");
        setError(null);
      };

      ws.onmessage = (message) => {
        try {
          const event = JSON.parse(message.data as string) as EngineEvent;
          if (!alive) return;
          if (!matchesCurrentFilters(event, filterRef.current)) {
            return;
          }
          setEvents((prev) => {
            const next = prev.length >= EVENT_LIST_LIMIT ? prev.slice(1) : prev.slice();
            next.push(event);
            return next;
          });
          scheduleMarkRead();
        } catch {
          // ignore malformed event frame
        }
      };

      ws.onerror = () => {
        if (!alive) return;
        setError(t("events.websocketError"));
      };

      ws.onclose = () => {
        if (!alive) return;
        setConnection("closed");
      };
    };

    openWs();

    return () => {
      alive = false;
      ws?.close();
      if (markReadTimer.current !== null) {
        clearTimeout(markReadTimer.current);
        markReadTimer.current = null;
      }
    };
  }, [engineUrl, scheduleMarkRead]);

  useEffect(() => {
    void loadSnapshot();
  }, [loadSnapshot]);

  const activeFilterCount = useMemo(() => {
    let count = 0;
    if (categoryFilter !== "all") count += 1;
    if (severityFilter !== "all") count += 1;
    if (rangePreset !== "all") count += 1;
    if (rangePreset === "custom" && startTime.trim()) count += 1;
    if (rangePreset === "custom" && endTime.trim()) count += 1;
    return count;
  }, [categoryFilter, severityFilter, rangePreset, startTime, endTime]);

  useEffect(() => {
    savePageState("events_category_v1", categoryFilter);
    savePageState("events_severity_v1", severityFilter);
    savePageState("events_range_v1", rangePreset);
    savePageState("events_start_v1", startTime);
    savePageState("events_end_v1", endTime);
    savePageState("events_export_v1", exportFormat);
  }, [categoryFilter, severityFilter, rangePreset, startTime, endTime, exportFormat]);

  const exportEvents = async () => {
    const params = buildFilterParams({
      categoryFilter,
      severityFilter,
      rangePreset,
      startTime,
      endTime,
      exportFormat,
    });

    try {
      const response = await fetch(`${engineUrl}/events/export?${params.toString()}`, {
        credentials: "include",
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const blob = await response.blob();
      const disposition = response.headers.get("content-disposition") || "";
      const matched = disposition.match(/filename=\"([^\"]+)\"/);
      const filename = matched?.[1] || `cyanrex-events.${exportFormat}`;

      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = filename;
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(url);
    } catch (err) {
      setError((err as Error).message);
    }
  };

  const deleteFilteredEvents = async () => {
    const count = events.length;
    if (count === 0) {
      setError(t("events.noFilteredToDelete"));
      return;
    }

    const confirmed = window.confirm(
      t("events.deleteConfirm", { count }),
    );
    if (!confirmed) {
      return;
    }

    const params = buildFilterParams({
      categoryFilter,
      severityFilter,
      rangePreset,
      startTime,
      endTime,
    });

    try {
      const response = await fetch(`${engineUrl}/events/delete?${params.toString()}`, {
        method: "POST",
        credentials: "include",
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      const json = (await response.json()) as { ok: boolean; deleted: number };
      if (!json.ok) {
        throw new Error("delete filtered events failed");
      }
      setEvents([]);
      setError(null);
    } catch (err) {
      setError((err as Error).message);
    }
  };

  return (
    <SidebarLayout title={t("events.title")}>
      <section className="panel">
        <h2>{t("events.title")}</h2>
        <p className="meta">
          {t("events.status")}: {connection === "connecting" ? t("events.connectionConnecting") : connection === "open" ? t("events.connectionOpen") : t("events.connectionClosed")} | {t("events.total")}: {events.length} | {t("events.filtered")}: {events.length} | {t("events.activeFilters", { count: activeFilterCount })}
        </p>
        <div className="row" style={{ marginTop: 10 }}>
          <label className="meta">
            {t("events.category")}:
            {" "}
            <select value={categoryFilter} onChange={(event) => setCategoryFilter(event.target.value as typeof categoryFilter)}>
              <option value="all">{t("events.all")}</option>
              <option value="kernel">{t("events.kernel")}</option>
              <option value="platform">{t("events.platform")}</option>
            </select>
          </label>
          <label className="meta">
            {t("events.severity")}:
            {" "}
            <select value={severityFilter} onChange={(event) => setSeverityFilter(event.target.value as typeof severityFilter)}>
              <option value="all">{t("events.all")}</option>
              <option value="success">{t("events.success")}</option>
              <option value="warning">{t("events.warning")}</option>
              <option value="error">{t("events.error")}</option>
            </select>
          </label>
          <label className="meta">
            {t("events.range")}:
            {" "}
            <select value={rangePreset} onChange={(event) => setRangePreset(event.target.value as typeof rangePreset)}>
              <option value="all">{t("events.all")}</option>
              <option value="10m">{t("events.last10m")}</option>
              <option value="1h">{t("events.last1h")}</option>
              <option value="24h">{t("events.last24h")}</option>
              <option value="custom">{t("events.custom")}</option>
            </select>
          </label>
          {rangePreset === "custom" && (
            <>
              <label className="meta">
                {t("events.start")}:
                {" "}
                <input type="datetime-local" value={startTime} onChange={(event) => setStartTime(event.target.value)} />
              </label>
              <label className="meta">
                {t("events.end")}:
                {" "}
                <input type="datetime-local" value={endTime} onChange={(event) => setEndTime(event.target.value)} />
              </label>
            </>
          )}
            <label className="meta">
            {t("events.export")}:
            {" "}
            <select value={exportFormat} onChange={(event) => setExportFormat(event.target.value as typeof exportFormat)}>
              <option value="json">{t("events.exportJson")}</option>
              <option value="csv">{t("events.exportCsv")}</option>
            </select>
          </label>
          <button type="button" onClick={exportEvents}>{t("events.exportDownload")}</button>
          <button type="button" onClick={deleteFilteredEvents}>{t("events.deleteFiltered")}</button>
        </div>
        {error && <p className="error">{error}</p>}
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        {events.length === 0 && <p className="meta">{t("events.noEvents")}</p>}
        {events.map((_, reverseIdx) => {
          const idx = events.length - 1 - reverseIdx;
          const event = events[idx];
          if (!event) return null;

            const safetyBadges = extractSafetyBadges(event, t);
            return (
              <article key={`${event.timestamp}-${reverseIdx}`} className="panel" style={{ marginBottom: 10, background: "#0b1425" }}>
                <p style={{ margin: 0 }}>
                  <strong>{event.event_type}</strong>
                </p>
                <p className="meta" style={{ margin: "6px 0" }}>
                  {new Date(event.timestamp).toLocaleString()} | {t("events.sourceField")}: {event.source} | {t("events.categoryField")}: {event.category}
                </p>
                <p className={`event-tag ${event.color}`} style={{ margin: "0 0 8px 0" }}>
                  {event.severity.toUpperCase()}
                </p>
                {safetyBadges.length > 0 && (
                  <div className="row" style={{ marginBottom: 8 }}>
                    {safetyBadges.map((badge, badgeIdx) => (
                      <span key={`${event.timestamp}-${idx}-safety-${badgeIdx}`} className={`safety-tag ${badge.tone}`}>
                        {badge.text}
                      </span>
                    ))}
                  </div>
                )}
                <pre style={{ margin: 0 }}>{JSON.stringify(event.payload, null, 2)}</pre>
              </article>
            );
          })}
      </section>
    </SidebarLayout>
  );
}

function presetToMinutes(preset: "all" | "10m" | "1h" | "24h" | "custom"): number | null {
  if (preset === "10m") return 10;
  if (preset === "1h") return 60;
  if (preset === "24h") return 24 * 60;
  return null;
}

function matchesCurrentFilters(
  event: EngineEvent,
  filters: EventFilterState,
): boolean {
  if (filters.categoryFilter !== "all" && event.category !== filters.categoryFilter) {
    return false;
  }

  if (filters.severityFilter !== "all" && event.severity !== filters.severityFilter) {
    return false;
  }

  return timeFilterPass(event.timestamp, filters.rangePreset, filters.startTime, filters.endTime);
}

function timeFilterPass(
  timestamp: string,
  preset: "all" | "10m" | "1h" | "24h" | "custom",
  start: string,
  end: string,
): boolean {
  const eventTime = new Date(timestamp).getTime();
  if (Number.isNaN(eventTime)) return true;

  const minutes = presetToMinutes(preset);
  if (minutes) {
    return eventTime >= Date.now() - minutes * 60 * 1000;
  }

  if (preset === "custom") {
    if (start) {
      const startMs = new Date(start).getTime();
      if (!Number.isNaN(startMs) && eventTime < startMs) return false;
    }
    if (end) {
      const endMs = new Date(end).getTime();
      if (!Number.isNaN(endMs) && eventTime > endMs) return false;
    }
  }

  return true;
}

function extractSafetyBadges(
  event: EngineEvent,
  t: (key: string, vars?: Record<string, string | number>) => string,
): Array<{ text: string; tone: SafetyTone }> {
  if (event.event_type !== "ebpf.detached") return [];

  const badges: Array<{ text: string; tone: SafetyTone }> = [];
  const clean = typeof event.payload.clean === "boolean" ? event.payload.clean : undefined;
  if (clean === true) {
    badges.push({ text: t("events.detachClean"), tone: "ok" });
  } else if (clean === false) {
    badges.push({ text: t("events.detachWithRisk"), tone: "warn" });
  }

  const notes = Array.isArray(event.payload.safety_notes)
    ? event.payload.safety_notes.filter((item): item is string => typeof item === "string")
    : [];

  for (const note of notes) {
    badges.push({
      text: mapSafetyNoteToLabel(note, t),
      tone: "warn",
    });
  }

  return badges;
}

function mapSafetyNoteToLabel(
  note: string,
  t: (key: string, vars?: Record<string, string | number>) => string,
): string {
  if (note.includes("still exists after detach")) return t("events.residualPinPath");
  if (note.includes("still tracked in attachment set")) return t("events.attachmentTrackingResidue");
  if (note.includes("detach all requested but")) return t("events.detachAllIncomplete");
  return note;
}

function buildFilterParams(input: {
  categoryFilter: "all" | "kernel" | "platform";
  severityFilter: "all" | "success" | "warning" | "error";
  rangePreset: "all" | "10m" | "1h" | "24h" | "custom";
  startTime: string;
  endTime: string;
  exportFormat?: "json" | "csv";
  limit?: number;
}): URLSearchParams {
  const params = new URLSearchParams();
  if (input.exportFormat) params.set("format", input.exportFormat);
  if (input.categoryFilter !== "all") params.set("category", input.categoryFilter);
  if (input.severityFilter !== "all") params.set("severity", input.severityFilter);
  if (input.limit) params.set("limit", String(input.limit));
  const minutes = presetToMinutes(input.rangePreset);
  if (minutes) params.set("since_minutes", String(minutes));
  if (input.rangePreset === "custom") {
    if (input.startTime) params.set("start", new Date(input.startTime).toISOString());
    if (input.endTime) params.set("end", new Date(input.endTime).toISOString());
  }
  return params;
}
