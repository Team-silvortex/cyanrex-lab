import assert from "node:assert/strict";
import test from "node:test";

import {
  compareOperationSets,
  extractEngineAccess,
  extractEngineOperations,
  extractSdkOperations,
  openApiOperations,
} from "../openapi-contract.mjs";

test("extractEngineOperations handles chained Axum methods and multiline routes", () => {
  const source = `
    Router::new()
      .route("/health", get(routes::health::health))
      .route(
        "/settings/events",
        get(routes::settings::get_event_settings)
          .post(routes::settings::update_event_settings),
      );
  `;

  assert.deepEqual(extractEngineOperations(source), [
    "GET /health",
    "GET /settings/events",
    "POST /settings/events",
  ]);
});

test("extractEngineAccess follows the Axum router trust tiers", () => {
  const source = `
    fn public_routes() -> Router<Arc<AppState>> {
      Router::new().route("/health", get(routes::health::health))
    }
    fn authenticated_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
      Router::new().route("/events", get(routes::events::list_events))
    }
    fn admin_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
      Router::new().route("/command", post(routes::command::dispatch_command))
    }
    fn runner_agent_routes() -> Router<Arc<AppState>> {
      Router::new()
        .route("/runner/agent/register", post(routes::runner_agent::register))
        .route("/runner/agent/heartbeat", post(routes::runner_agent::heartbeat))
    }
  `;

  assert.deepEqual(extractEngineAccess(source), {
    "GET /events": "authenticated",
    "GET /health": "public",
    "POST /command": "admin",
    "POST /runner/agent/heartbeat": "runner-agent-signed",
    "POST /runner/agent/register": "runner-agent-bootstrap",
  });
});

test("extractSdkOperations recognizes generic requests, downloads, and WebSockets", () => {
  const source = `
    this.get<{ status: string }>("/health", undefined, options)
    this.post<ApiMessage>("/events/delete", undefined, options)
    this.download("/events/export", query, options)
    this.websocketUrl("/ws/events")
  `;

  assert.deepEqual(extractSdkOperations(source), [
    "GET /events/export",
    "GET /health",
    "GET /ws/events",
    "POST /events/delete",
  ]);
});

test("OpenAPI operation extraction and comparison report exact drift", () => {
  const document = {
    paths: {
      "/health": { get: { operationId: "getHealth", responses: { 200: {} } } },
      "/command": { post: { operationId: "dispatchCommand", responses: { 200: {} } } },
    },
  };

  assert.deepEqual(openApiOperations(document), ["GET /health", "POST /command"]);
  assert.deepEqual(
    compareOperationSets(["GET /health", "POST /command"], ["GET /health", "GET /missing"]),
    { missing: ["POST /command"], extra: ["GET /missing"] },
  );
});
