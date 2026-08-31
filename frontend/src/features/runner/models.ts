export type RunnerAgentState = "healthy" | "degraded" | "draining" | "offline";
export type RunnerAgentIsolation =
  | "shared_kernel"
  | "container"
  | "virtual_machine"
  | "dedicated_host";

export type RunnerAgentView = {
  agent_id: string;
  protocol_version: number;
  agent_version: string;
  isolation: RunnerAgentIsolation;
  state: RunnerAgentState;
  max_concurrent: number;
  active_jobs: number;
  available_slots: number;
  capabilities: string[];
  labels: Record<string, string>;
  kernel_release?: string | null;
  message?: string | null;
  registered_at: string;
  last_seen_at: string;
  expires_at: string;
};

export type RunnerAgentInventory = {
  generated_at: string;
  enabled: boolean;
  total_agents: number;
  online_agents: number;
  agents: RunnerAgentView[];
};

export type RunnerJobState =
  | "queued"
  | "claimed"
  | "cancel_requested"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "expired";

export type RunnerJobView = {
  job_id: string;
  kind: string;
  state: RunnerJobState;
  target_agent_id?: string | null;
  assigned_agent_id?: string | null;
  owner_username?: string | null;
  message: string;
  source_bytes?: number | null;
  program_name?: string | null;
  timeout_seconds: number;
  result_message?: string | null;
  created_at: string;
  claimed_at?: string | null;
  deadline?: string | null;
  completed_at?: string | null;
};

export type RunnerJobInventory = {
  generated_at: string;
  total_jobs: number;
  jobs: RunnerJobView[];
};

export type RunnerAdminNotice = {
  kind: "probe_submitted" | "cancel_requested";
  subject: string;
};
