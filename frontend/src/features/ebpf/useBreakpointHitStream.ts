import { useEffect, useState } from "react";

import { toWebSocketUrl } from "../../config/runtime";
import { startEventStream, type EngineEvent, type EventStreamState } from "../events/eventStream";
import type { EbpfBreakpointHit } from "./models";

const MAX_VISIBLE_HITS = 50;

type View = { key: string; hits: EbpfBreakpointHit[]; streamGap: boolean; connection: EventStreamState };

export function useBreakpointHitStream(engineUrl: string, sessionId: string | null, instrumentedLines: readonly number[] = []) {
  const lineKey = JSON.stringify([...new Set(instrumentedLines.filter(line => Number.isSafeInteger(line) && line > 0))].sort((a, b) => a - b));
  const key = JSON.stringify([engineUrl, sessionId, lineKey]);
  const enabled = Boolean(sessionId?.trim()) && lineKey !== "[]";
  const empty: View = { key, hits: [], streamGap: false, connection: enabled ? "connecting" : "closed" };
  const [view, setView] = useState<View>(empty);

  useEffect(() => {
    setView(empty);
    if (!enabled) return;

    const allowed = new Set<number>(JSON.parse(lineKey));
    const accepts = (event: EngineEvent) => event.category === "kernel" && event.event_type === "ebpf.debug_breakpoint_hit"
      && event.payload.debug_session_id === sessionId && Number.isSafeInteger(event.payload.line)
      && allowed.has(event.payload.line as number);
    return startEventStream({
      socketUrl: toWebSocketUrl(engineUrl, "/ws/events"),
      snapshotUrl: `${engineUrl}/events?category=kernel&limit=200`,
      accepts,
      onEvents: events => setView(previous => ({ ...previous, hits: events.slice(-MAX_VISIBLE_HITS).map(event => ({
        line: event.payload.line as number, timestamp: event.timestamp,
      })) })),
      onState: connection => setView(previous => ({ ...previous, connection })),
      onGap: () => setView(previous => ({ ...previous, streamGap: true })),
    });
  }, [key]);

  // Never expose previous-session data during the render before effect cleanup/reset.
  const current = view.key === key ? view : empty;
  return { hits: current.hits, streamGap: current.streamGap, connection: current.connection };
}
