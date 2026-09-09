# SSH Deployment and Student Classroom Entry

## Decision and current scope

| Entry | Direction | Authorization | Implemented slice |
|---|---|---|---|
| SSH deployment | Teacher workstation → managed Linux host | OS SSH key and independently verified host key | Native `cyanrex-release ssh plan/apply` manages an already installed offline package |
| Minimal discovery link | Student → teacher classroom | Confirmed HTTPS origin, teacher-issued invitation, then password + TOTP | `/join`, minimal discovery descriptor, student invitations and revocation |

Personal use retains the local deployment teacher. Joining another classroom creates a **student**
account there; local teacher rights never transfer. Classroom invitations are unrelated to Runner Agent
bootstrap credentials. Discovered data cannot execute SSH or register privileged Agents. Teachers own
application management; the OS separately controls which keys may manage a host. No browser SSH shell
or private-key upload is added.

Not implemented: mDNS broadcasting/browsing, subnet scanning, package upload, machine provisioning,
automatic upgrades/rollback, VM lifecycle, membership removal or a separate unprivileged control service.
The first discovery transport is a teacher-published link. A future desktop DNS-SD browser may locate
the same entry, but advertisements must never contain invitations, passwords, TOTP, student names,
Agent bootstrap secrets or remote commands.

## SSH management

Use the existing verified extraction workflow to install a trusted offline package on the target.
Retain the versioned/timestamped directory name. Prepare `.env` privately, owned by the SSH user with
mode `0600`; Docker/Compose and required host access must already be configured. This command neither
uploads files nor generates credentials, acquires sudo or opens ports. Local/target release binaries
and target package metadata must have the same version.

Configure an OpenSSH host alias for the intended user/key/port. Verify the host key independently
through the console or a trusted operator before adding it to the explicit `known_hosts` file;
`ssh-keyscan` output alone is not authentication. Never disable host-key checking to fix a mismatch.
Local SSH configuration (including any jump/proxy commands) is trusted operator input. Keys remain
in OpenSSH/ssh-agent; the application does not read or forward them.

Example with placeholder host, directory and timestamp:

```bash
./cyanrex-release ssh plan --host classroom-host \
  --directory /srv/cyanrex/cyanrex-lab-0.3.7-20260909-010203 \
  --known-hosts /home/teacher/.ssh/known_hosts --action up
```

`plan` is offline. Review the complete host, directory, action, expected version and argv, then use
its exact `confirmation` value in a separate operation:

```bash
./cyanrex-release ssh apply --host classroom-host \
  --directory /srv/cyanrex/cyanrex-lab-0.3.7-20260909-010203 \
  --known-hosts /home/teacher/.ssh/known_hosts --action up \
  --confirm 'APPLY <hash-from-reviewed-plan>'
```

Use a fresh plan for `status` or `down`. `down` stops the stack and optional Agent without volume-delete
flags. Confirmation binds host, path, action, version, known-hosts digest and argv; it is an accidental
action safeguard, not an SSH credential. Changes to trusted local SSH configuration also need review.

Only literal ASCII aliases and absolute paths are accepted. Shell fragments, traversal and arbitrary
action arguments are rejected. Strict host checking, noninteractive public-key authentication and
no agent/X11 forwarding, implicit forwards, host-key updates or connection multiplexing are enforced.
See the [OpenSSH client](https://man.openbsd.org/ssh) and
[configuration manual](https://man.openbsd.org/ssh_config) for underlying option semantics.

The target validates binary/package version, checksum-bound metadata and deployment control files,
then private `.env` ownership/permissions, before invoking fixed `deploy.sh up/down/status`. Checksums
detect changes relative to a trusted package, not a malicious author or compromised host. Image content
still requires existing package acceptance tooling; this is not a new full image/kernel acceptance run.

Connection timeout, keepalives and a 10-minute overall bound apply. No failed operation is retried.
Disconnect/timeout does **not** imply rollback: inspect status and host logs before retrying. No firewall,
bind-address, CORS, TLS, certificate or production configuration changes happen automatically.

## Discovery and invitations

Disabled by default. Set all three classroom values together in private Engine configuration:

```dotenv
CYANREX_CLASSROOM_ID=23d40d83-19de-43d3-85fe-506d86592a70
CYANREX_CLASSROOM_NAME=Lab A
CYANREX_CLASSROOM_PUBLIC_URL=https://teacher.example
CYANREX_CORS_ORIGINS=https://teacher.example
NEXT_PUBLIC_ENGINE_URL=https://teacher.example:8443
CYANREX_SECURE_COOKIES=true
CYANREX_ALLOW_REGISTRATION=false
CYANREX_ALLOW_TOTP_BOOTSTRAP=false
CYANREX_ALLOW_MISSING_ORIGIN=false
```

These are non-secret examples, not ready-to-run network configuration. Generate a stable, non-nil
UUID for your instance and retain it across upgrades. Use a non-sensitive label. PUBLIC_URL is the
**frontend origin**, without credentials, path, query or fragment. LAN access requires HTTPS; HTTP is
only for trusted loopback/local SSH access. The classroom ID is a continuity identifier, **not a
cryptographic fingerprint**. Independently confirm the exact HTTPS origin and trust its certificate;
a private LAN CA must be provisioned separately through a trusted channel.

Provide protected TLS ingress and firewall/source restrictions; preserve loopback Compose bindings
(the shared bind setting also publishes PostgreSQL). Do not expose the privileged Engine/database to
enable discovery. Frontend/API origins must satisfy the existing same-site cookie policy.

`NEXT_PUBLIC_ENGINE_URL` is baked into Next.js assets and CSP at **build time**. Source Compose passes
it as a frontend build argument, and the package builder passes the exported value. Rebuild/repackage
for the intended API origin: changing a distribution `.env` cannot rewrite an existing image. Native/WSL
development forwards this URL and CORS/secure-cookie settings. A frontend built for `localhost:8080`
cannot serve LAN students unchanged.

1. Student opens the teacher's published `/join`. The page reads the configured Engine's
   `GET /.well-known/cyanrex-classroom` without credentials, caching or redirects.
2. The descriptor contains only service type, classroom ID/label, product version, protocol range,
   entry URL and capabilities. No teacher username, roster, kernel detail or secret is exposed. It
   must describe the current frontend; the page never changes API origin from discovery data.
3. Teacher opens **Classroom → Student discovery & invitations**, confirms an assigned username,
   and privately shares its one-time link. This is authorization for that student, not open registration.
4. Student independently verifies the displayed frontend/API addresses and classroom ID, explicitly
   confirms them, and sets a password for that username. Final confirmation names the exact target.
5. Success displays a TOTP secret once, with a locally generated QR code. Save it in an authenticator
   and use normal password-plus-OTP login. Joining does not issue a session, deploy software or run eBPF.

Each invitation is a random 64-hex-character capability, bound to one normalized student username,
valid for 10 minutes and usable once. Engine stores only its SHA-256 digest and non-secret inventory,
in memory, with at most 256 outstanding invitations. Restart invalidates them. The token is in a URL
**fragment**, not query parameters. The page clears the fragment and Next history state before API
requests, and never stores secrets in local/session storage. Invitation/TOTP responses are `no-store`.
Never put private links in advertisements, announcements, logs, screenshots or shared clipboard history.

Inventory cannot recover a private link. Revocation only invalidates an unused invitation; it does not
delete an enrolled account or revoke an existing session. Public registration/bootstrap may remain off.
Reserved teacher names and client-provided role fields cannot be used for enrollment.

Redemption is atomic **before** asynchronous account creation. A valid invitation remains consumed if
creation fails or a response is lost: no blind retry or credential restoration. Existing accounts use
normal login. If a created account's OTP response was lost, inspect it and follow supervised recovery;
there is no new teacher account/OTP reset UI. Existing account persistence/fallback rules still apply,
including loss of memory-only accounts on restart.

## Compatibility and limits

The join protocol is independent of product releases and Runner Agent v1. Current range is `1..1`,
with `student-invite-v1`. Clients send explicit classroom ID, protocol version, product/client version
and required capabilities. Product patch differences alone do not reject compatible clients; product
version is informational, not proof that security updates are installed or permission to run a
vulnerable release. Unsupported protocol/required capability returns `426` before token consumption;
wrong classroom ID returns `409`. There is no downgrade, alternate-origin or public-registration fallback.
Invalid/wrong-user/expired/revoked/replayed invitations return `403`. Bodies are capped at 4 KiB;
teacher writes retain session/CSRF checks.

SDK `classroom.discovery/invitations/invite/revoke/join` and generated operation IDs cover this flow.
SDK classroom requests reject non-loopback HTTP, redirects and caching. API consumers must perform
their own teacher-origin verification; compatibility and DTOs are not authentication.

This is account onboarding into the current Engine, **not multi-student kernel isolation**. Do not invite
untrusted students into a shared privileged runtime. Teacher-owned unprivileged control plus exclusive
student VMs remain the [target](classroom-isolation.md). A discovered student-owned host is not authoritative.
Tests use synthetic invitations, fake SSH/target scripts and browser-intercepted Engine responses;
real LAN TLS, SSH installation and VM isolation still require designated-host acceptance.
