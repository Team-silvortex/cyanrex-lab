import { CyanrexClient } from "../src/index.js";
import type { ApiDownload, ModuleInfo } from "../src/index.js";

declare const client: CyanrexClient;
declare const signal: AbortSignal;

async function verifyOperationTypes() {
  const health = await client.operation("getHealth");
  health.status.toUpperCase();

  const module: ModuleInfo = await client.operation("postModulesStart", {
    body: { name: "module-network" },
  });
  module.name.toUpperCase();

  await client.operation("getLearningTeacherAttempts", {
    query: { username: "student", limit: 10 },
  });
  await client.operation("getEvents", { query: { category: "kernel", limit: 20 } });
  await client.operation("getHealth", {}, { signal });

  const download: ApiDownload = await client.operation("getEventsExport", {
    query: { format: "csv" },
  });
  const websocketUrl: string = await client.operation("getWsEvents");
  void download;
  void websocketUrl;

  // @ts-expect-error request body is required by the OpenAPI operation
  await client.operation("postModulesStart");
  // @ts-expect-error username is a required query parameter
  await client.operation("getLearningTeacherAttempts", { query: {} });
  // @ts-expect-error generated enum excludes unknown event categories
  await client.operation("getEvents", { query: { category: "userspace" } });
  // @ts-expect-error signed Runner Agent operations are intentionally not browser SDK operations
  await client.operation("postRunnerAgentHeartbeat", {});
}

void verifyOperationTypes;
