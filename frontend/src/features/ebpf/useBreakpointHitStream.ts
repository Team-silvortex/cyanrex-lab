import { useEffect, useState } from "react";

import { toWebSocketUrl } from "../../config/runtime";
import { startEventStream, type EngineEvent, type EventStreamState } from "../events/eventStream";
import type { EbpfBreakpointHit } from "./models";

const MAX_VISIBLE_HITS = 50;

export function useBreakpointHitStream(engineUrl: string, sessionId: string | null) {
  const [hits, setHits] = useState<EbpfBreakpointHit[]>([]);
  const [streamGap, setStreamGap] = useState(false);
  const [connection, setConnection] = useState<EventStreamState>("closed");

  useEffect(() => {
    setHits([]);
    setStreamGap(false);
    setConnection("closed");
    if (!sessionId) {
      return;
    }

    const accepts = (event: EngineEvent) => event.event_type === "ebpf.debug_breakpoint_hit"
      && event.payload.debug_session_id === sessionId && typeof event.payload.line === "number";
    return startEventStream({
      socketUrl: toWebSocketUrl(engineUrl, "/ws/events"),
      snapshotUrl: `${engineUrl}/events?category=kernel&limit=200`,
      accepts,
      onEvents: (events) => setHits(events.slice(-MAX_VISIBLE_HITS).map((event) => ({
        line: event.payload.line as number, timestamp: event.timestamp,
      }))),
      onState: setConnection,
      onGap: () => setStreamGap(true),
    });
  }, [engineUrl, sessionId]);

  return { hits, streamGap, connection };
}
