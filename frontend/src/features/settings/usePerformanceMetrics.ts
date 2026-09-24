import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useI18n } from "../../i18n/context";
import { buildHotspotSummary } from "./hotspots";
import type { PerformanceMetrics } from "./models";
import { parsePerformanceMetrics } from "./performanceMetrics";
import { requestSettings } from "./settingsRequest";

const POLL_DELAY_MS = 10_000;
type State = { scope: string; metrics: PerformanceMetrics | null; refreshing: boolean; errorKey: string; messageKey: string };
type Generation = { scope: string; controller: AbortController; refresh: (silent: boolean) => Promise<void> };
const initial = (scope: string): State => ({ scope, metrics: null, refreshing: true, errorKey: "", messageKey: "" });

export function usePerformanceMetrics(engineUrl: string, navigation = "") {
  const { t } = useI18n();
  const scope = JSON.stringify([engineUrl, navigation]);
  const currentScope = useRef(scope); currentScope.current = scope;
  const generation = useRef<Generation | null>(null);
  const [state, setState] = useState(() => initial(scope));
  const view = state.scope === scope ? state : initial(scope);

  useEffect(() => {
    const controller = new AbortController();
    let inFlight: Promise<void> | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const owns = () => generation.current?.controller === controller
      && currentScope.current === scope && !controller.signal.aborted;

    const refresh = (silent: boolean): Promise<void> => {
      if (!owns()) return Promise.resolve();
      if (inFlight) return inFlight;
      clearTimeout(timer);
      setState(previous => ({ ...previous, refreshing: true, messageKey: "" }));
      const work = (async () => {
        try {
          const metrics = await requestSettings(`${engineUrl}/settings/performance`, controller.signal, parsePerformanceMetrics);
          if (owns()) setState({ scope, metrics, refreshing: true, errorKey: "",
            messageKey: silent ? "" : "settings.metricsUpdated" });
        } catch {
          if (owns()) setState(previous => ({ ...previous, errorKey: "settings.metricsReadFailed", messageKey: "" }));
        } finally {
          if (owns()) {
            inFlight = null;
            setState(previous => ({ ...previous, refreshing: false }));
            // Wait after completion, including failure. No overlapping interval requests or retry loop.
            timer = setTimeout(() => { void refresh(true); }, POLL_DELAY_MS);
          }
        }
      })();
      inFlight = work;
      return work;
    };

    generation.current = { scope, controller, refresh };
    setState(initial(scope));
    void refresh(true);
    return () => {
      controller.abort();
      clearTimeout(timer);
      if (generation.current?.controller === controller) generation.current = null;
    };
  }, [engineUrl, scope]);

  const refresh = useCallback(({ silent = false }: { silent?: boolean } = {}) => {
    const owner = generation.current;
    return owner?.scope === scope && currentScope.current === scope ? owner.refresh(silent) : Promise.resolve();
  }, [scope]);
  const hotspotSummary = useMemo(() => view.metrics ? buildHotspotSummary(view.metrics, t) : null, [view.metrics, t]);

  return { metrics: view.metrics, hotspotSummary, refreshing: view.refreshing,
    stale: Boolean(view.metrics && view.errorKey),
    message: view.messageKey ? t(view.messageKey) : "", error: view.errorKey ? t(view.errorKey) : "", refresh };
}
