# Native HTTPS loopback Session host

Included in source **0.6.3**, Linux `cyanrex-platform serve-loopback-tls` serves the existing three
[Session routes](platform-session-router.md), [private Task index](platform-task-index-connection.md), [content GET](platform-task-content-connection.md)
and [empty manual Draft POST](platform-task-create-connection.md), plus [Draft manifest PUT](platform-task-replace-connection.md), through native TLS under
[host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). It shares the
[plain loopback host](platform-loopback-host.md)'s original storage, process and HTTP boundaries,
without a local plaintext proxy. Only `127.0.0.1` and `::1` on ports 1024–65535 are supported.
It does not add LAN/DNS listening, browser integration or deployment acceptance.

The optional final `--notify-systemd` flag adds [service-state notifications](platform-host-notifications.md).
Unflagged startup remains unchanged; notifications do not establish client trust or continuous health.

## Explicit private inputs

The unflagged 15-argument shape is:

```text
cyanrex-platform serve-loopback-tls --uid UID --gid GID --groups GID,...|none --config /absolute/private/input.json --bind 127.0.0.1:PORT --cert /absolute/private/chain.pem --key /absolute/private/key.pem
```

The bind policy is unchanged: canonical `127.0.0.1:PORT` or `[::1]:PORT`, no DNS, wildcard, mapped IPv6,
other 127/8 address, low port or automatic port selection. Private JSON remains complete format 3;
its Origin must be exactly `https://` followed by the canonical bind authority. TLS flags add no
JSON fields, input version, environment fallback or change to `check-config`/`check-storage`.

The process gate still checks the actual nonroot identity, groups and zero effective/permitted/
inheritable/ambient capabilities and sets `no_new_privs=1` before reading secrets. Format/Origin
rejection precedes TLS-file access. The mode then loads and validates TLS material before observing
the Artifact root, creating the runtime or dialing the database, and rechecks the process even when
loading fails. Original catalog/five-marker/root checks, one expected-storage opener, private role
selection, signal registration and final bind/Ready ordering are shared with the plain mode.

Each JSON, certificate and key file independently uses the existing descriptor-relative no-follow
reader: at most **16 KiB**, owned single-link 0400 or 0600 regular file and an owned 0700 final parent.
Certificate files must satisfy this private-file policy too. Symlinks, FIFOs, hard links, broad modes,
unsafe ancestors or changed snapshots fail without repairs. Inputs are read once per startup; there
is no hot reload or atomic transaction across the three files. File reading and PEM/DER/key parsing
are synchronous startup work; byte/count bounds do not make them preemptible or give a hard wall-clock
deadline. Keeping loaded material in memory
does not mean all original PEM/password buffers are securely zeroized.

## Certificate material and client trust

The certificate file contains 1–8 `CERTIFICATE` PEM blocks, leaf first. The key file contains exactly
one unencrypted PKCS8 `PRIVATE KEY`, PKCS1 `RSA PRIVATE KEY` or SEC1 `EC PRIVATE KEY` block. Only ASCII
whitespace may surround blocks; extra text, mixed types, additional keys, encrypted keys or trailing
junk are rejected. BEGIN/END markers occupy exact lines, with LF or CRLF; padding on a marker line
is not outer whitespace. No subprocess, shell expansion, system certificate lookup or automatic generation
is used by the host.

The explicit ring provider must positively confirm certificate/private-key matching; an unknown
match result is not success. Startup also parses the leaf and verifies its IP SAN against the exact
bind IP. This is not a complete PKI check: it does not establish trust, certificate-chain signatures,
validity time, EKU or revocation. `platform loopback host ready` means startup and binding completed,
not that a client will trust the certificate. Clients must perform their own normal chain, validity
and exact-IP verification using an explicitly trusted CA. Never disable verification to get a login
working. CA installation, certificate issuance/renewal and client trust distribution remain external.

IP clients normally send no DNS SNI; this mode permits absent SNI and refuses every DNS SNI, even if
the certificate also lists that DNS name. TLS 1.2 and 1.3 are supported. ALPN offers can select only
`http/1.1`; mixed offers select it and h2-only offers fail. Absent ALPN uses HTTP/1.1. There is no HTTP/2,
Upgrade or plaintext fallback. TLS client certificates are not an authentication path: current
Session commands still authorize every operation. Session caching, tickets, early data and key logging
are disabled; no process-global crypto provider is installed.

## Admission and shutdown

The same 16-connection fail-fast budget is acquired before spawning or beginning a TLS handshake and
held through HTTP response writing. An independent two-second whole-handshake deadline includes
idle and partial input. Stop cancels unfinished handshakes; failed or timed-out handshakes close only their
connection. Successful TLS enters the same HTTP/1.1 driver with a separate two-second complete-header
deadline, 32 headers, 16 KiB parser buffer, exact single Host and one request per connection.

Handshake and parser failures do not run business commands or promise private JSON/HTTP headers.
Host-gate failures after valid TLS retain their empty private 400 responses. Forwarded headers do not
choose HTTPS, authority or storage. All seven workspace routes retain their own strict HTTP contracts;
creation accepts only `schema_version: 1`, `task_id` and `title`, with no payload, code or Run prerequisite.
Replacement PUT takes only schema version, expected revision and the complete title/existing-reference
manifest. Login body/rate limits remain unchanged. The six independent login/lifecycle/index/content/create/replace
response lanes each default to four, with trusted capacities 1–16; old capacity signatures default replace
to four, and `with_replace_capacities` checks all six before splitting the original binding. No Artifact
publication, status writes, browser saving or SDK/OpenAPI is connected. There is no new timeout around a dispatched command or complete connection.

The existing 30-second soft signal drain and one-second runtime shutdown wait remain unchanged.
Synchronous file access and already-dispatched blocking work prevent a hard process-exit guarantee.
Shutdown/disconnect is not SQL rollback, OTP restoration, Session compensation or retry authority.
Both modes retain the same fixed Ready/stopped/failed notices and serve-only private panic hook;
ordinary errors do not disclose PEM, passwords, paths or tokens.

## Cookie and verification boundaries

Confirmed HTTPS login adds Secure to the existing host-only HttpOnly SameSite=Strict Cookie at
Path `/platform/v1`; confirmed logout clears it with the same Secure policy. No token is returned in
JSON. HTTP loopback keeps its old non-Secure policy and is not a fallback for failed TLS.

Secure does not isolate cookies by port, and actual TCP/TLS delivery does not prove browser receipt.
Do not mix unrelated services on this Cookie host or describe HTTPS loopback as completed Cookie-safe
domain planning. Dedicated HTTPS host/domain policy and authorized LAN/browser integration remain
separate; the privileged teaching Engine and its cookies are unchanged.

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_serve_loopback_tls_defaults_tdd
bash scripts/test-platform-loopback-tls-host.sh
```

Native TLS test fixtures require OpenSSL for bounded generation of disposable CA/certificates/keys;
the product host does not invoke it. Tests do not install a system CA or commit private keys. Default
private synthetic routers exercise transport/material handling, not SQL authorization. Eight exact
SQL selections require disposable `CYANREX_TEST_DATABASE_URL` administration and runtime-role password
authentication: five cover HTTPS children, Secure Cookies, peer refusal, original drift, permissions
and observed drain; three cover optional plain/TLS notifications. No tests establish a deployed LAN
or browser workflow.
