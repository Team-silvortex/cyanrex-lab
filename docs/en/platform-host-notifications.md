# Platform host service notifications

Included in source **0.6.3**, the Linux [plain](platform-loopback-host.md) and
[native HTTPS](platform-loopback-tls-host.md) Session hosts optionally send service-state notifications.
Append the single final flag `--notify-systemd` to either existing serve command. Diagnostics do not
accept it. Without the flag, the old behavior remains and `NOTIFY_SOCKET` does not enable notifications.
This implements a bounded service-management seam under [proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3),
not an installer, persistent service or accepted deployment.

## Explicit notification endpoint

With the flag, the trusted service manager must provide `NOTIFY_SOCKET`. The host reads it once,
after its existing process checks and before opening private configuration or TLS files. Missing,
empty, overlong or unsupported values fail with the existing neutral host failure. Only an absolute
Unix datagram pathname or Linux abstract name beginning with `@` is supported; VSOCK and other
transports are refused. The endpoint is selected by the operator, not authenticated as systemd by
the host. It does not select an Origin, listener, database, role, credentials or storage baseline.

The host connects one nonblocking, close-on-exec datagram socket and retains it. It neither creates
nor removes the receiver, follows a replacement by reconnecting, invokes `systemd-notify`, nor retries
a failed notification. Native datagrams carry only `READY=1` or `STOPPING=1`. A receiver using
`SO_PASSCRED` obtains the kernel's actual sender PID, UID and GID; the host sends no claimed PID,
file descriptors, status text, paths or secrets. No notification depends on unbounded queue waiting.
Unix pathname lookup is synchronous and still has no hard wall-clock deadline.

## Readiness and shutdown ordering

One `READY=1` is sent only after complete private input, optional TLS-material checks, original-root
and declared-storage inspection, initial client close, router creation, signal registration, final
listener binding and process recheck. Only a successful send allows the existing stdout Ready notice.
A failed send closes the just-bound listener and returns failure without accepting business work.
An error before that point sends neither Ready nor Stopping. Binding can briefly occur without Ready.

When the HTTP driver stops on a signal or handled running failure, it first drops its listener,
attempts one `STOPPING=1`, then requests connection shutdown and performs the existing soft drain.
A failed Stopping send marks the final result failed but cannot skip that drain. No final stopped
datagram is defined; clean exit and the existing stdout stopped notice retain their meanings.
Panics in the main thread, abort, OOM and external process killing do not acquire a new drain guarantee.

Successful send means the kernel accepted a datagram, not that a manager acknowledged it. Ready is
one startup observation, not continuous database health, client certificate trust, browser Cookie
receipt or renewed authorization. Stopping is not completed drain, SQL rollback, OTP restoration,
Session compensation, recovery or permission to retry. No watchdog, heartbeat, reload, socket
activation, PID file or automatic restart is added.

## Manual service integration

For an independently reviewed systemd service, execute `cyanrex-platform` directly as its main
process, append `--notify-systemd`, and select `Type=notify`, `NotifyAccess=main` and `Restart=no`.
These settings follow the [systemd notification protocol](https://www.freedesktop.org/software/systemd/man/latest/sd_notify.html)
and [service notification access policy](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html#NotifyAccess=).
Keep `WatchdogSec` disabled; the host does not implement watchdog notifications. Do not add a wrapper
that reports Ready before the host, or use an HTTP request to the teaching Engine as this host's probe.

The operator must still select and review the actual fixed nonroot UID, primary GID and complete
supplementary groups. systemd's `User=` can initialize NSS groups; `SupplementaryGroups=` adds groups,
not an arbitrary exact replacement. The host's `--groups` gate remains authoritative and rejects
differences. Private input/root access and existing database privileges need independent provisioning;
no service command repairs them. The current teaching Docker/offline package is not a platform-host
deployment package.

Manager startup/stop timeouts and any final killing are separate operator policies. Leave room for
the host's 30-second soft drain and runtime shutdown wait, while recognizing that synchronous work
prevents a hard exit guarantee. A manager timeout or forced kill does not establish rollback or
authorize automatic retry. This change installs no unit, enables no service and changes no system CA,
firewall, LAN/DNS binding, teaching authentication, browser route or storage schema.

## Verification

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_notify_defaults_tdd
bash scripts/test-platform-loopback-tls-host.sh
```

Default tests use actual Unix datagrams and capless CLI children. Disposable PostgreSQL selections
verify notification ordering with real startup and Session work over TCP and HTTPS, including observed
startup waits, refused Ready delivery and failed Stopping delivery with actual SQL drain. They do not install a service or prove
PID 1 attribution, a deployed service policy, durable recovery or multi-machine/browser acceptance.
