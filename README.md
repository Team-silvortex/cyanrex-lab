# cyanrex-lab

[简体中文](README.zh-CN.md)

Version: `0.6.4`

Cyanrex is a self-hosted collaboration and teaching platform. A Task owns its goal and optional
content payload; code and execution are not required. eBPF teaching is the first supported domain,
not the definition of every task.

The current teaching application, local task authoring and independent platform host have different
entry points and persistence boundaries. Start with the path that matches your work below.

[Documentation](docs/en/README.md) · [Current status](docs/en/project-status.md) ·
[Architecture map](docs/en/architecture-map.md) · [Reference index](docs/en/reference-index.md) ·
[Changelog](CHANGELOG.md)

## Start here

- **Learn:** follow the [student guide](docs/en/student-guide.md) and five introductory labs.
- **Teach:** use the [teacher guide](docs/en/teacher-guide.md) for classroom and deployment authority.
- **Deploy or maintain:** choose the [Docker guide](docker/README.md) or
  [installed-package operations](docker/README-DEPLOY.md); use the [runtime reference](docs/en/runtime-reference.md)
  for complete commands and API details.
- **Develop the platform:** read the [architecture map](docs/en/architecture-map.md),
  [capability map](docs/en/platform-network.md) and [contributing guide](CONTRIBUTING.md).
- **Integrate an API:** start with the [JavaScript SDK](sdk-js/README.md), not a preparatory platform route.

The [documentation index](docs/en/README.md) provides role-based reading paths. The
[reference index](docs/en/reference-index.md) groups detailed contracts, implementation guides and history.

## Current Capabilities

| Area | Available path | Important boundary |
|---|---|---|
| Teaching | Authenticated browser workflow, eBPF checks/runs, learning history and teacher feedback | Shared-kernel execution is not student isolation |
| Task authoring | Local drafts, optional text payloads, 14 editor language profiles, reviewed imports and downloads | No generic browser server saving, autosave or Run dispatch |
| Platform host | Explicit Linux loopback HTTP/native TLS host with bounded Sessions and owner-private Task create/list/reopen/manifest replacement | Separate startup and storage; not a LAN or browser deployment |
| Large-file transfer | Authorized publication, grants, verified chunks and explicit delivery components | Not mounted in the normal application UI; no restart recovery |
| SDK and AI adaptation | Typed Engine client, host profiles and explicit agent tool-format adapters | No autonomous AI orchestration or provider transport |

The UI supports English, Simplified Chinese, Spanish, Japanese and Korean. Course bodies are English
and Simplified Chinese; other interface languages retain an explicit course-language fallback.

See [project status](docs/en/project-status.md) for current gaps and next steps, and
[capability maturity](docs/en/capability-maturity.md) for implementation and evidence detail.
Released changes and post-tag security fixes are recorded separately in [CHANGELOG.md](CHANGELOG.md).

## Quick Start

Run the existing teaching application on a trusted Linux/WSL2 host or a disposable Docker environment.
Read the [security guide](docs/en/security.md) before granting kernel privileges.

```bash
./start.sh start --mode docker
./start.sh status
./start.sh diagnose
./start.sh logs
./start.sh stop
```

Use `--mode wsl` for the WSL2 Linux kernel or `--mode native` for native Linux.
eBPF always targets a Linux kernel; Docker Desktop observes its VM kernel, not the desktop OS.

Default local endpoints:

- Frontend: `http://localhost:3000`
- Engine health: `http://localhost:8080/health`
- PostgreSQL: `127.0.0.1:15432`, not exposed to the LAN

The seeded `admin` account has the `teacher` role and manages both teaching and deployment.
Single-person use makes you the teacher. Initial password and TOTP material are generated in the
private, Git-ignored `docker/.env`; do not publish that file. Registration is disabled by default.

Use `/ebpf` and `/learn` for teaching. `/tasks/new` (`/editor` is a compatibility entry) owns a
local task draft; a downloaded draft is not a server-saved Task.
The [editor guide](docs/en/editor.md) explains persistence and navigation limits.

For the **independent platform host**, follow its [explicit startup guide](docs/en/platform-loopback-host.md).
Starting the teaching application does not initialize or migrate collaboration storage.
Full launcher options, account settings and runtime endpoints are in the
[runtime reference](docs/en/runtime-reference.md).

## Next-generation Architecture

Task, Artifact, Run and Review have separate responsibilities: task intent and lifecycle, immutable
content revisions, an execution attempt, and version-bound judgment. Identity and Workspace scope
ownership; a current Session authorizes commands within their storage transaction.

The [architecture map](docs/en/architecture-map.md) describes current wiring.
The [detailed architecture](docs/en/architecture.md) records implementation boundaries; the
[next-generation design](docs/en/next-architecture.md) describes the target, not completed AI,
generic Run, cross-user collaboration or live-migration capabilities.

## Repository Layout

```text
frontend/
engine/
sdk-js/
modules/
docker/
scripts/
docs/
reports/
```

`frontend/public/course/` is generated from `docs/`; edit the sources and then synchronize.
Detailed document responsibilities and the complete topic inventory are in the
[reference index](docs/en/reference-index.md).

## Deployment and updates

A source checkout and an extracted offline package use different operational commands.
Use the [deployment guide](docker/README-DEPLOY.md) for package verification, routine health
observations and safe updates, and the [runtime reference](docs/en/runtime-reference.md#25-build-a-distribution-package)
for packaging and verified extraction.

Installation smoke and live-kernel acceptance belong on disposable hosts, not an installed classroom
instance. A dependency or lockfile change does not patch running binaries or containers; rebuild and
redeploy. Do not expose the privileged Engine directly to untrusted networks.

## API Contract

The existing Engine serves its generated [OpenAPI 3.1 contract](engine/openapi/openapi.json) at
`GET /openapi.json`. The [Engine reference](engine/README.md) and
[JavaScript SDK guide](sdk-js/README.md) cover runtime access and integration.
The independent platform host is not automatically part of that public Engine/SDK contract.

Full authentication, eBPF, Runner, script, learning and persistence details are in the
[runtime API reference](docs/en/runtime-reference.md#auth-api-implemented).

## Engineering Rule

Follow [CONTRIBUTING.md](CONTRIBUTING.md) and [TDD.md](TDD.md). Run checks from the repository root:

```bash
./scripts/quality-gate.sh --security
```

The security option requires `cargo-audit`; install it with `cargo install cargo-audit --locked`.
The [testing guide](docs/en/testing-guide.md) separates default checks, explicit database/browser
fixtures and privileged acceptance. Passing a mocked browser fixture does not prove live authorization
or server saving.

### CI Gate (`ci-gate`)

The `CI gate` aggregates the required audit, formatting, Rust, frontend, SDK, permissions and distribution
jobs. See the [runtime CI reference](docs/en/runtime-reference.md#ci-gate-ci-gate) for details.

### Tag candidate acceptance

New annotated version tags trigger immutable metadata and offline-package acceptance checks;
they do not publish a GitHub Release or deploy services. Follow the
[release workflow](CONTRIBUTING.md#release-workflow), and keep dated outcomes in
[release reports](reports/releases/0.6.4/README.md) rather than the homepage.

## Contributing

Cyanrex is free and open source under [Apache License 2.0](LICENSE). Contributions to collaboration,
teaching domains, payload tools, translations and operations are welcome.
Follow the [code of conduct](CODE_OF_CONDUCT.md) and report vulnerabilities through the
[private security process](SECURITY.md).
