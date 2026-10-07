import type { AiAgentProfile, AiAgentSettings } from "../types.js";
import { bytes, exact, invalid, jsonCopy } from "./json.js";

const protocols = new Set(["openai_responses", "openai_chat_completions", "anthropic_messages", "gemini_generate_content", "custom"]);
function text(value: unknown, limit: number): value is string {
  return typeof value === "string" && value.trim().length > 0 && [...value].length <= limit
    && !/[\u0000-\u001f\u007f-\u009f\uD800-\uDFFF]/u.test(value);
}
export function validateAiAgentProfile(value: unknown): AiAgentProfile {
  const item = exact(jsonCopy(value), ["id", "name", "protocol", "base_url", "model", "credential_ref", "enabled"]);
  if (typeof item.id !== "string" || !/^[a-z][a-z0-9_-]{0,63}$/.test(item.id)
    || !text(item.name, 128) || !text(item.model, 128) || typeof item.protocol !== "string" || !protocols.has(item.protocol)
    || typeof item.enabled !== "boolean" || !text(item.base_url, 2048)
    || item.credential_ref !== null && (typeof item.credential_ref !== "string"
      || !/^[A-Z][A-Z0-9_]{0,63}$/.test(item.credential_ref))) invalid();
  let url: URL;
  try { url = new URL(item.base_url); } catch { return invalid(); }
  const authority = /^https?:\/\/([^/]+)/i.exec(item.base_url)?.[1];
  const loopback = authority !== undefined && /^(localhost|127\.0\.0\.1|\[::1\])(?::\d+)?$/i.test(authority);
  if (url.username || url.password || url.search || url.hash || item.base_url !== item.base_url.trim()
    || !authority || !/^(?:\[[^\]]+\]|[^:]+)(?::[0-9]+)?$/u.test(authority)
    || authority.includes("@") || /[\s\\?#]/u.test(item.base_url) || url.hostname.includes("*")
    || !(url.protocol === "https:" || url.protocol === "http:" && loopback)) invalid();
  return item as unknown as AiAgentProfile;
}

export function validateAiAgentSettings(value: unknown): AiAgentSettings {
  const item = exact(jsonCopy(value, 32 * 1024), ["revision", "default_profile_id", "profiles"]);
  if (!Number.isInteger(item.revision) || Number(item.revision) < 0 || Number(item.revision) > 0xffff_ffff
    || !Array.isArray(item.profiles) || item.profiles.length > 16) invalid();
  const profiles = item.profiles.map(validateAiAgentProfile);
  if (new Set(profiles.map(profile => profile.id)).size !== profiles.length) invalid();
  if (item.default_profile_id !== null && !profiles.some(profile => profile.id === item.default_profile_id && profile.enabled)) invalid();
  return { revision: Number(item.revision), default_profile_id: item.default_profile_id as string | null, profiles };
}

/** Host-only credential resolution; no environment reads, storage or model HTTP requests. */
export async function resolveAgentCredential(
  profile: AiAgentProfile,
  resolve: (reference: string) => string | Promise<string>,
): Promise<string | null> {
  const safe = validateAiAgentProfile(profile);
  if (!safe.enabled) throw new TypeError("AI Agent profile is disabled");
  if (safe.credential_ref === null) return null;
  const credential = await resolve(safe.credential_ref);
  if (typeof credential !== "string" || !credential.length || bytes(credential) > 8192) {
    throw new TypeError("AI Agent credential unavailable");
  }
  return credential;
}
