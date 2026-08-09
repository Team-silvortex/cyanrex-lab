# Contributing to cyanrex-lab

Thanks for helping make eBPF education easier to access. cyanrex-lab is a free,
open-source, self-hosted teaching platform. Contributions to the runtime,
course material, translations, documentation, tests, and deployment tooling
are all welcome.

## Before You Start

- Read the [architecture overview](docs/en/architecture.md) before changing
  service boundaries, routes, authentication, or deployment modes.
- Read the [security guide](docs/en/security.md) before changing privileged
  Engine behavior, eBPF loading, remote access, or classroom isolation.
- Search existing issues before opening a new one.
- For a large feature or architectural change, open a proposal issue first so
  the teaching use case and compatibility impact can be discussed.
- Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## Development Setup

The supported launcher modes are Docker, WSL2, and native Linux. Docker is the
recommended first-time setup:

```bash
./start.sh start --mode docker
./start.sh status
./start.sh logs
./start.sh stop
```

Use `./start.sh diagnose` to collect environment information before reporting
a startup problem. See the [main README](README.md) and
[troubleshooting guide](docs/en/troubleshooting.md) for prerequisites and
mode-specific instructions.

## Change Workflow

The repository follows test-driven development:

1. Add or update a test that describes the intended behavior.
2. Make the smallest implementation change that passes the test.
3. Refactor while keeping the test suite green.
4. Update user-facing documentation and translations when behavior changes.

Backend route changes should update tests under `engine/tests/`. Frontend
permission changes should update `frontend/tests/sidebarPermissions.test.mjs`
when applicable.

Run the full local quality gate before submitting a pull request:

```bash
./scripts/quality-gate.sh
```

Useful focused checks are:

```bash
./scripts/quality-gate.sh --backend-only
./scripts/quality-gate.sh --frontend-only
./scripts/quality-gate.sh --permissions-only
./scripts/quality-gate.sh --security
./scripts/quality-gate.sh --format-only
```

The frontend check installs dependencies with `npm ci`. If the lockfile is
already installed and unchanged, `--no-npm-install` can shorten a local run.

## Repository Rules

- Maintained source files must not exceed 600 lines.
- Documentation files must not exceed 2,000 lines.
- Avoid hidden fallback behavior for security-sensitive operations.
- Do not commit passwords, session tokens, TOTP secrets, private keys, database
  dumps, student data, or raw incident logs.
- Never expose the privileged Engine directly to untrusted networks.
- Keep changes focused; unrelated refactors belong in separate pull requests.
- Preserve backward compatibility unless the pull request clearly documents a
  migration path.

Run `./scripts/check-file-lengths.sh` to verify the line limits directly.

## Course Material and Translations

English is the primary source language for new interface strings and course
material. When practical, update the matching Simplified Chinese content in
the same pull request. Other supported interface languages are Spanish and
Japanese.

Course contributions should include:

- a clear learning objective;
- kernel, capability, and tool prerequisites;
- a safe cleanup path for every attachment or resource;
- expected output that is stable enough for learners to recognize;
- an explanation of common verifier or environment failures;
- automated validation when the lesson can be checked reliably.

Examples must not assume that a shared classroom Engine is a security boundary.

## Pull Requests

A pull request should explain:

- the teaching or operational problem being solved;
- the chosen behavior and important tradeoffs;
- how it was tested;
- any security, migration, deployment, or translation impact.

Screenshots are helpful for visible UI changes. Remove account names, hostnames,
IP addresses, tokens, and student information before attaching logs or images.

By submitting a contribution, you agree that it may be distributed under the
repository's [Apache License 2.0](LICENSE).
