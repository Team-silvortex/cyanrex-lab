const string = (extra = {}) => ({ type: "string", ...extra });
const nullable = schema => ({ anyOf: [schema, { type: "null" }] });
const ref = name => ({ $ref: `#/components/schemas/${name}` });
const object = properties => ({ type: "object", properties,
  required: Object.keys(properties), additionalProperties: false });
const identifier = string({ minLength: 1, maxLength: 64, pattern: "^[a-z][a-z0-9_-]{0,63}$" });
const profiles = { type: "array", maxItems: 16, items: ref("AiAgentProfile") };
export const aiAgentSchemas = {
  AiAgentProtocol: string({ enum: ["openai_responses", "openai_chat_completions",
    "anthropic_messages", "gemini_generate_content", "custom"] }),
  AiAgentProfile: object({
    id: identifier,
    name: string({ minLength: 1, maxLength: 128 }),
    protocol: ref("AiAgentProtocol"),
    base_url: string({ minLength: 1, maxLength: 2048, format: "uri",
      description: "Metadata only: absolute HTTPS, or HTTP on literal loopback/localhost; no URL credentials, query or fragment. The Engine never contacts this URL." }),
    model: string({ minLength: 1, maxLength: 128 }),
    credential_ref: nullable(string({ minLength: 1, maxLength: 64, pattern: "^[A-Z][A-Z0-9_]{0,63}$",
      description: "Symbolic host-side secret reference, never an API key. This service does not resolve it or inspect environment secrets." })),
    enabled: { type: "boolean" },
  }),
  AiAgentSettings: object({
    revision: { type: "integer", minimum: 0, maximum: 4294967295 },
    default_profile_id: nullable(identifier), profiles,
  }),
  UpdateAiAgentSettingsRequest: object({
    expected_revision: { type: "integer", minimum: 0, maximum: 4294967295 },
    default_profile_id: nullable(identifier), profiles,
  }),
  UpdateAiAgentSettingsResponse: object({ ok: { type: "boolean", const: true }, settings: ref("AiAgentSettings") }),
};
