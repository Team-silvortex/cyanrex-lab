import type { EbpfCheckBackend, EbpfCompilerTarget } from "./models";

type Props = {
  target: EbpfCompilerTarget;
  agents: EbpfCheckBackend[];
  loading: boolean;
  error: string;
  setTarget: (target: EbpfCompilerTarget) => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
};

export default function CompileBackendSelector({
  target,
  agents,
  loading,
  error,
  setTarget,
  t,
}: Props) {
  const selectedAgentId = target.startsWith("agent:") ? target.slice(6) : "";
  const selectedAgent = agents.find((agent) => agent.agent_id === selectedAgentId);
  const missingTarget = Boolean(selectedAgentId && !selectedAgent);

  return (
    <div style={{ marginTop: 10 }}>
      <label className="meta">
        {t("ebpf.compilerBackend")}:{" "}
        <select
          value={target}
          onChange={(event) => setTarget(event.target.value as EbpfCompilerTarget)}
          style={{ marginLeft: 6 }}
        >
          <option value="local">{t("ebpf.compilerBackendLocal")}</option>
          {missingTarget && (
            <option value={target}>{t("ebpf.compilerBackendMissing", { agent: selectedAgentId })}</option>
          )}
          {agents.map((agent) => (
            <option key={agent.agent_id} value={`agent:${agent.agent_id}`}>
              {agent.agent_id} · {agent.isolation} · {agent.available_slots}/{agent.max_concurrent}
            </option>
          ))}
        </select>
      </label>
      <p className="meta" style={{ margin: "6px 0 0" }}>
        {loading
          ? t("common.checking")
          : error
            ? t("ebpf.compilerBackendDiscoveryFailed", { error })
            : target === "local"
              ? t("ebpf.compilerBackendLocalHint")
              : missingTarget
                ? t("ebpf.compilerBackendUnavailableHint")
                : t("ebpf.compilerBackendRemoteHint")}
      </p>
    </div>
  );
}
