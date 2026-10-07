# AI Agent SDK helpers and connection profiles

Included in source release 0.5.3. Decision date: 2026-10-07. This additive layer targets AI Agent hosts, not signed Linux Runner Agents.
It does not implement autonomous participants, delegation, model inference or general Task execution.

## Connection settings

Teachers open **Settings → AI Agents** to manage IDs, display names, protocols, service URLs, models,
credential references, enabled profiles and an optional enabled default. Protocols are OpenAI Responses,
OpenAI-compatible Chat Completions, Anthropic Messages, Gemini GenerateContent and custom. Model names
are supplied by the user. Selecting a protocol does not test compatibility or start a connection.

Enter a symbolic reference such as `OPENAI_API_KEY`, never the API key itself. The browser has no secret
input/storage; the Engine never reads environment secrets, resolves references or contacts providers.
The separately trusted Agent host owns secret resolution and model transport. Never forward Engine
cookies or CSRF headers to a model provider. An enabled profile is not permission to execute tools.

Teacher-only `GET /settings/ai-agents` reads the list; `POST` replaces it using
`{ expected_revision, default_profile_id, profiles }`. Writes retain CSRF checks. All responses are
private/no-store. Confirmed success is `{ ok: true, settings }` with revision exactly incremented once.
SDK entry points are `client.aiAgents.settings()` and `client.aiAgents.updateSettings(request)`.
The UI verifies a read and confirms the exact replacement. In a surviving page, a failed/unconfirmed
save preserves a read-only draft until explicit reload; reload failure cannot erase it. Cancelling
before confirmation does not write or lock the draft. Navigation, Engine switching and unmount discard
in-memory drafts and ignore late responses; there is no persistent recovery. A lost response is not
rollback. Do not automatically retry or accept an acknowledgement with changed profile contents.

Limits: 16 unique profiles; IDs `[a-z][a-z0-9_-]{0,63}`; nonblank/control-free names and models of 1–128
Unicode scalar characters; URLs up to 2048 characters. URLs require absolute HTTPS, or HTTP on literal
`localhost`, `127.0.0.1` or `[::1]`, without user information, whitespace, query or fragment. Alternative
IP spellings cannot become HTTP loopback through URL normalization. Credential references are null or
`[A-Z][A-Z0-9_]{0,63}`. A default must be enabled and present. Revision is u32; overflow is refused.
The independent JSON request limit is 32 KiB. Unknown, duplicate and API-key fields are rejected.

Metadata persists at `CYANREX_DATA_DIR/ai-agents/<runtime_instance_id>/settings.json`. Construction and
absent-file reads do not write defaults. Corrupt/unsafe/unreadable storage fails closed. Same-directory
atomic replacement and shared clone admission preserve ordering after a dispatched caller is cancelled.
Independently constructed stores or processes do not share that gate. Back up this metadata with the
data directory; it is not a secret vault, migration or cross-process coordination service.
The backend requires Unix; other platforms return storage-unavailable (503). The data root defaults
to `./data`; the instance name is the sanitized `CYANREX_INSTANCE_ID` (default `default`). The
`ai-agents` and instance directories must belong to the service user and be `0700`; the snapshot must
be a regular single-link `0600` file. Paths cannot traverse symlinks. Backups/restores must preserve
ownership, permissions and instance naming. Existing shared parent directories are never chmodded.

## Explicit tool bridge

```ts
import { CyanrexClient } from "@cyanrex/sdk-js";
import { createAgentBridge, formatAgentResults } from "@cyanrex/sdk-js/agents";

const client = new CyanrexClient("https://engine.example", {
  csrfOrigin: "https://teacher.example",
}); // The host separately owns its Engine session and provider transport.
const bridge = createAgentBridge(client, { operations: ["getHealth", "getLearningAttempts"] });
const tools = bridge.tools("openai_responses");
// Send tools through your provider client; pass only completed function-call records here.
const results = await bridge.executeCalls("openai_responses", functionCallRecords, { signal });
const outputs = formatAgentResults("openai_responses", results);
// The host sends outputs back to its conversation; the bridge does not call the provider.
```

The caller must select a finite tool list. The reviewed catalogue contains 28 existing JSON operations
(23 reads, 5 writes/diagnostics), not every SDK operation. Auth, classroom invitations, all settings,
management commands, kernel run/detach, module mutations and Runner job dispatch are excluded. Prepared
Task/Artifact/Review commands have no public SDK routes. Existing server roles/owner checks still apply.

A selected mutation requires both `allowMutations: true` and a trusted
`approveMutation({ call, operationName })` callback, once per call, with a frozen reviewed target.
Model-supplied `approved: true` cannot authorize anything. Diagnostics follow the same approval rule.
Use an actual user confirmation or reviewed host policy, not an unconditional approval callback.

The entire batch is validated before dispatch, then executed serially. A batch is not a transaction:
denial or dispatch failure of one item does not prevent subsequent items; inspect every result.
Unknown tools/arguments and
prototype-sensitive keys fail closed. Maximums: 32 unique calls/batch, 64 KiB arguments, 1024 consumed
IDs/bridge, 1 MiB result JSON copy per call (argument limits are also per call). Consumed IDs include denied/uncertain calls and cannot be retried
on that bridge. This ledger is not durable deduplication across new bridges/restarts. Dispatch failure
returns generic unconfirmed errors, not private exception details or rollback evidence. Never blindly
repeat a write with a new ID. Cancellation is cooperative with the host transport/approval callback;
there is no added total deadline or HTTP stream memory bound. The result limit applies after return.
An abort-ignoring late success is not acknowledged; remaining calls are not dispatched after abort.

Formats: `openai_responses`, `openai_chat_completions`, `anthropic_messages`,
`gemini_generate_content`, `mcp`, `custom`. OpenAI uses explicit `strict: false` to preserve optional
inputs. Only complete nonstreaming records are normalized. Gemini/MCP require a stable string call ID;
if absent/numeric, the host must explicitly correlate `{ id, name, arguments }` and use `custom`.
JSON-RPC envelopes, Gemini thought signatures and conversation metadata remain host-owned. These are
format helpers, not a provider client, MCP server or framework workflow engine.

Exports also include `validateAiAgentProfile`, `validateAiAgentSettings` and host-only
`resolveAgentCredential(profile, resolver)`. Resolution calls only the supplied callback. Never send
its result to a model/log. Resolve only host-allowlisted references, not arbitrary environment names
from a profile. Tool results may contain private code, feedback or operational data;
the host owns consent, redaction and disclosure policy.

## Verification and references

Run `ai_agent_settings_tdd`, frontend `test:ai-agents` and explicit `test:ai-agents-browser`, SDK
`npm run check`, and common `agentToolGenerator`/`aiAgentSettingsContract` regressions. Synthetic records
and browser fixtures do not prove a real provider connection; see dated [project status](project-status.md).

Protocol references: [OpenAI function calling](https://developers.openai.com/api/docs/guides/function-calling),
[Anthropic tools](https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview),
[Gemini FunctionDeclaration](https://ai.google.dev/api/generate-content#FunctionDeclaration),
[MCP tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools).
