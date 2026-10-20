# Linux platform process startup check

Source **0.6.3** includes explicit loopback/TLS hosting, limited Session issuance and private
Task create/list/reopen/manifest replacement. Browser/SDK saving, new-byte publication, LAN access
and migration remain absent; earlier implementation-stage labels retain their original scope.

The Unreleased [loopback host](platform-loopback-host.md) now reuses these checks before private
input and runtime creation. The `check-process` diagnostic below retains its check-only behavior.

Included in source **0.6.1** on **2026-10-15**; see [release validation](../../reports/releases/0.6.1/README.md).
The implementation-stage wording below keeps its original scope; source inclusion is not deployment acceptance.

This 2026-10-15 Unreleased follow-up after **0.6.0** adds explicit Linux process preparation under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3).
`cyanrex-platform check-process` checks an already selected service identity and restricts its own
no-new-privileges flag. It does not start a host, read secrets, open a database or listen on a port.
The [Session database privileges](platform-session-permissions.md) remain a separate requirement.

## Explicit identity policy

`PlatformProcessIdentity::new(uid, gid, groups)` requires numeric UID/GID and the complete expected
supplementary-group allowlist. Zero and `u32::MAX` are rejected. Groups may be empty; otherwise they
must be strictly increasing, unique, nonzero and contain at most 128 entries. There are no account-name
lookups, environment defaults or adoption of the calling user's groups. An allowed group can still
grant powerful file/socket access; the operator must review it.

For example, **only if these are the independently reviewed service IDs**, run:

```bash
cargo run --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform -- \
  check-process --uid 1000 --gid 1000 --groups 1000
```

Use `--groups none` for an explicitly empty supplementary-group set. Argument order is fixed;
missing, reordered, duplicate, extra, noncanonical, oversized or invalid arguments fail with exit 64.
Unsafe or unavailable process state fails with exit 1. Errors are generic and never echo arguments.
Only success prints `platform process check passed; no host started` and exits 0.
Non-Linux builds refuse the check rather than selecting a weaker fallback.

## Before the first runtime worker

`prepare_platform_process` requires the single main thread before runtime workers, secrets, databases
or listeners. It accepts a 16 KiB record from the fixed `/proc/thread-self/status` descriptor, reading
one extra byte to detect overflow. It checks procfs, rejects missing/duplicate/malformed required fields, and cross-checks native
UID/GID/group and no-new-privileges syscall results.

Real, effective, saved and filesystem UID/GID must all match the explicit nonzero policy. Groups must
match exactly. Effective, permitted, inheritable and ambient capabilities must all be zero. Current
PID/TID and single-main-thread state must match. Non-main callers or observed additional threads are
rejected before setting the flag. The caller must still preserve startup order: thread snapshots do
not prove the absence of a previously constructed runtime, secrets, connections or listeners, and
cannot retrofit an already-running Engine or audit sibling credentials.

Only then does it set and read back `no_new_privs=1` and repeat identity/capability and single-thread
checks. The [kernel contract](https://docs.kernel.org/userspace-api/no_new_privs.html) prevents exec
from gaining set-ID/file capabilities; newly created threads inherit the flag and it cannot be unset.
No UID/GID/group/capability drop or repair is performed. The bounding set may remain nonzero; it is
not a current capability or a replacement for the four zero sets and this flag.

The returned `PlatformProcessStartup` has no Debug, Clone or serialization. Its
`verify_current_thread` rechecks the calling thread against the original process and policy, including
`no_new_privs=1`; it is not continuous enforcement or resource authority. The command repeats
preparation, verifies a newly created worker and checks the main thread before success.

The later [private startup input check](platform-startup-input.md) adds `verify_before_runtime`,
which also requires the single main thread. Its separate `check-config` branch uses it before reading
and after parsing a private file; this does not make `check-process` read secrets or start a host.

## Failure and deployment limits

Setting the flag is irreversible. A later failure may leave it set; failure does not restore old state
or authorize automatic retry. Stop startup and investigate with the trusted launcher. Production
never clears inherited capabilities to make the check pass.

This is not a sandbox or deployed-host acceptance. Inherited descriptors, same-UID access, group
privileges, user namespaces, filesystem access and resource limits need their own deployment policy.
Database transport, physical-target pinning on every connection, ingress, Session issuance and
Task/browser wiring remain separate gates. Teaching Engine behavior and existing storage are unchanged.

## Verification

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_process_startup_tdd
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_host::process_startup::tests
```

Real subprocess tests cover a capability-free child, identity/group rejection, generic arguments,
already-set flag handling and unchanged parent identity/capabilities/flag/environment. The positive
fixture explicitly reduces **only its child** capabilities before exec; production never does.
The existing non-main, multithreaded test harness is refused. Pure status tests independently cover
thread count, saved/filesystem identities, each capability set, bounds, corruption and unknown syscall
results. Synthetic root/capability cases are policy tests, not actual privilege acquisition.
