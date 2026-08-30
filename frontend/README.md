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
