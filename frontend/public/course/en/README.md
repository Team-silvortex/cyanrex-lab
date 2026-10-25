# Cyanrex project documentation

[简体中文](../zh-CN/README.md)

Cyanrex is a self-hosted collaboration and teaching platform built around domain-neutral Tasks.
A payload may contain notes, text files, code or configuration; code and a Run are optional.
eBPF teaching is the first connected domain, not the definition of every Task.

Use the role paths below to get started. [Project status](project-status.md) separates current
capabilities and next decisions; the [complete reference index](reference-index.md) contains the
implementation topics and historical records.

## Choose the right workflow

| Workflow | Current entry and boundary |
|---|---|
| Teach or learn eBPF | The existing teaching application connects accounts, learning records, feedback and Linux execution. |
| Author general Task content | The browser editor owns local drafts and reviewed text-file imports; it does not save them to the platform server. |
| Use the independent platform backend | An explicitly configured low-privilege Linux host exposes limited login, Session and owner-private Task routes on loopback, optionally with native TLS. |
| Compose resource or file services | Separate stores, commands and transport adapters require explicit callers; their existence does not mount them in the teaching application. |
| Explore the target platform | General Run and autonomous AI orchestration, browser-to-platform saving and desktop file transfer remain separate development goals. |

The independent host does not replace teaching authentication. Its Task routes can create an empty
manual Draft, list and reopen owned content, or replace a Draft manifest using existing exact text
references. They do not publish new Artifact bytes or change Task status.

## Teach a class

Start with the [teacher guide](teacher-guide.md), then use [classroom connection and SSH](classroom-connection.md)
for teacher-managed deployment and student entry. A personal instance makes its owner the teacher;
that authority does not transfer to a classroom they join.

Before teaching:

1. Select a trusted Linux execution environment and follow the operations path below.
2. Review the labs from the student's perspective and perform a dry run.
3. Keep deployment credentials private; use the supported student enrollment path.
4. Confirm cleanup and attachment ownership before reusing the environment.

Teacher authority covers the existing teaching workflow and deployment. It does not grant access to
another owner's general Tasks, Artifacts, Reviews or recipient-granted file bytes.

## Learn or join a classroom

Follow the [student guide](student-guide.md) for accounts, safe confirmations, learning records and
the current interface. Use the [concept map](concepts.md) to connect the platform model with the
first eBPF domain.

The introductory labs form one teaching path:

1. [Understand the execution pipeline](labs/01-first-program.md).
2. [Observe execve](labs/02-trace-execve.md).
3. [Count with Maps](labs/03-map-counter.md).
4. [Pass events through a Ring Buffer](labs/04-ring-buffer.md).
5. [Read verifier diagnostics and debug](labs/05-verifier-debugging.md).

eBPF always runs in a Linux kernel. Docker Desktop and WSL2 use a Linux VM or subsystem kernel,
not the Windows or macOS desktop kernel. An application account is not a kernel isolation boundary.

## Deploy and operate

Use the [runtime reference](runtime-reference.md) for launchers, offline packages, accounts,
runtime APIs and persistence. Read [security and deployment boundaries](security.md) before
connecting clients or enabling privileged execution; use [troubleshooting](troubleshooting.md)
to distinguish environment, account, storage and runtime failures.

Choose the matching operational guide:

- [Runner Agent](runner-agent.md) covers the existing compute worker and its explicit permissions.
- [Plain loopback host](platform-loopback-host.md) covers the separate low-privilege platform process.
- [Native HTTPS loopback host](platform-loopback-tls-host.md) adds private TLS identity and Secure Cookies.

Loopback hosting is not LAN or browser deployment acceptance. Client trust, private storage,
process identity and exact configuration still need the checks in the selected host guide.
Service notifications and storage preflights are indexed with the other host details, not extra
normal-application startup steps.

## Develop the collaboration platform

Begin with the [current architecture map](architecture-map.md) for runtime wiring and ownership.
Use [detailed architecture](architecture.md) for implementation decisions, and
[target architecture](next-architecture.md) for intended direction rather than completed features.

For a change to a module or connection:

1. Locate the existing edge in the [platform feature map](platform-network.md).
2. Check the [capability and evidence matrix](capability-maturity.md) without treating a dated score as deployment acceptance.
3. Select the relevant host, identity, Task, Artifact, Review or file topic from the reference index.
4. Preserve the separation between local state, trusted direct stores, current-Session commands and HTTP projection.

A catalogue definition does not grant execution or assessment authority. Publishing an Artifact,
replacing Task content, starting a Run and recording a Review remain distinct operations.
The teaching Engine still combines control with privileged Linux execution; source-level separation
does not make it a multi-tenant sandbox.

## Edit task payloads or integrate an SDK

The [payload editor guide](editor.md) explains task-owned local documents and editor capabilities.
[Local text-file import](local-text-file-import.md) adds a reviewed batch without replacing the
existing draft. Local filenames, IDs and language hints are not server references or permissions.

Use the [JavaScript SDK](../../sdk-js/README.md) for its supported Engine contracts and the
[AI Agent integration guide](ai-agent-integration.md) for connection profiles and explicitly selected
tool adapters. These helpers do not provide autonomous orchestration or a generic platform-save API.

The [module catalogue](../../modules/README.md) contains declarative domain metadata.
A manifest entry alone is not an executable plugin or a newly deployed runtime.

## Review large file distribution work

The [large file references](reference-index.md#large-file-preparation) separate the proposal from
implemented staging, grants, transport and saving components. Controlled delivery can be composed
explicitly; normal-app distribution, save dialogs, resume and desktop transfers are not connected.

Use the selected component's authority and verification boundary rather than treating a completed
download, a saved file, published metadata and a reusable permission as the same outcome.

## Verify or contribute

Read [CONTRIBUTING](../../CONTRIBUTING.md) and the [TDD policy](../../TDD.md) before changing behavior.
The [testing guide](testing-guide.md) distinguishes default checks from opt-in SQL, browser,
transport, SDK and privileged kernel acceptance. [Scripts and tooling](../../scripts/README.md)
provide the corresponding command entry points.

Default Cargo tests skip opt-in database and kernel cases. Browser fixtures with intercepted
Engine calls verify their UI or component boundary, not live authorization or server saving.

## Find a detailed reference or an older result

The reference index groups the full topic inventory by host, identity, content, large files,
teaching runtime and verification. It also links archived status snapshots and chain-guided bug hunts.
Dated reports retain their original versions, failures and limits; they are not current test results.

Use the [changelog](../../CHANGELOG.md) for release contents and Unreleased fixes.
[Translation coverage](localization.md) explains the five interface languages, bilingual guides
and the explicit course-body fallback.
