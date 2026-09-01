# Cyanrex Lab Offline Deployment

Runtime package values remain in `manifest.env`. Auditable build details are recorded in
`release-metadata.json`: package identity/time, Git revision and clean/dirty state, a matching annotated
version Tag when available, image references/build mode, Compose template source, and the streamed
SHA-256 of `cyanrex-images.tar`. Each image reference is also bound to its Docker content ID; installation
acceptance ignores inherited image overrides and verifies those IDs again after loading. The JSON file is
covered by `checksums.sha256`, but neither file is a cryptographic signature.

## Package contents

- `docker-compose.yml` and `.env.example`
- `deploy.sh`, `run.sh`, and `stop.sh`
- `runner-agent.sh` and `runner-agent-smoke.sh`
- `install-smoke.sh` for disposable clean-host installation acceptance
- `cyanrex-images.tar` containing PostgreSQL, Engine, and frontend images
- `manifest.env` and machine-readable `release-metadata.json`
- `checksums.sha256`, `LICENSE`, and quick-readme documents

## Disposable release acceptance

Run this only in a freshly extracted package on a disposable Docker host. It refuses to overwrite an
existing `.env`, creates temporary credentials and a dedicated PostgreSQL volume, starts with image
pulling disabled, exercises login and remote compilation, then removes its generated state.

```bash
./install-smoke.sh
```

Set `CYANREX_SMOKE_KEEP=1` to retain a failed smoke stack for diagnosis.
On a host already using the default ports, select another loopback address such as
`CYANREX_SMOKE_BIND_ADDRESS=127.0.0.2`; non-loopback smoke bindings are rejected.

## Normal deployment

```bash
cp .env.example .env
# Replace every credential placeholder and review bind settings.
./run.sh
./deploy.sh status
./deploy.sh logs
./stop.sh
```

The package publishes only to loopback by default. Prefer an SSH tunnel or TLS reverse proxy for
remote access. If LAN publishing is required, configure the host firewall and trusted-user access
controls before changing `CYANREX_BIND_ADDRESS`.

The optional unprivileged compiler Agent can be enabled after the main stack is healthy:

```bash
./runner-agent.sh start
./runner-agent-smoke.sh
```

Set `CYANREX_DEPLOY_WAIT_FOR_HEALTH=0` only when an external orchestrator owns readiness checks.
