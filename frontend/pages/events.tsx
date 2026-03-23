import { useEffect, useMemo, useState } from "react";

import SidebarLayout from "../src/components/SidebarLayout";

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

export default function EventsPage() {
  const [events, setEvents] = useState<EngineEvent[]>([]);
  const [connection, setConnection] = useState<"connecting" | "open" | "closed">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [categoryFilter, setCategoryFilter] = useState<"all" | "kernel" | "platform">("all");
  const [severityFilter, setSeverityFilter] = useState<"all" | "success" | "warning" | "error">("all");
  const [rangePreset, setRangePreset] = useState<"all" | "10m" | "1h" | "24h" | "custom">("all");
  const [startTime, setStartTime] = useState("");
  const [endTime, setEndTime] = useState("");
  const [exportFormat, setExportFormat] = useState<"json" | "csv">("json");

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

  const exportEvents = async () => {
    const params = new URLSearchParams();
    params.set("format", exportFormat);
    if (categoryFilter !== "all") params.set("category", categoryFilter);
    if (severityFilter !== "all") params.set("severity", severityFilter);
    const minutes = presetToMinutes(rangePreset);
    if (minutes) params.set("since_minutes", String(minutes));
    if (rangePreset === "custom") {
      if (startTime) params.set("start", new Date(startTime).toISOString());
      if (endTime) params.set("end", new Date(endTime).toISOString());
    }

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

  return (
    <SidebarLayout title="Cyanrex Events">
      <section className="panel">
        <h2>Events</h2>
        <p className="meta">
          status: {connection} | total: {events.length} | filtered: {filteredEvents.length}
        </p>
        <div className="row" style={{ marginTop: 10 }}>
          <label className="meta">
            category:
            {" "}
            <select value={categoryFilter} onChange={(event) => setCategoryFilter(event.target.value as typeof categoryFilter)}>
              <option value="all">all</option>
              <option value="kernel">kernel</option>
              <option value="platform">platform</option>
            </select>
          </label>
          <label className="meta">
            severity:
            {" "}
            <select value={severityFilter} onChange={(event) => setSeverityFilter(event.target.value as typeof severityFilter)}>
              <option value="all">all</option>
              <option value="success">success</option>
              <option value="warning">warning</option>
              <option value="error">error</option>
            </select>
          </label>
          <label className="meta">
            range:
            {" "}
            <select value={rangePreset} onChange={(event) => setRangePreset(event.target.value as typeof rangePreset)}>
              <option value="all">all</option>
              <option value="10m">last 10m</option>
              <option value="1h">last 1h</option>
              <option value="24h">last 24h</option>
              <option value="custom">custom</option>
            </select>
          </label>
          {rangePreset === "custom" && (
            <>
              <label className="meta">
                start:
                {" "}
                <input type="datetime-local" value={startTime} onChange={(event) => setStartTime(event.target.value)} />
              </label>
              <label className="meta">
                end:
                {" "}
                <input type="datetime-local" value={endTime} onChange={(event) => setEndTime(event.target.value)} />
              </label>
            </>
          )}
          <label className="meta">
            export:
            {" "}
            <select value={exportFormat} onChange={(event) => setExportFormat(event.target.value as typeof exportFormat)}>
              <option value="json">json</option>
              <option value="csv">csv</option>
            </select>
          </label>
          <button type="button" onClick={exportEvents}>Export Download</button>
        </div>
        {error && <p className="error">{error}</p>}
      </section>

      <section className="panel" style={{ marginTop: 16 }}>
        {filteredEvents.length === 0 && <p className="meta">No events yet. Run an eBPF task to generate stream data.</p>}
        {filteredEvents
          .slice()
          .reverse()
          .map((event, idx) => (
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
              <pre style={{ margin: 0 }}>{JSON.stringify(event.payload, null, 2)}</pre>
            </article>
          ))}
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
