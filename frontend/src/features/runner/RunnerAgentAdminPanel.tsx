import { useMemo } from "react";

import { useI18n } from "../../i18n/context";
import type {
  RunnerAgentIsolation,
  RunnerAgentState,
  RunnerAgentView,
  RunnerJobState,
  RunnerJobView,
} from "./models";
import { useRunnerAgentAdmin } from "./useRunnerAgentAdmin";

type Props = {
  engineUrl: string;
};

const stateColors: Record<RunnerAgentState | RunnerJobState, string> = {
  healthy: "#8ad66a",
  degraded: "#f3b33d",
  draining: "#76b7ff",
  offline: "#9a9a9a",
  queued: "#76b7ff",
  claimed: "#8ad66a",
  cancel_requested: "#f3b33d",
  succeeded: "#8ad66a",
  failed: "#f05f5f",
  cancelled: "#9a9a9a",
  expired: "#f05f5f",
};

export default function RunnerAgentAdminPanel({ engineUrl }: Props) {
  const { t } = useI18n();
  const admin = useRunnerAgentAdmin(engineUrl);
  const agents = admin.agents?.agents ?? [];
  const recentJobs = useMemo(
    () => [...(admin.jobs?.jobs ?? [])]
      .sort((left, right) => Date.parse(right.created_at) - Date.parse(left.created_at))
      .slice(0, 12),
    [admin.jobs],
  );
  const eligibleAgents = agents.filter((agent) => agent.state === "healthy");
  const totalCapacity = eligibleAgents.reduce((total, agent) => total + agent.max_concurrent, 0);
  const freeCapacity = eligibleAgents.reduce((total, agent) => total + agent.available_slots, 0);
  const activeJobs = (admin.jobs?.jobs ?? []).filter((job) => isActiveJob(job.state)).length;
  const notice = admin.notice
    ? t(`settings.runnerNotice.${admin.notice.kind}`, { subject: admin.notice.subject })
    : "";

  return (
    <section className="panel" style={{ marginTop: 14 }}>
      <div className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h3>{t("settings.runnerTitle")}</h3>
          <p className="meta">{t("settings.runnerSubtitle")}</p>
        </div>
        <button
          type="button"
          onClick={() => void admin.refresh()}
          disabled={admin.loading || admin.refreshing}
        >
          {admin.refreshing ? t("settings.runnerRefreshing") : t("settings.runnerRefresh")}
        </button>
      </div>

      {admin.loading && <p className="meta">{t("settings.runnerLoading")}</p>}
      {admin.error && <p className="error">{admin.error}</p>}
      {notice && <p className="meta" style={{ color: "#9cd67a" }}>{notice}</p>}

      {admin.agents && !admin.agents.enabled && (
        <div className="panel" style={{ borderLeft: "4px solid #f3b33d", marginTop: 10 }}>
          <strong>{t("settings.runnerDisabled")}</strong>
          <p className="meta">{t("settings.runnerDisabledHint")}</p>
          <code>./scripts/runner-agent.sh start</code>
        </div>
      )}

      {admin.agents?.enabled && (
        <>
          <div className="grid cols-2" style={{ marginTop: 10 }}>
            <Summary label={t("settings.runnerAgents")} value={`${admin.agents.online_agents}/${admin.agents.total_agents}`} />
            <Summary label={t("settings.runnerCapacity")} value={`${freeCapacity}/${totalCapacity}`} />
            <Summary label={t("settings.runnerActiveJobs")} value={String(activeJobs)} />
            <Summary label={t("settings.runnerRetainedJobs")} value={String(admin.jobs?.total_jobs ?? 0)} />
          </div>

          {agents.length === 0 ? (
            <p className="meta" style={{ marginTop: 12 }}>{t("settings.runnerNoAgents")}</p>
          ) : (
            <div className="grid cols-2" style={{ marginTop: 12 }}>
              {agents.map((agent) => (
                <AgentCard
                  key={agent.agent_id}
                  agent={agent}
                  busy={admin.actionId === `probe:${agent.agent_id}`}
                  onProbe={() => void admin.probeAgent(agent.agent_id)}
                  t={t}
                />
              ))}
            </div>
          )}

          <h4 style={{ marginTop: 16 }}>{t("settings.runnerRecentJobs")}</h4>
          {recentJobs.length === 0 ? (
            <p className="meta">{t("settings.runnerNoJobs")}</p>
          ) : (
            <div style={{ overflowX: "auto", marginTop: 8 }}>
              <table style={{ width: "100%", borderCollapse: "collapse", minWidth: 780 }}>
                <thead>
                  <tr>
                    {["runnerJob", "runnerStateColumn", "runnerAgent", "runnerOwner", "runnerCreated", "runnerAction"].map((key) => (
                      <th key={key} style={tableCellStyle}>{t(`settings.${key}`)}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {recentJobs.map((job) => (
                    <JobRow
                      key={job.job_id}
                      job={job}
                      busy={admin.actionId === `cancel:${job.job_id}`}
                      onCancel={() => void admin.cancelJob(job.job_id)}
                      t={t}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </section>
  );
}

function Summary({ label, value }: { label: string; value: string }) {
  return <div className="panel"><p className="meta">{label}</p><strong>{value}</strong></div>;
}

function AgentCard({
  agent,
  busy,
  onProbe,
  t,
}: {
  agent: RunnerAgentView;
  busy: boolean;
  onProbe: () => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
}) {
  const labels = Object.entries(agent.labels).map(([key, value]) => `${key}=${value}`).join(" · ");
  return (
    <div className="panel" style={{ borderLeft: `4px solid ${stateColors[agent.state]}` }}>
      <div className="row" style={{ justifyContent: "space-between" }}>
        <strong>{agent.agent_id}</strong>
        <span style={{ color: stateColors[agent.state] }}>{stateLabel(agent.state, t)}</span>
      </div>
      <p className="meta">{isolationLabel(agent.isolation, t)} · v{agent.agent_version}</p>
      <p className="meta">{t("settings.runnerAgentCapacity")}: {agent.available_slots}/{agent.max_concurrent} · {t("settings.runnerActiveJobs")}: {agent.active_jobs}</p>
      <p className="meta">{t("settings.runnerLastSeen")}: {formatDate(agent.last_seen_at)}</p>
      {agent.kernel_release && <p className="meta">Kernel: {agent.kernel_release}</p>}
      <p className="meta">{t("settings.runnerCapabilities")}: {agent.capabilities.join(", ")}</p>
      {labels && <p className="meta">{t("settings.runnerLabels")}: {labels}</p>}
      {agent.message && <p className="meta">{agent.message}</p>}
      <button type="button" onClick={onProbe} disabled={busy || agent.state !== "healthy"}>
        {busy ? t("settings.runnerProbing") : t("settings.runnerProbe")}
      </button>
    </div>
  );
}

function JobRow({
  job,
  busy,
  onCancel,
  t,
}: {
  job: RunnerJobView;
  busy: boolean;
  onCancel: () => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
}) {
  const agent = job.assigned_agent_id || job.target_agent_id || "—";
  const label = job.program_name || (job.kind === "control_probe" ? t("settings.runnerProbeJob") : job.kind);
  return (
    <tr>
      <td style={tableCellStyle}><strong>{label}</strong><div className="meta">{job.job_id.slice(0, 18)}…</div></td>
      <td style={{ ...tableCellStyle, color: stateColors[job.state] }}>{stateLabel(job.state, t)}</td>
      <td style={tableCellStyle}>{agent}</td>
      <td style={tableCellStyle}>{job.owner_username || t("settings.runnerSystemOwner")}</td>
      <td style={tableCellStyle}>{formatDate(job.created_at)}{job.result_message && <div className="meta">{job.result_message}</div>}</td>
      <td style={tableCellStyle}>
        {isCancellable(job.state) ? (
          <button type="button" onClick={onCancel} disabled={busy}>
            {busy ? t("settings.runnerCancelling") : t("settings.runnerCancel")}
          </button>
        ) : "—"}
      </td>
    </tr>
  );
}

const tableCellStyle = { borderBottom: "1px solid #30343b", padding: "8px 6px", textAlign: "left" as const };
const formatDate = (value: string) => new Date(value).toLocaleString();
const isActiveJob = (state: RunnerJobState) => ["queued", "claimed", "cancel_requested"].includes(state);
const isCancellable = (state: RunnerJobState) => ["queued", "claimed"].includes(state);
const stateLabel = (state: RunnerAgentState | RunnerJobState, t: (key: string) => string) => t(`settings.runnerState.${state}`);
const isolationLabel = (isolation: RunnerAgentIsolation, t: (key: string) => string) => t(`settings.runnerIsolation.${isolation}`);
