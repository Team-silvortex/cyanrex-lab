import { useCallback, useEffect, useState } from "react";

import { loadPageState, savePageState } from "../../utils/pageState";
import type {
  EbpfCheckBackend,
  EbpfCheckBackendInventory,
  EbpfCompilerTarget,
} from "./models";

const STORAGE_KEY = "ebpf_compiler_target_v1";

function loadTarget(): EbpfCompilerTarget {
  const stored = loadPageState<string>(STORAGE_KEY);
  return stored === "local" || stored?.startsWith("agent:") ? stored as EbpfCompilerTarget : "local";
}

export function useCompileBackends(engineUrl: string) {
  const [target, setTargetState] = useState<EbpfCompilerTarget>(loadTarget);
  const [agents, setAgents] = useState<EbpfCheckBackend[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    try {
      const response = await fetch(`${engineUrl}/ebpf/check/backends`, {
        credentials: "include",
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const inventory = (await response.json()) as EbpfCheckBackendInventory;
      setAgents(inventory.agents ?? []);
      setError("");
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setLoading(false);
    }
  }, [engineUrl]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 15_000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const setTarget = (next: EbpfCompilerTarget) => {
    setTargetState(next);
    savePageState(STORAGE_KEY, next);
  };

  return { target, setTarget, agents, loading, error, refresh };
}
