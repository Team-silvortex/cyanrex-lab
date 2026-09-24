import { useEffect, useRef, useState } from "react";
import { EVENT_UNREAD_TIMEOUT_MS, parseUnread, requestEvent, UNREAD_CHANGED } from "./eventRequest";

export function useUnreadEvents(engineUrl: string, navigation: string, enabled: boolean) {
  const key = JSON.stringify([engineUrl, navigation, enabled]);
  const current = useRef(key); current.current = key;
  const [view, setView] = useState({ key, count: 0, unavailable: false });
  useEffect(() => {
    if (!enabled) return;
    let active = true, request: AbortController | null = null, timer: ReturnType<typeof setTimeout> | null = null;
    const refresh = async () => {
      if (timer !== null) clearTimeout(timer);
      request?.abort();
      const controller = new AbortController(); request = controller;
      const owns = () => active && current.current === key && request === controller;
      try {
        const count = await requestEvent(`${engineUrl}/events/unread-count`, controller.signal,
          async response => parseUnread(await response.json()), "GET", EVENT_UNREAD_TIMEOUT_MS);
        if (owns()) setView({ key, count, unavailable: false });
      } catch {
        if (owns()) setView(previous => ({ key, count: previous.key === key ? previous.count : 0, unavailable: true }));
      } finally {
        if (owns()) { request = null; timer = setTimeout(() => void refresh(), 4000); }
      }
    };
    const changed = (event: Event) => {
      if ((event as CustomEvent<{ engineUrl: string }>).detail?.engineUrl === engineUrl) void refresh();
    };
    window.addEventListener(UNREAD_CHANGED, changed);
    void refresh();
    return () => {
      active = false; request?.abort(); if (timer !== null) clearTimeout(timer);
      window.removeEventListener(UNREAD_CHANGED, changed);
    };
  }, [key, engineUrl, enabled]);
  return view.key === key ? view : { key, count: 0, unavailable: false };
}
