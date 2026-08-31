# @cyanrex/sdk-js

Typed browser and Node.js client for the Cyanrex Lab Engine API.

## Use

```ts
import { CyanrexApiError, CyanrexClient } from "@cyanrex/sdk-js";

const cyanrex = new CyanrexClient("http://localhost:8080");

await cyanrex.auth.login({
  username: "admin",
  password: "your-password",
  otp: "123456",
});

const modules = await cyanrex.modules.list();
const check = await cyanrex.ebpf.check("int main(void) { return 0; }");

// The generated operationId surface stays synchronized with OpenAPI.
const health = await cyanrex.operation("getHealth");
await cyanrex.operation("postModulesStart", {
  body: { name: "module-network" },
});

try {
  await cyanrex.command.dispatch({ commandType: "StartModule", moduleName: "module-network" });
} catch (error) {
  if (error instanceof CyanrexApiError) {
    console.error(error.status, error.message, error.details);
  }
}
```

The client covers authentication, modules and the admin command bus, events, settings, scripts,
learning, eBPF checks/runs/attachments, Runner status/admin jobs, and environment diagnostics. Every
method accepts an optional `{ signal }` argument for cancellation. The generated `operation()` layer
covers all 56 browser-facing operations and derives required bodies, query parameters, and response
types from each OpenAPI operation. The five signed Runner Agent protocol operations remain isolated
from the browser SDK. `client.request<T>()` remains available for forward-compatible calls.

The generated dispatcher also preserves non-JSON behavior: `getEventsExport` returns `ApiDownload`,
and `getWsEvents` resolves to the authenticated WebSocket URL. Existing namespaces remain the stable,
task-oriented facade; the operationId surface is additive.

Public request and response models are generated from the Engine OpenAPI component schemas. The
hand-designed client namespaces remain stable while the quality gate rejects stale generated types.
The complete generated map also has an explicit type subpath:

```ts
import type { OpenApiSchemas } from "@cyanrex/sdk-js/openapi";

type RunnerJob = OpenApiSchemas["RunnerJobView"];
```

Operation metadata is available without constructing a client:

```ts
import { openApiOperations } from "@cyanrex/sdk-js/operations";
import type { OpenApiOperationInput } from "@cyanrex/sdk-js/operations";

type EventInput = OpenApiOperationInput<"getEvents">;
console.log(openApiOperations.getEvents.access); // authenticated
```

## Compatibility Baseline

The committed `0.3.0` OpenAPI baseline is checked before SDK builds. Existing inputs must remain
accepted, successful responses must retain their previous guarantees, and access-tier changes
require an explicit compatibility reset. Additive operations and optional input/output growth pass.

Run `npm run check:compatibility` to inspect a candidate contract. Replacing the baseline with
`node ../scripts/api-compatibility.mjs --write-baseline` is reserved for an intentionally reviewed
breaking release; routine generation and quality checks never rewrite it.

## Sessions and CSRF

Browser requests use `credentials: "include"`; the browser owns the `HttpOnly` session cookie and
adds the request `Origin`. For Node.js, the client captures the `cyanrex_session` cookie returned by
login and sends it on later calls. Configure the frontend origin allowed by the Engine so unsafe
requests satisfy its Origin-based CSRF guard:

```ts
const cyanrex = new CyanrexClient("http://localhost:8080", {
  csrfOrigin: "http://localhost:3000",
});
```

Use `getSessionCookie()`, `setSessionCookie()` and a custom `fetch` implementation when integrating
with an external cookie jar. Never expose a Node-managed session cookie to browser JavaScript.

## Development

Requires Node.js 18 or later.

```bash
npm ci
npm run generate # after intentionally changing OpenAPI schemas or operations
npm run check:compatibility
npm run check
npm run test:package
```

`npm run build` emits ESM JavaScript, declarations, declaration maps, and source maps under `dist/`.
`npm run test:package` validates the dry-run npm manifest, relative declaration/runtime imports, and
consumer-style loading of both package exports. The package remains private until a compatibility
and release policy is approved.
