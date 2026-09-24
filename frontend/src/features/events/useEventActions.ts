import { useEffect, useRef, useState } from "react";
import { buildEventFilterParams, type EventFilters, type ExportFormat } from "./eventFilters";
import { decodeEventExport, EventHttpError, notifyUnreadChanged, parseEventDeletion, requestEvent } from "./eventRequest";

type Translate = (key: string) => string;
export function useEventActions(engineUrl: string, navigation: string, filters: EventFilters, format: ExportFormat,
  refresh: () => void, t: Translate) {
  const scope = JSON.stringify([engineUrl, navigation]), exportScope = JSON.stringify([scope, filters, format]);
  const current = useRef({ scope, exportScope }); current.current = { scope, exportScope };
  const mounted = useRef(false), exporting = useRef<AbortController | null>(null), deleting = useRef<AbortController | null>(null);
  const [exportingScope, setExportingScope] = useState<string | null>(null);
  const [error, setError] = useState({ scope: exportScope, key: "" });
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; deleting.current?.abort(); deleting.current = null; };
  }, [scope]);
  useEffect(() => () => { exporting.current?.abort(); exporting.current = null; }, [exportScope]);
  const exportEvents = async () => {
    if (!mounted.current || current.current.exportScope !== exportScope || exporting.current) return;
    const controller = new AbortController(); exporting.current = controller;
    const owns = () => mounted.current && current.current.exportScope === exportScope && exporting.current === controller;
    setExportingScope(exportScope); setError({ scope: exportScope, key: "" });
    try {
      const params = buildEventFilterParams(filters, undefined, format);
      const { blob, filename } = await requestEvent(`${engineUrl}/events/export?${params}`, controller.signal,
        response => decodeEventExport(response, format));
      if (!owns()) return;
      const url = URL.createObjectURL(blob), anchor = document.createElement("a");
      try {
        anchor.href = url; anchor.download = filename; document.body.appendChild(anchor); anchor.click();
      } finally {
        anchor.remove(); setTimeout(() => URL.revokeObjectURL(url), 0);
      }
    } catch { if (owns()) setError({ scope: exportScope, key: "events.exportFailed" }); }
    finally { if (owns()) { exporting.current = null; setExportingScope(null); } }
  };
  const performDelete = async (params: URLSearchParams, parent: AbortSignal) => {
    parent.throwIfAborted();
    if (!mounted.current || current.current.scope !== scope || deleting.current) throw new Error(t("events.deleteUnconfirmed"));
    const controller = new AbortController(), cancel = () => controller.abort(parent.reason);
    parent.addEventListener("abort", cancel, { once: true }); deleting.current = controller;
    const owns = () => mounted.current && current.current.scope === scope && deleting.current === controller && !parent.aborted;
    try {
      await requestEvent(`${engineUrl}/events/delete?${params}`, controller.signal,
        async response => parseEventDeletion(await response.json()), "POST");
      if (owns()) { setError({ scope: current.current.exportScope, key: "" }); refresh(); notifyUnreadChanged(engineUrl); }
    } catch (cause) {
      const key = cause instanceof EventHttpError && [401, 403].includes(cause.status) ? "events.requestRejected" : "events.deleteUnconfirmed";
      if (owns()) setError({ scope: current.current.exportScope, key });
      throw new Error(t(key));
    } finally { parent.removeEventListener("abort", cancel); if (deleting.current === controller) deleting.current = null; }
  };
  return { exportEvents, performDelete, exporting: exporting.current !== null && exportingScope === exportScope,
    error: error.scope === exportScope && error.key ? t(error.key) : null };
}
