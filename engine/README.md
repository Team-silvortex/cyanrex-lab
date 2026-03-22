# cyanrex-engine

Axum backend skeleton for Cyanrex.

## Planned modules

- `auth`
- `module-manager`
- `event-bus`
- `command-dispatcher`
- `ebpf-loader`

## Initial API

- `GET /`
- `GET /health`
- `GET /modules`
- `POST /modules/start`
- `POST /modules/stop`
- `GET /events`
- `POST /command`
- `POST /ebpf/run`
- `GET /helper/environment`
