import { useCallback, useEffect, useRef, useState } from "react";
import { EVENT_UNREAD_TIMEOUT_MS, notifyUnreadChanged, parseReadAcknowledgement, requestEvent } from "./eventRequest";

// The existing server endpoint acknowledges ALL current-owner events, not only filtered/visible rows.
export function useEventReadAcknowledgement(engineUrl: string, navigation: string) {
  const key = JSON.stringify([engineUrl, navigation]), current = useRef(key); current.current = key;
  const mounted = useRef(false), request = useRef<AbortController | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null), failed = useRef(false), dirty = useRef(false);
  const [view, setView] = useState({ key, state: "idle" });
  const scheduleMarkRead = useCallback(() => {
    if (!mounted.current || current.current !== key || failed.current) return;
    dirty.current = true;
    if (timer.current !== null || request.current) return;
    timer.current = setTimeout(() => {
      timer.current = null;
      if (!mounted.current || current.current !== key || failed.current) return;
      const controller = new AbortController(); request.current = controller; dirty.current = false;
      const owns = () => mounted.current && current.current === key && request.current === controller;
      setView({ key, state: "pending" });
      void requestEvent(`${engineUrl}/events/mark-read`, controller.signal,
        async response => parseReadAcknowledgement(await response.json()), "POST", EVENT_UNREAD_TIMEOUT_MS)
        .then(() => { if (owns()) { setView({ key, state: "idle" }); notifyUnreadChanged(engineUrl); } })
        .catch(() => { if (owns()) { failed.current = true; setView({ key, state: "error" }); } })
        .finally(() => { if (owns()) { request.current = null; if (dirty.current && !failed.current) scheduleMarkRead(); } });
    }, 1200);
  }, [key, engineUrl]);
  useEffect(() => {
    mounted.current = true; failed.current = false; dirty.current = false;
    setView({ key, state: "idle" });
    return () => {
      mounted.current = false; request.current?.abort(); request.current = null;
      if (timer.current !== null) clearTimeout(timer.current); timer.current = null;
    };
  }, [key]);
  const retryMarkRead = () => {
    if (!mounted.current || current.current !== key || request.current) return;
    failed.current = false; setView({ key, state: "pending" }); scheduleMarkRead();
  };
  return { scheduleMarkRead, retryMarkRead, readState: view.key === key ? view.state : "idle" };
}
