import { useEffect, useRef, useState } from "react";

import { toWebSocketUrl } from "../../config/runtime";
import type { EbpfBreakpointHit } from "./models";

type EngineEvent = {
  timestamp?: unknown;
  event_type?: unknown;
  payload?: unknown;
};

const MAX_VISIBLE_HITS = 50;

export function useBreakpointHitStream(engineUrl: string, sessionId: string | null) {
  const [hits, setHits] = useState<EbpfBreakpointHit[]>([]);
  const seenRef = useRef(new Set<string>());

  useEffect(() => {
    setHits([]);
    seenRef.current.clear();
    if (!sessionId) {
      return;
    }

    let alive = true;
    let socket: WebSocket | null = null;

    const acceptEvent = (event: EngineEvent) => {
      if (!alive || event.event_type !== "ebpf.debug_breakpoint_hit") {
        return;
      }
      const payload = event.payload;
      if (!payload || typeof payload !== "object") {
        return;
      }
      const data = payload as Record<string, unknown>;
      if (data.debug_session_id !== sessionId || typeof data.line !== "number") {
        return;
      }
      const timestamp = typeof event.timestamp === "string" ? event.timestamp : new Date().toISOString();
      const key = `${timestamp}:${data.line}`;
      if (seenRef.current.has(key)) {
        return;
      }
      seenRef.current.add(key);
      setHits((current) => [...current.slice(-(MAX_VISIBLE_HITS - 1)), {
        line: data.line as number,
        timestamp,
      }]);
    };

    socket = new WebSocket(toWebSocketUrl(engineUrl, "/ws/events"));
    socket.onmessage = (message) => {
      try {
        acceptEvent(JSON.parse(message.data as string) as EngineEvent);
      } catch {
        // Ignore malformed or unrelated event frames.
      }
    };

    void fetch(`${engineUrl}/events?limit=200`, { credentials: "include" })
      .then(async (response) => response.ok ? response.json() as Promise<EngineEvent[]> : [])
      .then((events) => events.forEach(acceptEvent))
      .catch(() => undefined);

    return () => {
      alive = false;
      socket?.close();
    };
  }, [engineUrl, sessionId]);

  return hits;
}
