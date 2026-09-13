import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { HeaderSelectionMetadata, SelectedHeaderMetadata } from "./models";

export function useSelectedHeaders(engineUrl: string) {
  const [view, setView] = useState({ engineUrl, headers: [] as SelectedHeaderMetadata[], loading: false, failed: false, revision: 0 });
  const active = useRef<AbortController | null>(null);
  const refresh = useCallback(async () => {
    active.current?.abort();
    const controller = new AbortController(); active.current = controller;
    // Refresh itself invalidates checks/caches, even if filenames do not change.
    setView(previous => ({ engineUrl, headers: previous.engineUrl === engineUrl ? previous.headers : [],
      loading: true, failed: false, revision: previous.revision + 1 }));
    const timer = setTimeout(() => controller.abort(new DOMException("Header metadata timed out", "TimeoutError")), 10_000);
    try {
      const response = await fetch(`${engineUrl}/modules/c-headers/selected-metadata`, {
        credentials: "include", cache: "no-store", redirect: "error", signal: controller.signal,
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const result = await response.json() as HeaderSelectionMetadata;
      controller.signal.throwIfAborted();
      if (!Array.isArray(result?.selected_headers) || !result.selected_headers.every(item => item
        && typeof item.id === "string" && typeof item.include_hint === "string"
        && typeof item.local_path === "string" && typeof item.downloaded === "boolean")) {
        throw new Error("invalid selected-header metadata");
      }
      if (active.current === controller) setView(previous => ({ ...previous, headers: result.selected_headers, loading: false, failed: false }));
    } catch {
      if (active.current === controller) setView(previous => ({ ...previous, loading: false, failed: true }));
    } finally {
      clearTimeout(timer);
      if (active.current === controller) active.current = null;
    }
  }, [engineUrl]);
  useEffect(() => {
    void refresh();
    return () => { active.current?.abort(); active.current = null; };
  }, [refresh]);
  const headers = view.engineUrl === engineUrl ? view.headers : [];
  const contextKey = useMemo(() => JSON.stringify([engineUrl, view.revision,
    headers.map(item => [item.id, item.include_hint, item.local_path, item.downloaded]).sort((a, b) => String(a[0]).localeCompare(String(b[0]))),
  ]), [engineUrl, view.revision, headers]);
  return { injectedMetadata: headers, injectedHeaderContext: contextKey, refreshInjectedMetadata: refresh,
    injectedMetadataError: view.engineUrl === engineUrl && view.failed,
    injectedMetadataLoading: view.engineUrl !== engineUrl || view.loading };
}
