# Contributing to cyanrex-lab

[简体中文](CONTRIBUTING.zh-CN.md)

cyanrex-lab is a free, open-source, self-hosted project moving from eBPF teaching toward a
domain-neutral collaboration platform. The teaching runtime remains supported while general identity,
Task, Artifact and Review services are developed separately. Contributions to the platform, domain
packages, runtime, editors, course material, translations, tests and deployment tooling are welcome.

## Before You Start

- Read the [architecture overview](docs/en/architecture.md) before changing
  service boundaries, routes, authentication, or deployment modes.
- Read the [security guide](docs/en/security.md) before changing privileged
  Engine behavior, eBPF loading, remote access, or classroom isolation.
- Search existing issues before opening a new one.
- Read [current project status](docs/en/project-status.md) and the [feature map](docs/en/platform-network.md)
  to distinguish live services, local-only UI and explicit preparation layers before choosing an entry point.
- For a large feature or architectural change, open a proposal issue first so the platform or domain
  use case, authorization boundary and compatibility impact can be discussed.
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

### Choose the right boundary

- Generic identity, content, task and review contracts belong in the shared collaboration model, not
  in an eBPF controller. Code is optional task payload; a Task need not execute a Run.
- Domain definitions and typed assessment rules belong in an explicit domain package. A catalogue
  entry does not grant execution, read access, verified evidence or Task acceptance.
- Local editor state belongs to its owning task draft. Do not treat local IDs, filenames, language
  labels or browser confirmation as server authority, filesystem access or persisted Artifact references.
- Direct stores assume trusted ownership. Untrusted commands need the current-Session transaction
  guard, exact namespace/scope checks and post-write verification, not a detached permission precheck.
- Live AuthService/teaching tables and preparatory stores remain separate. Do not wire startup
  installers, automatic migration, new fallback behavior or cross-user access as incidental refactoring.
- Source-level control/execution separation does not isolate the privileged Engine. Runner Agents are
  compute workers; AI orchestration and executable plugins need separately reviewed boundaries.

### Test the change

The repository follows test-driven development:

1. Add or update a test that describes the intended behavior.
2. Make the smallest implementation change that passes the test.
3. Refactor while keeping the test suite green.
4. Update user-facing documentation and translations when behavior changes.

Backend route changes should update tests under `engine/tests/`. Frontend
permission changes should update `frontend/tests/sidebarPermissions.test.mjs`
when applicable.

Use the [current testing guide](docs/en/testing-guide.md) for service, transaction, browser, SDK and
real external-boundary coverage. Default Cargo tests deliberately skip opt-in database/kernel cases;
browser fixtures with mocked Engine responses do not establish production authorization or server saving.

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

### Local Build Disk Usage

Rust development and test builds keep line-level debug information and disable
incremental compilation to limit disk growth during repeated test runs. Release
builds are unchanged. For a debugging session that needs local variable inspection,
override with `CARGO_PROFILE_DEV_DEBUG=2` (or `CARGO_PROFILE_TEST_DEBUG=2` for tests).
You can opt back into incremental compilation with `CARGO_INCREMENTAL=1`; this
trades more disk space for faster rebuilds.

Inspect disk usage first and stop builds/processes using the selected output. Prefer cleaning only
the project package, keeping third-party dependency artifacts. From the repository root, preview
the exact target before applying the same command without `--dry-run`:

```bash
cargo clean --manifest-path engine/Cargo.toml --target-dir engine/target --locked --offline -p cyanrex-engine --dry-run --verbose
cargo clean --manifest-path engine/Cargo.toml --target-dir engine/target --locked --offline -p cyanrex-engine
```

This removes this package's outputs across cached versions, not just an old version. The next
build must compile/link the project again. Full `cargo clean` also discards dependency builds;
it is a larger reset, not the default for routine cleanup. Compact local validation can explicitly
set `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2` without changing release profiles.

| Path | Owner and cleanup boundary |
|---|---|
| `engine/target/` | Rebuildable Cargo output; prefer the package-scoped preview above |
| `frontend/.next/cache/` | Rebuildable Next cache; remove only with no active build/dev writer, keeping the rest of `.next/` when preserving a built app |
| `node_modules/`, `sdk-js/dist/`, `*.tsbuildinfo` | Installed dependencies/build outputs; keep useful installed dependencies unless reinstall/rebuild is intended |
| `frontend/public/course/`, `frontend/public/monaco/` | Tracked generated assets; regenerate/check with their scripts, do not delete as cache |
| `.git/`, `docs/`, `reports/` | Source history, maintained documentation and validation evidence; not build caches |
| Runtime data, `.env`, `.run/`, database volumes, migration backups, rollback images | Persistent/private or recovery state; excluded from routine cleanup |

Check filesystem mounts before treating `/tmp` as repository disk usage; a separate tmpfs reclaims
memory, not repository storage. Do not replace scoped cleanup with global Docker/npm/Cargo pruning,
home-directory deletion or removal of other projects' artifacts. If it is unclear whether a target
is in use, leave it alone rather than stopping services or deleting uncertain data.

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

## Release Workflow

`engine/Cargo.toml` is the canonical version source. A release must update every synchronized version
target, move the relevant entries from `Unreleased` into a dated `CHANGELOG.md` section, and pass the
full quality gate before the version commit is tagged.

Create an annotated `v<version>` tag on the exact version commit. Do not move an existing release tag;
publish a follow-up patch release when a released artifact needs correction. Branch and tag publication
remain explicit operations so a local validation run cannot publish accidentally:

```bash
./scripts/quality-gate.sh
git commit -m "0.5.0"
git fetch origin --tags --prune
node scripts/release-preflight.mjs --version 0.5.0
git tag -a v0.5.0 -m "cyanrex-lab 0.5.0"
node scripts/release-preflight.mjs --tag v0.5.0
git push origin main
git push origin v0.5.0
```

The release owner should verify the tag target, distribution checksum, extracted-package smoke result,
and changelog links before publication. A pushed version tag also runs the dedicated release workflow:
it validates immutable metadata, builds the offline Docker package, checks tag-bound extraction and
live-kernel installation smoke, and verifies the candidate. These remote results require separate
verification; the workflow does not create a GitHub Release, publish npm packages or deploy services.

## Course Material and Translations

Project-wide documentation starts at [the index](docs/en/README.md). Keep each layer focused:

- The root README covers positioning, current capability boundaries and a short start path.
- The documentation index guides readers by role; the [reference index](docs/en/reference-index.md)
  owns the complete topic catalogue instead of repeating every implementation link on each entry page.
- Project status describes current capabilities, gaps and next decisions; architecture/network guides
  explain responsibilities and connections. Runtime commands and endpoint detail belong in the
  [runtime reference](docs/en/runtime-reference.md) or their component guide.
- Dated runs belong in development history, status archives or reports. Preserve their versions,
  hashes and failure counts; do not rewrite them to imply new acceptance.

Keep reference links reachable from the entry pages. Documentation navigation tests follow bounded
local Markdown paths, so a new feature does not need a repeated paragraph in every overview.

Update English and Chinese sources under `docs/`, then run `node frontend/scripts/sync-course-docs.mjs`;
the `frontend/public/course/` tree is generated from those sources, not a second place to author changes.

English is the primary source language for new interface strings and course
material. When practical, update the matching Simplified Chinese content in
the same pull request. Other supported interface languages are Spanish and
Japanese and Korean (`ko`). New UI keys must be added to all five merged dictionaries; keep
interpolation parameters, protocol identifiers and inserted code unchanged. Korean UI support does
not imply Korean course bodies; retain the explicit fallback notice until those sources exist.

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

- the platform, domain or operational problem being solved;
- the chosen behavior and important tradeoffs;
- how it was tested;
- any security, migration, deployment, or translation impact.

Screenshots are helpful for visible UI changes. Remove account names, hostnames,
IP addresses, tokens, and student information before attaching logs or images.

By submitting a contribution, you agree that it may be distributed under the
repository's [Apache License 2.0](LICENSE).
