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

The client covers authentication, modules and the teacher management command bus, events, settings, scripts,
learning, eBPF checks/runs/attachments, Runner status/management jobs, and environment diagnostics. Every
method accepts an optional `{ signal }` argument for cancellation. The generated `operation()` layer
covers all 63 browser-facing operations and derives required bodies, query parameters, and response
types from each OpenAPI operation. The five signed Runner Agent protocol operations remain isolated
from the browser SDK. `client.request<T>()` remains available for forward-compatible calls.

The generated dispatcher also preserves non-JSON behavior: `getEventsExport` returns `ApiDownload`,
and `getWsEvents` resolves to the authenticated WebSocket URL. Existing namespaces remain the stable,
task-oriented facade; the operationId surface is additive.

Event sockets require the session cookie and an allowed Origin/Referer, including for native clients;
the URL helper does not attach those headers or manage reconnects. Frames remain raw Event records.
Code `1013` means possible broadcast lag: reconnect with backoff and refresh retained `/events` history.
Slow sends can instead end abnormally. Complete replay is not guaranteed; see
[Event Stream Recovery](../docs/en/event-stream.md) for migration and recovery limits.

The additive-only namespace baseline and deprecation window are defined in [STABILITY.md](STABILITY.md).
The quality gate rejects removal or renaming of any captured public client path.

Classroom entry uses `classroom.discovery()`, `invitations()`, `invite({ username })`, `revoke(inviteId)`
and `join(request)`, plus generated operation IDs. These requests require HTTPS except on loopback,
disable caching/redirects and retain CSRF for writes. Independently confirm the teacher's exact origin;
discovery metadata and compatible product patches do not prove identity. Invitations are name-bound,
single-use and valid for 10 minutes; join creates only a student account, returns TOTP setup once and
does not log in. Never log the private link or auto-retry a consumed/uncertain invitation. Revoking one
does not revoke existing accounts/sessions. See [Classroom Connection](../docs/en/classroom-connection.md).

Teachers (including compatible legacy administrators) can leave student-visible feedback on existing lab attempts:

```ts
const { attempts } = await cyanrex.learning.teacherAttempts("student");
const attempt = attempts[0];
if (attempt) {
  await cyanrex.learning.saveTeacherFeedback({
    username: attempt.username,
    attempt_id: attempt.id,
    comment: "Explain why the pointer is safe before dereferencing it.",
    expected_revision: attempt.teacher_feedback?.revision ?? 0,
  });
}
```

Comments are trimmed plain text (1–2000 Unicode characters). The server assigns the reviewer and time;
students read them through `learning.attempts()`. A stale revision returns `CyanrexApiError` with status
`409`: reload, compare, and explicitly resubmit instead of automatically retrying an overwrite.
Feedback never modifies automated acceptance. Node clients must configure `csrfOrigin` as above.

Read one of the current session user's submissions with `cyanrex.learning.attempt(attemptId, { signal })`
or `cyanrex.operation("getLearningAttempt", { query: { attempt_id: attemptId } }, { signal })`. It returns
the original source, lab/template context and current feedback without updating the attempt or executing
code. There is no username override, including for staff; missing/other-owner attempts return `404`,
invalid IDs `400`, and record-storage failures `500`. The response is marked `Cache-Control: no-store`.

Teachers now own teaching and deployment management, including the seeded personal-use account.
Login/session responses report `teacher` for legacy administrator identities; the `admin` role remains
in the schema for compatibility. Operation access labels `admin` and `staff` are stable historical
categories, both requiring teacher authority; OpenAPI's `x-cyanrex-roles` records this explicitly.
Existing namespaces, operation IDs and request/response shapes remain compatible. Clients must not
hide deployment controls just because the session is `teacher` instead of `admin`.

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

Run `npm run check:surface` to inspect the hand-designed client namespace baseline. Its baseline writer
is reserved for accepted additions or a separately reviewed breaking release, as described by the
stability policy.

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
