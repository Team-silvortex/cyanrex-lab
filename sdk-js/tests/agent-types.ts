import { CyanrexClient, type AiAgentSettings } from "../src/index.js";
import { createAgentBridge, normalizeAgentCalls, validateAiAgentProfile, type AgentToolOperationName } from "../src/agents/index.js";

const client = new CyanrexClient("http://localhost:8080");
const settings: AiAgentSettings = await client.aiAgents.settings();
await client.aiAgents.updateSettings({ expected_revision: settings.revision, profiles: settings.profiles, default_profile_id: settings.default_profile_id });
const operation: AgentToolOperationName = "getHealth";
const bridge = createAgentBridge(client, { operations: [operation] });
await bridge.executeCalls("custom", [{ id: "one", name: operation, arguments: {} }]);
bridge.tools("anthropic_messages");
normalizeAgentCalls("openai_responses", []);
validateAiAgentProfile({}); // untrusted input is runtime validated
// @ts-expect-error no default complete-platform toolset
createAgentBridge(client);
// @ts-expect-error auth is not a model tool
createAgentBridge(client, { operations: ["postAuthLogin"] });
// @ts-expect-error real kernel execution is never a model tool
createAgentBridge(client, { operations: ["postEbpfRun"] });
// @ts-expect-error provider protocol is explicit
bridge.tools("unknown");
// @ts-expect-error revision required for configuration writes
await client.aiAgents.updateSettings({ profiles: [], default_profile_id: null });
