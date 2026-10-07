import { PRIVATE_READ_TIMEOUT_MS, PRIVATE_WRITE_TIMEOUT_MS, requestPrivate } from "../../transport/privateRequest";

export const AI_AGENT_PROTOCOLS = ["openai_responses", "openai_chat_completions", "anthropic_messages", "gemini_generate_content", "custom"] as const;
export const AI_AGENT_READ_TIMEOUT_MS = PRIVATE_READ_TIMEOUT_MS;
export const AI_AGENT_SAVE_TIMEOUT_MS = PRIVATE_WRITE_TIMEOUT_MS;
export const MAX_AI_AGENT_PROFILES = 16;
export type AiAgentProtocol = typeof AI_AGENT_PROTOCOLS[number];
export type AiAgentProfile = {
  id: string; name: string; protocol: AiAgentProtocol; base_url: string; model: string;
  credential_ref: string | null; enabled: boolean;
};
export type AiAgentSettings = { revision: number; default_profile_id: string | null; profiles: AiAgentProfile[] };
export type AiAgentSettingsUpdate = Omit<AiAgentSettings, "revision"> & { expected_revision: number };

function invalid(): never { throw new Error("Invalid AI Agent configuration"); }
function record(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return invalid();
  if (Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value, key))) return invalid();
  return value as Record<string, unknown>;
}
function text(value: unknown, max: number): string {
  if (typeof value !== "string" || !value.trim() || [...value].length > max || /[\u0000-\u001f\u007f-\u009f]/u.test(value)
    || [...value].some(char => { const code = char.codePointAt(0)!; return code >= 0xd800 && code <= 0xdfff; })) return invalid();
  return value;
}
function symbol(value: unknown, pattern: RegExp): string {
  if (typeof value !== "string" || !pattern.test(value)) return invalid();
  return value;
}
function endpoint(value: unknown): string {
  const raw = text(value, 2048);
  if (!/^https?:\/\//i.test(raw) || /[\s?#\\]/u.test(raw) || raw.split("/")[2]?.includes("@")) return invalid();
  let url: URL;
  try { url = new URL(raw); } catch { return invalid(); }
  const authority = raw.split("/")[2], host = authority.match(/^(\[[^\]]+\]|[^:]+)(?::[0-9]+)?$/)?.[1].toLowerCase();
  if (!host || !url.hostname || url.hostname.includes("*") || url.username || url.password || url.search || url.hash
    || (url.protocol !== "https:" && !(url.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(host ?? "")))) return invalid();
  return raw;
}
function profile(value: unknown): AiAgentProfile {
  const item = record(value, ["id", "name", "protocol", "base_url", "model", "credential_ref", "enabled"]);
  if (!AI_AGENT_PROTOCOLS.includes(item.protocol as AiAgentProtocol) || typeof item.enabled !== "boolean") return invalid();
  return { id: symbol(item.id, /^[a-z][a-z0-9_-]{0,63}$/), name: text(item.name, 128),
    protocol: item.protocol as AiAgentProtocol, base_url: endpoint(item.base_url), model: text(item.model, 128),
    credential_ref: item.credential_ref === null ? null : symbol(item.credential_ref, /^[A-Z][A-Z0-9_]{0,63}$/), enabled: item.enabled };
}

export function parseAiAgentSettings(value: unknown): AiAgentSettings {
  const data = record(value, ["revision", "default_profile_id", "profiles"]);
  if (!Number.isSafeInteger(data.revision) || (data.revision as number) < 0 || (data.revision as number) > 0xffffffff
    || !Array.isArray(data.profiles) || data.profiles.length > MAX_AI_AGENT_PROFILES) return invalid();
  const profiles = data.profiles.map(profile), ids = new Set(profiles.map(item => item.id));
  if (ids.size !== profiles.length || (data.default_profile_id !== null
    && !profiles.some(item => item.id === data.default_profile_id && item.enabled))) return invalid();
  return { revision: data.revision as number, default_profile_id: data.default_profile_id as string | null, profiles };
}

// Exact copies preserve the reviewed text. Normalization must never hide an acknowledgement mismatch.
export function reviewAiAgentSettings(value: unknown): AiAgentSettingsUpdate {
  const { revision, ...draft } = parseAiAgentSettings(value);
  if (revision === 0xffffffff) return invalid();
  const body = { expected_revision: revision, ...draft };
  if (new TextEncoder().encode(JSON.stringify(body)).length > 32 * 1024) return invalid();
  return body;
}

export function parseAiAgentSettingsSaved(value: unknown, expected: AiAgentSettingsUpdate): AiAgentSettings {
  const ack = record(value, ["ok", "settings"]), settings = parseAiAgentSettings(ack.settings);
  if (ack.ok !== true || settings.revision !== expected.expected_revision + 1
    || settings.default_profile_id !== expected.default_profile_id
    || JSON.stringify(settings.profiles) !== JSON.stringify(expected.profiles)) return invalid();
  return settings;
}

export class AiAgentSettingsConflict extends Error {
  constructor() { super("AI Agent configuration changed; reload before saving"); this.name = "AiAgentSettingsConflict"; }
}

export function requestAiAgentSettings(url: string, parent: AbortSignal, input?: AiAgentSettingsUpdate): Promise<AiAgentSettings> {
  // Keep acknowledgement expectations private even if a caller subsequently edits its input object.
  const body = input === undefined ? undefined : reviewAiAgentSettings({
    revision: input.expected_revision, default_profile_id: input.default_profile_id, profiles: input.profiles,
  });
  return requestPrivate(url, parent, async (response, signal) => {
    if (!response.ok || response.headers.get("content-type")?.split(";")[0].trim().toLowerCase() !== "application/json") {
      void response.body?.cancel().catch(() => undefined);
      if (response.status === 409) throw new AiAgentSettingsConflict();
      throw new Error("AI Agent settings request failed");
    }
    const value: unknown = await response.json(); signal.throwIfAborted();
    return body === undefined ? parseAiAgentSettings(value) : parseAiAgentSettingsSaved(value, body);
  }, { body, timeoutMs: body === undefined ? AI_AGENT_READ_TIMEOUT_MS : AI_AGENT_SAVE_TIMEOUT_MS,
    timeoutMessage: "AI Agent settings request timed out" });
}
