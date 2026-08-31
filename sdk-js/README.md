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
method accepts an optional `{ signal }` argument for cancellation. `client.request<T>()` remains
available for forward-compatible calls to newer Engine endpoints.

Public request and response models are generated from the Engine OpenAPI component schemas. The
hand-designed client namespaces remain stable while the quality gate rejects stale generated types.
The complete generated map also has an explicit type subpath:

```ts
import type { OpenApiSchemas } from "@cyanrex/sdk-js/openapi";

type RunnerJob = OpenApiSchemas["RunnerJobView"];
```

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

Requires Node.js 22 or later.

```bash
npm ci
npm run generate:types # only after intentionally changing the OpenAPI schemas
npm run check
npm run test:package
```

`npm run build` emits ESM JavaScript, declarations, declaration maps, and source maps under `dist/`.
`npm run test:package` validates the dry-run npm manifest, relative declaration/runtime imports, and
consumer-style loading of both package exports. The package remains private until a compatibility
and release policy is approved.
