import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useI18n } from "../../i18n/context";
import { buildHotspotSummary } from "./hotspots";
import type { PerformanceMetrics } from "./models";

export function usePerformanceMetrics(engineUrl: string) {
  const { t } = useI18n();
  const [metrics, setMetrics] = useState<PerformanceMetrics | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const mounted = useRef(true);
  const refreshInFlight = useRef(false);

  const refresh = useCallback(async ({ silent = false }: { silent?: boolean } = {}) => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    if (!silent) {
      setRefreshing(true);
      setMessage("");
      setError("");
    }
    try {
      const response = await fetch(`${engineUrl}/settings/performance`, {
        credentials: "include",
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const payload = (await response.json()) as PerformanceMetrics;
      if (mounted.current) {
        setMetrics(payload);
        if (!silent) setMessage(t("settings.metricsUpdated"));
      }
    } catch (err) {
      if (!silent && mounted.current) setError((err as Error).message);
    } finally {
      refreshInFlight.current = false;
      if (!silent && mounted.current) setRefreshing(false);
    }
  }, [engineUrl, t]);

  const clearFeedback = useCallback(() => {
    setMessage("");
    setError("");
  }, []);

  useEffect(() => {
    mounted.current = true;
    void refresh({ silent: true });
    const timer = window.setInterval(() => void refresh({ silent: true }), 10_000);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
    };
  }, [refresh]);

  const hotspotSummary = useMemo(
    () => metrics ? buildHotspotSummary(metrics, t) : null,
    [metrics, t],
  );

  return { metrics, hotspotSummary, refreshing, message, error, refresh, clearFeedback };
}
