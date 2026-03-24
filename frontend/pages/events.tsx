import { useEffect, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";
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

  const engineUrl = useMemo(
    () => process.env.NEXT_PUBLIC_ENGINE_URL ?? "http://localhost:8080",
    [],
  );

  useEffect(() => {
    let ws: WebSocket | null = null;
    let alive = true;

    const loadSnapshot = async () => {
      try {
        const response = await fetch(`${engineUrl}/events`, {
          credentials: "include",
        });
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        const snapshot = (await response.json()) as EngineEvent[];
        if (alive) {
          setEvents(snapshot.slice(-200));
          await fetch(`${engineUrl}/events/mark-read`, {
            method: "POST",
            credentials: "include",
          });
        }
      } catch (err) {
        if (alive) setError((err as Error).message);
      }
    };

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
          setEvents((prev) => [...prev, event].slice(-200));
        } catch {
          // ignore malformed event frame
        }
      };

      ws.onerror = () => {
        if (!alive) return;
        setError("WebSocket error");
      };

      ws.onclose = () => {
        if (!alive) return;
        setConnection("closed");
      };
    };

    loadSnapshot();
    openWs();

    const markReadTimer = setInterval(() => {
      fetch(`${engineUrl}/events/mark-read`, {
        method: "POST",
        credentials: "include",
      }).catch(() => undefined);
    }, 2000);

    return () => {
      alive = false;
      ws?.close();
      clearInterval(markReadTimer);
    };
  }, [engineUrl]);

  const filteredEvents = useMemo(() => {
    return events.filter((event) => {
      const categoryPass = categoryFilter === "all" || event.category === categoryFilter;
      const severityPass = severityFilter === "all" || event.severity === severityFilter;
      const rangePass = timeFilterPass(event.timestamp, rangePreset, startTime, endTime);
      return categoryPass && severityPass && rangePass;
    });
  }, [events, categoryFilter, severityFilter, rangePreset, startTime, endTime]);

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
    const count = filteredEvents.length;
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

      const snapshotResp = await fetch(`${engineUrl}/events`, {
        credentials: "include",
      });
      if (!snapshotResp.ok) {
        throw new Error(`HTTP ${snapshotResp.status}`);
      }
      const snapshot = (await snapshotResp.json()) as EngineEvent[];
      setEvents(snapshot.slice(-200));
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
          {t("events.status")}: {connection} | {t("events.total")}: {events.length} | {t("events.filtered")}: {filteredEvents.length} | {t("events.activeFilters", { count: activeFilterCount })}
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
              <option value="json">json</option>
              <option value="csv">csv</option>
            </select>
          </label>
          <button type="button" onClick={exportEvents}>{t("events.exportDownload")}</button>
          <button type="button" onClick={deleteFilteredEvents}>{t("events.deleteFiltered")}</button>
        </div>
        {error && <p className="error">{error}</p>}
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        {filteredEvents.length === 0 && <p className="meta">{t("events.noEvents")}</p>}
        {filteredEvents
          .slice()
          .reverse()
          .map((event, idx) => {
            const safetyBadges = extractSafetyBadges(event);
            return (
              <article key={`${event.timestamp}-${idx}`} className="panel" style={{ marginBottom: 10, background: "#0b1425" }}>
                <p style={{ margin: 0 }}>
                  <strong>{event.event_type}</strong>
                </p>
                <p className="meta" style={{ margin: "6px 0" }}>
                  {new Date(event.timestamp).toLocaleString()} | source: {event.source} | category: {event.category}
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

function toWebSocketUrl(baseHttpUrl: string, path: string): string {
  const url = new URL(baseHttpUrl);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  url.pathname = path;
  url.search = "";
  url.hash = "";
  return url.toString();
}

function presetToMinutes(preset: "all" | "10m" | "1h" | "24h" | "custom"): number | null {
  if (preset === "10m") return 10;
  if (preset === "1h") return 60;
  if (preset === "24h") return 24 * 60;
  return null;
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

function extractSafetyBadges(event: EngineEvent): Array<{ text: string; tone: SafetyTone }> {
  if (event.event_type !== "ebpf.detached") return [];

  const badges: Array<{ text: string; tone: SafetyTone }> = [];
  const clean = typeof event.payload.clean === "boolean" ? event.payload.clean : undefined;
  if (clean === true) {
    badges.push({ text: "Detach Clean", tone: "ok" });
  } else if (clean === false) {
    badges.push({ text: "Detach With Risk", tone: "warn" });
  }

  const notes = Array.isArray(event.payload.safety_notes)
    ? event.payload.safety_notes.filter((item): item is string => typeof item === "string")
    : [];

  for (const note of notes) {
    badges.push({
      text: mapSafetyNoteToLabel(note),
      tone: "warn",
    });
  }

  return badges;
}

function mapSafetyNoteToLabel(note: string): string {
  if (note.includes("still exists after detach")) return "Residual Pin Path";
  if (note.includes("still tracked in attachment set")) return "Attachment Tracking Residue";
  if (note.includes("detach all requested but")) return "Detach-All Incomplete";
  return note;
}

function buildFilterParams(input: {
  categoryFilter: "all" | "kernel" | "platform";
  severityFilter: "all" | "success" | "warning" | "error";
  rangePreset: "all" | "10m" | "1h" | "24h" | "custom";
  startTime: string;
  endTime: string;
  exportFormat?: "json" | "csv";
}): URLSearchParams {
  const params = new URLSearchParams();
  if (input.exportFormat) params.set("format", input.exportFormat);
  if (input.categoryFilter !== "all") params.set("category", input.categoryFilter);
  if (input.severityFilter !== "all") params.set("severity", input.severityFilter);
  const minutes = presetToMinutes(input.rangePreset);
  if (minutes) params.set("since_minutes", String(minutes));
  if (input.rangePreset === "custom") {
    if (input.startTime) params.set("start", new Date(input.startTime).toISOString());
    if (input.endTime) params.set("end", new Date(input.endTime).toISOString());
  }
  return params;
}
