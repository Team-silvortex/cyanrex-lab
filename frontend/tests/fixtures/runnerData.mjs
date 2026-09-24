// Synthetic DTOs mirror the Engine wire format, not real Agent identities or jobs.
export const stamp = "2026-09-13T00:00:01Z";
export const runnerAgent = (changes = {}) => ({
  agent_id: "fixture-agent", protocol_version: 1, agent_version: "0.4.1", isolation: "virtual_machine",
  state: "healthy", max_concurrent: 2, active_jobs: 0, available_slots: 2,
  capabilities: ["clang_check"], labels: { room: "fixture" }, kernel_release: null, message: null,
  registered_at: stamp, last_seen_at: stamp, expires_at: "2026-09-13T00:01:01Z", ...changes,
});
export const runnerJob = (changes = {}) => ({
  job_id: "fixture-job", kind: "ebpf_compile_check", state: "queued", owner_username: "student",
  target_agent_id: "fixture-agent", assigned_agent_id: null, message: "compile check: fixture",
  source_bytes: 20, program_name: "fixture", timeout_seconds: 20, result_message: null,
  created_at: stamp, claimed_at: null, deadline: null, completed_at: null, output: null, ...changes,
});
export const agentInventory = (agents = [runnerAgent()], changes = {}) => ({
  generated_at: stamp, enabled: true, total_agents: agents.length,
  online_agents: agents.filter(agent => agent.state !== "offline").length, agents, ...changes,
});
export const jobInventory = (jobs = [runnerJob()], changes = {}) => ({ generated_at: stamp, total_jobs: jobs.length, jobs, ...changes });
