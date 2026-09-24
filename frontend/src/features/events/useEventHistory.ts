import { useEffect, useRef, useState } from "react";
import { toWebSocketUrl } from "../../config/runtime";
import { buildEventFilterParams, matchesEventFilters, presetMinutes, type EventFilters } from "./eventFilters";
import { startEventStream, type EngineEvent, type EventStreamState } from "./eventStream";

type View = { key: string; context: string; rows: EngineEvent[]; connection: EventStreamState; gap: boolean };
export function useEventHistory(engineUrl: string, navigation: string, filters: EventFilters,
  revision: number, scheduleMarkRead: () => void) {
  const context = JSON.stringify([engineUrl, navigation, filters]), key = JSON.stringify([context, revision]);
  const current = useRef(key); current.current = key;
  const [view, setView] = useState<View>({ key, context, rows: [], connection: "connecting", gap: false });
  const [, tick] = useState(0);
  let query: string | null = null;
  try { query = buildEventFilterParams(filters, 200).toString(); } catch { /* Invalid filters never issue a wider query. */ }
  useEffect(() => {
    if (query === null) return;
    let active = true;
    const update = (patch: Partial<View>) => {
      if (active && current.current === key) setView(previous => ({
        ...(previous.key === key ? previous : { key, context, rows: [], connection: "connecting", gap: previous.context === context && previous.gap }), ...patch,
      }));
    };
    update({ rows: [], connection: "connecting" });
    const dispose = startEventStream({
      socketUrl: toWebSocketUrl(engineUrl, "/ws/events"), snapshotUrl: `${engineUrl}/events?${query}`,
      accepts: row => matchesEventFilters(row, filters),
      onEvents: rows => { if (active && current.current === key) { update({ rows }); scheduleMarkRead(); } },
      onState: connection => update({ connection }), onGap: () => update({ gap: true }),
    });
    // Aging only changes the local view. It must not reconnect or acknowledge events on every tick.
    const timer = presetMinutes(filters.rangePreset) ? window.setInterval(() => tick(value => value + 1), 1000) : null;
    return () => { active = false; dispose(); if (timer !== null) window.clearInterval(timer); };
  }, [key, query, engineUrl, scheduleMarkRead]);
  const visible = view.key === key && query !== null;
  return { valid: query !== null, events: visible ? view.rows.filter(row => matchesEventFilters(row, filters)) : [],
    connection: query === null ? "closed" as const : visible ? view.connection : "connecting" as const,
    streamGap: query !== null && view.context === context && view.gap };
}
