# Security Policy

cyanrex-lab compiles and loads eBPF programs through a privileged Engine. A
security issue can therefore affect the host running a lab, not only the web
application. Please report suspected vulnerabilities privately.

## Reporting a Vulnerability

Use [GitHub private vulnerability reporting](https://github.com/Team-silvortex/cyanrex-lab/security/advisories/new).
Do not open a public issue for an unpatched vulnerability.

Include as much of the following as is safe to share:

- affected commit, release, and deployment mode;
- Linux distribution, kernel version, container runtime, or WSL version;
- required account role and network position;
- reproduction steps or a minimal proof of concept;
- expected and observed impact;
- relevant sanitized logs;
- suggested mitigation, if known.

Do not include real credentials, TOTP seeds, session cookies, private keys,
student records, or unrelated host data. If a proof of concept could damage a
system, describe it first and wait before sending executable material.

## Response Process

Maintainers will triage reports according to impact and available volunteer
capacity. We aim to:

1. confirm receipt and request any missing reproduction details;
2. reproduce and assess affected versions and deployment modes;
3. prepare a fix or documented mitigation;
4. coordinate disclosure after users have a reasonable update path;
5. credit the reporter unless anonymity is requested.

This is a volunteer-maintained project and does not provide a guaranteed
response or remediation SLA.

## Supported Code

Security fixes target the latest published release and the default branch.
Older releases may receive an upgrade recommendation instead of a backport.

## Deployment Boundary

Authentication and roles do not turn a shared privileged Engine into a secure
multi-tenant sandbox. Untrusted learners should run in separate disposable VMs
or equivalently strong host boundaries. Do not expose the Engine directly to
the public Internet.

Operational isolation, incident response, and dependency-audit guidance is in
the [English security guide](docs/en/security.md) and
[Simplified Chinese security guide](docs/zh-CN/security.md).
