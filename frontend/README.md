# cyanrex frontend

Next.js control plane for Cyanrex eBPF experiments.

Pages compose shared components and feature controllers; runtime endpoints live under `src/config/`,
and privileged behavior remains in the Engine. See the [system architecture](../docs/en/architecture.md)
for the complete frontend/Engine boundary.

## Implemented pages

- `/dashboard`: service overview and quick actions
- `/ebpf`: Monaco editor, templates, script persistence, runtime controls, and attachment management
- `/helper`: engine and eBPF environment diagnostics
- `/modules`: module catalog and lifecycle controls
- `/events`: event filters, export, deletion, and realtime updates
- `/settings`: per-user event retention settings
- `/terminal`: administrator-only module command bus and eBPF workspace handoff (not a system shell)
- `/login`, `/register`, `/otp-setup`, `/account`: account and TOTP flows

The interface supports Simplified Chinese, English, Spanish, and Japanese.

## Development

```bash
npm ci
npm run dev
```

Verify a production build with:

```bash
npm run build
```

## Compiler and editor intelligence regressions

`npm run test:compiler-check` runs the portable request/deadline/cancellation tests and is included in
CI and the frontend quality gate. `npm run test:compiler-diagnostics-browser` additionally exercises
the real React hook in Chromium with controlled fetch responses and a paused clock. It builds an
ephemeral fixture using the installed Next.js/TypeScript dependencies; no running app, Engine, account,
network access or kernel work is needed. Set `CYANREX_PLAYWRIGHT_MODULE` to an installed Playwright module
and, when necessary, `CYANREX_CHROMIUM_PATH` to the Chromium executable; neither is newly vendored here.

The existing `npm run test:safety-browser` suite includes production-editor remote expiry/cancellation
and explicit source-transfer confirmation. It requires a separately started local production frontend
at `CYANREX_UI_BASE_URL` (default `http://localhost:3217`); Engine requests remain fixtures. This is not a
real Engine/Agent or LAN acceptance run. See [bug hunt 03](../docs/en/functional-network-bug-hunt-03.md).

`npm run test:semantic-completion` covers editor-owned transport/cache/cancellation and is also included
in CI and the frontend quality gate. `npm run test:editor-intelligence-browser` exercises the actual
controller and language-provider implementation with React StrictMode, synthetic Monaco models/fetch,
and a paused clock. It needs the same optional Playwright/Chromium setup, not a running Engine.
The production safety suite additionally inserts semantic suggestions and multiline snippets in real
Monaco, and verifies explicit local header checks after metadata errors. See
[bug hunt 04](../docs/en/functional-network-bug-hunt-04.md) for the distinction between these test boundaries.

## Manual run and attachment regressions

`npm run test:runtime-request` covers private transport, full-body deadlines and runtime response
validation; it is included in CI and the frontend quality gate. `npm run test:runtime-lifecycle-browser`
uses the actual controller under React StrictMode with synthetic responses and a paused browser clock.
It covers duplicate admission, navigation/draft changes, late responses and inventory reconciliation
without a running frontend or Engine. It needs the same optional Playwright/Chromium setup above.

`npm run test:runtime-safety-browser` uses the separately built and started frontend, real Monaco and
mocked Engine HTTP/WebSocket traffic. Set `CYANREX_UI_BASE_URL` and match `CYANREX_UI_ENGINE_URL` to the
build's `NEXT_PUBLIC_ENGINE_URL`. Run it together with `test:safety-browser` and `test:auth-session-browser`
for confirmation/navigation preservation. Tests do not load real programs or contact a live Engine.
Browser cancellation is not kernel rollback; refresh the inventory before deciding to retry an uncertain
operation. See [bug hunt 05](../docs/en/functional-network-bug-hunt-05.md) for evidence and remaining limits.

## Debug-session and event recovery regressions

`npm run test:breakpoint-lifecycle-browser` uses real React hooks, synthetic editor models and sockets,
and a paused clock. It checks session/Engine transitions, declared instrumented lines, bounded hit history,
reconnection and exact editor/model cleanup. `npm run test:breakpoint-safety-browser` uses the built app
and real Monaco with the same optional browser settings as the runtime suite. Run the existing
`test:event-stream-browser` alongside it to cover the event center's shared recovery controller.

The expanded portable `test:event-stream` remains in CI and the quality gate. Browser suites remain
optional and mock all Engine traffic; no kernel programs are loaded. F9/gutter changes prepare a later
manual run, while clearing requested breakpoints does not remove probes already running in the kernel.
See [bug hunt 06](../docs/en/functional-network-bug-hunt-06.md) and [event recovery](../docs/en/event-stream.md).

## Event history and acknowledgement regressions

`npm run test:event-page` covers filter validation, response parsing and private request deadlines, and
is included in CI and the frontend quality gate. `npm run test:event-page-browser` mounts the real Events
page, Sidebar and confirmation components under React StrictMode. Only routing/page storage and Engine
transports are fixtures, with controlled responses and a paused clock; no running server is needed.
It uses the optional Playwright/Chromium settings above and the installed Next.js/TypeScript toolchain.

`npm run test:event-actions-browser` exercises the built frontend's native JSON download, four-locale
320 px invalid-range controls and explicit acknowledgement retry. Run it alongside the existing event,
confirmation, runtime, breakpoint and logout production-browser suites. Every Engine request is mocked.
Read acknowledgement still affects all current-account events; filtered IDs or durable acknowledgement
are not introduced. See [bug hunt 07](../docs/en/functional-network-bug-hunt-07.md).
