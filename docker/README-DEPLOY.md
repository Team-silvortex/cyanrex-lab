# Cyanrex Lab Offline Deployment

[简体中文](README-DEPLOY.zh-CN.md)

Runtime package values remain in `manifest.env`. Auditable build details are recorded in
`release-metadata.json`: package identity/time, Git revision and clean/dirty state, a matching annotated
version Tag when available, image references/build mode, Compose template source, and the streamed
SHA-256 of `cyanrex-images.tar`. Each image reference is also bound to its Docker content ID; installation
acceptance ignores inherited image overrides and verifies those IDs again after loading. The JSON file is
covered by `checksums.sha256`, but neither file is a cryptographic signature.

## Package contents

- `docker-compose.yml` and `.env.example`
- `deploy.sh`, `run.sh`, and `stop.sh`
- `runner-agent.sh`, `runner-agent-smoke.sh`, `live-kernel-smoke.sh`, and the native
  `cyanrex-release` CLI
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

The packaged acceptance path requires Docker, Bash, curl, OpenSSL, and standard shell/checksum tools;
it uses the bundled Rust CLI for JSON processing and does not require host Python. Before creating
runtime configuration, `./cyanrex-release package verify .` checks the exact extracted file set and
metadata. After loading images, `./cyanrex-release package verify-loaded-images .` checks the
checksum-bound metadata and Docker image IDs; this second command does not recheck every package file.

The default path stays non-destructive to the host kernel beyond starting the privileged Engine. On a
disposable privileged Linux acceptance host, require a real Aya attach/ring-buffer-event/exact-detach
cycle as part of the installation smoke:

```bash
CYANREX_SMOKE_RUN_LIVE_KERNEL=1 \
CYANREX_KERNEL_SMOKE_REPORT=/safe/output/live-kernel-acceptance.json \
./install-smoke.sh
```

The live check refuses a non-empty administrator attachment set and verifies that no tracked attachment
remains. When an evidence path is supplied, it refuses to overwrite an existing file and records the
candidate metadata hash, environment, unique event, and cleanup result. It is enabled for annotated Tag
candidates, not for routine non-privileged tooling checks.

The packaged verifier performs strict offline schema and candidate-binding checks. Supply the same
package metadata used to create the report:

```bash
./cyanrex-release evidence verify /safe/output/live-kernel-acceptance.json \
  --release-metadata ./release-metadata.json
```

Before extracting a four-file Tag workflow artifact, a trusted checkout of the matching source Tag can
run `cyanrex-release candidate verify /path/to/artifact --extract-to /path/to/new-output`.
It rejects unsafe archive members, verifies every package checksum, binds the archived metadata to the
external evidence, then manually writes only verified regular files without overwriting an existing
output. A two-file package without evidence can use `cyanrex-release package extract` from a trusted
source checkout instead; `scripts/release-package.py` remains a compatibility entry point. These checks
prove consistency, not publisher authenticity; signing remains a separate release decision.

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
./runner-agent-smoke.sh # prefers the adjacent native acceptance client
```

Set `CYANREX_DEPLOY_WAIT_FOR_HEALTH=0` only when an external orchestrator owns readiness checks.

Normal `up` validates health settings with the bundled CLI before invoking Docker. The health phase
uses `smoke wait-health`: `CYANREX_DEPLOY_HEALTH_TIMEOUT_SECONDS` defaults to 90 (1–3600), and
`CYANREX_DEPLOY_HEALTH_POLL_SECONDS` defaults to 2 (1–60). A single deadline covers connection,
response headers/body and retry delays; zero or invalid values fail before starting services.
This deadline does not cover `.env` evaluation, Docker operations or image loading.

Only a complete JSON response with `status: "ok"` passes, with at most 4 KiB of response data.
The public GET sends no credentials and disables proxies and redirects; HTTP to an explicitly
selected LAN address is allowed, but does not authenticate the server. It proves Engine health only,
not frontend, login or complete-stack readiness. A timeout or unhealthy response exits unsuccessfully
without stopping services or rolling back; inspect `./deploy.sh status` and `./deploy.sh logs engine`.
Skipping the check explicitly reports readiness as unverified.

## Routine checks on an installed package

Run these from the extracted package directory on the deployment host. Use a trusted copy of the
matching-version CLI; `verify-controls` rejects a package from another version.

```bash
./cyanrex-release package verify-controls .
./deploy.sh status
./cyanrex-release smoke health --engine-url http://127.0.0.1:8080
./deploy.sh logs engine
```

The first command checks checksum-bound metadata and only `deploy.sh`, `docker-compose.yml` and
`cyanrex-release`. It tolerates runtime files, never reads or executes `.env`, does not read image
archive contents, and does not call Docker or SSH. It does not check configuration, service health,
all package files or publisher authenticity, and its file observations are not an atomic snapshot.
Keep `package verify` for a freshly extracted directory; do not delete runtime files to make it pass.

The other commands inspect Compose state, probe the selected Engine health URL, or follow logs until
Ctrl-C. Adjust the health port to your configured instance. If a control checksum fails, stop and
compare with a separately verified package; do not overwrite `.env`, delete volumes or rerun
`install-smoke.sh` on the installed instance. That smoke test is for disposable acceptance only.

In the browser, **Helper** runs an explicit environment/Runner check. A new check clears old results;
only a complete successful pair is displayed, and errors require a deliberate retry. Results stay in
the current page rather than a cross-session cache. They are observations, not continuous readiness
or authorization to deploy. **Deployment & settings** distinguishes source-checkout and offline-package
Runner commands; execute them on the deployment host, not in the browser.
The browser wait is limited to 10 seconds; cancelling it does not prove Engine checks or subprocesses stopped.
