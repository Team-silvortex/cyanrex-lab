import { useCallback, useEffect, useRef, useState } from "react";

import type {
  RunnerAdminNotice,
  RunnerAgentInventory,
  RunnerJobInventory,
} from "./models";

export function useRunnerAgentAdmin(engineUrl: string) {
  const [agents, setAgents] = useState<RunnerAgentInventory | null>(null);
  const [jobs, setJobs] = useState<RunnerJobInventory | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [actionId, setActionId] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState<RunnerAdminNotice | null>(null);
  const mounted = useRef(true);
  const refreshInFlight = useRef(false);

  const refresh = useCallback(async ({ silent = false }: { silent?: boolean } = {}) => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    if (!silent) setRefreshing(true);
    try {
      const [agentResponse, jobResponse] = await Promise.all([
        fetch(`${engineUrl}/runner/agents`, { credentials: "include" }),
        fetch(`${engineUrl}/runner/jobs`, { credentials: "include" }),
      ]);
      if (!agentResponse.ok || !jobResponse.ok) {
        throw new Error(`HTTP ${!agentResponse.ok ? agentResponse.status : jobResponse.status}`);
      }
      const [agentInventory, jobInventory] = await Promise.all([
        agentResponse.json() as Promise<RunnerAgentInventory>,
        jobResponse.json() as Promise<RunnerJobInventory>,
      ]);
      if (mounted.current) {
        setAgents(agentInventory);
        setJobs(jobInventory);
        setError("");
      }
    } catch (err) {
      if (mounted.current) setError((err as Error).message);
    } finally {
      refreshInFlight.current = false;
      if (mounted.current) {
        setLoading(false);
        if (!silent) setRefreshing(false);
      }
    }
  }, [engineUrl]);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    const timer = window.setInterval(() => void refresh({ silent: true }), 10_000);
    return () => {
      mounted.current = false;
      window.clearInterval(timer);
    };
  }, [refresh]);

  const postAction = useCallback(async (
    actionKey: string,
    path: string,
    body: Record<string, unknown>,
    nextNotice: RunnerAdminNotice,
  ) => {
    setActionId(actionKey);
    setError("");
    setNotice(null);
    try {
      const response = await fetch(`${engineUrl}${path}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include",
        body: JSON.stringify(body),
      });
      if (!response.ok) {
        const payload = await response.json().catch(() => ({})) as { message?: string };
        throw new Error(payload.message || `HTTP ${response.status}`);
      }
      if (mounted.current) setNotice(nextNotice);
      await refresh({ silent: true });
    } catch (err) {
      if (mounted.current) setError((err as Error).message);
    } finally {
      if (mounted.current) setActionId("");
    }
  }, [engineUrl, refresh]);

  const probeAgent = (agentId: string) => postAction(
    `probe:${agentId}`,
    "/runner/jobs/probe",
    { agent_id: agentId, message: "administrator health probe", timeout_seconds: 30 },
    { kind: "probe_submitted", subject: agentId },
  );

  const cancelJob = (jobId: string) => postAction(
    `cancel:${jobId}`,
    "/runner/jobs/cancel",
    { job_id: jobId },
    { kind: "cancel_requested", subject: jobId },
  );

  return {
    agents,
    jobs,
    loading,
    refreshing,
    actionId,
    error,
    notice,
    refresh,
    probeAgent,
    cancelJob,
  };
}
