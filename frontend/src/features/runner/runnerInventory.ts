import type { RunnerAgentInventory, RunnerAgentView, RunnerJobInventory, RunnerJobView } from "./models";

const invalid = () => new Error("Invalid Runner response");
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw invalid();
  return value as Record<string, unknown>;
}
function text(value: unknown, max = 512): string {
  if (typeof value !== "string" || value.length > max) throw invalid();
  return value;
}
function identity(value: unknown): string {
  const result = text(value, 128);
  if (!/^[A-Za-z0-9_.-]+$/.test(result)) throw invalid();
  return result;
}
function integer(value: unknown, max = Number.MAX_SAFE_INTEGER, min = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max) throw invalid();
  return value;
}
function date(value: unknown): string {
  const result = text(value, 64);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(result)
    || !Number.isFinite(Date.parse(result))) throw invalid();
  const year = Number(result.slice(0, 4)), month = Number(result.slice(5, 7)), day = Number(result.slice(8, 10));
  const leap = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
  if (month < 1 || month > 12 || day < 1 || day > days[month - 1] || Number(result.slice(11, 13)) > 23) throw invalid();
  return result;
}
const optional = <T>(value: unknown, parse: (value: unknown) => T): T | null => value == null ? null : parse(value);
function choice<T extends string>(value: unknown, choices: readonly T[]): T {
  if (!choices.includes(value as T)) throw invalid();
  return value as T;
}
function list<T>(value: unknown, parse: (value: unknown) => T, max: number): T[] {
  if (!Array.isArray(value) || value.length > max) throw invalid();
  return value.map(parse);
}

function agent(value: unknown): RunnerAgentView {
  const v = record(value);
  const max_concurrent = integer(v.max_concurrent, 32, 1), active_jobs = integer(v.active_jobs, max_concurrent),
    available_slots = integer(v.available_slots, max_concurrent);
  if (active_jobs + available_slots > max_concurrent) throw invalid();
  const entries = Object.entries(record(v.labels));
  if (entries.length > 16) throw invalid();
  const labels = Object.fromEntries(entries.map(([key, value]) => [text(key, 32), text(value, 64)]));
  return {
    agent_id: identity(v.agent_id), protocol_version: integer(v.protocol_version, 65535, 1), agent_version: text(v.agent_version, 32),
    isolation: choice(v.isolation, ["shared_kernel", "container", "virtual_machine", "dedicated_host"]),
    state: choice(v.state, ["healthy", "degraded", "draining", "offline"]), max_concurrent, active_jobs, available_slots,
    capabilities: list(v.capabilities, value => text(value, 32), 32), labels,
    kernel_release: optional(v.kernel_release, text), message: optional(v.message, text),
    registered_at: date(v.registered_at), last_seen_at: date(v.last_seen_at), expires_at: date(v.expires_at),
  };
}
export function parseAgentInventory(value: unknown): RunnerAgentInventory {
  const v = record(value), agents = list(v.agents, agent, 256);
  if (typeof v.enabled !== "boolean" || integer(v.total_agents) !== agents.length
    || integer(v.online_agents) !== agents.filter(agent => agent.state !== "offline").length
    || new Set(agents.map(agent => agent.agent_id)).size !== agents.length) throw invalid();
  return { generated_at: date(v.generated_at), enabled: v.enabled, total_agents: agents.length, online_agents: v.online_agents as number, agents };
}
function job(value: unknown): RunnerJobView {
  const v = record(value);
  return {
    job_id: identity(v.job_id), kind: identity(v.kind),
    state: choice(v.state, ["queued", "claimed", "cancel_requested", "succeeded", "failed", "cancelled", "expired"]),
    target_agent_id: optional(v.target_agent_id, identity), assigned_agent_id: optional(v.assigned_agent_id, identity),
    owner_username: optional(v.owner_username, text), message: text(v.message), source_bytes: optional(v.source_bytes, integer),
    program_name: optional(v.program_name, text), timeout_seconds: integer(v.timeout_seconds, Number.MAX_SAFE_INTEGER, 1),
    result_message: optional(v.result_message, text), created_at: date(v.created_at),
    claimed_at: optional(v.claimed_at, date), deadline: optional(v.deadline, date), completed_at: optional(v.completed_at, date),
  };
}
export function parseJobInventory(value: unknown): RunnerJobInventory {
  const v = record(value), jobs = list(v.jobs, job, 512);
  if (integer(v.total_jobs) !== jobs.length || new Set(jobs.map(job => job.job_id)).size !== jobs.length) throw invalid();
  return { generated_at: date(v.generated_at), total_jobs: jobs.length, jobs };
}
export function parseProbeAcknowledgement(value: unknown, agentId: string): RunnerJobView {
  const result = job(value);
  if (result.kind !== "control_probe" || result.target_agent_id !== agentId || result.state !== "queued"
    || result.message !== "teacher health probe" || result.timeout_seconds !== 30 || result.owner_username != null
    || result.assigned_agent_id != null) throw invalid();
  return result;
}
export function parseCancelAcknowledgement(value: unknown, expected: RunnerJobView): RunnerJobView {
  const result = job(value);
  if (!["cancelled", "cancel_requested"].includes(result.state)
    || cancelIdentity(result) !== cancelIdentity(expected)) throw invalid();
  return result;
}
// Heartbeats and claim/cancel transitions may advance after the confirmation was reviewed.
export const cancelIdentity = (job: RunnerJobView) => JSON.stringify([
  job.job_id, job.kind, job.owner_username ?? null, job.target_agent_id ?? null, job.program_name ?? null, job.created_at,
]);
export const probeIdentity = (agent: RunnerAgentView) => JSON.stringify([
  agent.agent_id, agent.registered_at, agent.protocol_version, agent.agent_version, agent.isolation, agent.max_concurrent,
]);
