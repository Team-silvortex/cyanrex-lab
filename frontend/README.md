# cyanrex frontend

Next.js control plane for Cyanrex eBPF experiments.

## Implemented pages

- `/dashboard`: service overview and quick actions
- `/ebpf`: Monaco editor, templates, script persistence, runtime controls, and attachment management
- `/helper`: engine and eBPF environment diagnostics
- `/modules`: module catalog and lifecycle controls
- `/events`: event filters, export, deletion, and realtime updates
- `/settings`: per-user event retention settings
- `/terminal`: command dispatch interface
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
