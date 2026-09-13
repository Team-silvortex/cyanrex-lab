import { useCallback, useEffect, useRef, useState } from "react";
import type { EbpfAttachmentDetail } from "./models";
import { ATTACHMENT_READ_TIMEOUT_MS, parseAttachmentDetails, requestRuntimeJson } from "./runtimeRequest";

type InventoryState = "loading" | "ready" | "stale" | "error";
export function useAttachmentInventory(engineUrl: string) {
  const [view, setView] = useState({ engineUrl, items: [] as EbpfAttachmentDetail[], state: "loading" as InventoryState });
  const active = useRef<AbortController | null>(null);
  const mounted = useRef(false), currentEngine = useRef(engineUrl);
  currentEngine.current = engineUrl;
  const invalidateAttachments = useCallback(() => {
    active.current?.abort(); active.current = null;
    if (mounted.current) setView(previous => ({ ...previous, state: "stale" }));
  }, []);
  const refreshAttachments = useCallback(async () => {
    if (!mounted.current || currentEngine.current !== engineUrl) return false;
    active.current?.abort();
    const controller = new AbortController(); active.current = controller;
    setView(previous => ({ engineUrl, items: previous.engineUrl === engineUrl ? previous.items : [], state: "loading" }));
    try {
      const items = parseAttachmentDetails(await requestRuntimeJson(`${engineUrl}/ebpf/attachments/details`,
        controller.signal, undefined, ATTACHMENT_READ_TIMEOUT_MS));
      if (active.current !== controller) return false;
      setView({ engineUrl, items, state: "ready" }); return true;
    } catch {
      if (active.current === controller) setView(previous => ({ ...previous, state: "error" }));
      return false;
    } finally { if (active.current === controller) active.current = null; }
  }, [engineUrl]);
  useEffect(() => {
    mounted.current = true; void refreshAttachments();
    return () => { mounted.current = false; active.current?.abort(); active.current = null; };
  }, [refreshAttachments]);
  return {
    attachmentDetails: view.engineUrl === engineUrl ? view.items : [],
    attachmentState: view.engineUrl === engineUrl ? view.state : "loading" as InventoryState,
    refreshAttachments, invalidateAttachments,
  };
}
