# Platform Artifact private directory preflight

Included in source **0.6.0** on **2026-10-15**; see [release validation](../../reports/releases/0.6.0/README.md).
The dated implementation scope below remains historical, not a new deployment acceptance.

This 2026-10-15 Unreleased follow-up to 0.5.9 adds an independent, read-only directory gate under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). It checks an explicitly
selected existing Artifact root without constructing a store, opening a database or starting a host.
It is separate from the [database catalog preflight](platform-host-preflight.md).

## Existing private directory

`inspect_platform_artifact_root(config, root)` is a synchronous trusted preparation call. It reuses
the existing Artifact store's descriptor-relative directory walk rather than introducing another
path policy. The path must be absolute, bounded and below a protected ancestor chain. Every component
is opened without following symlinks; missing paths, parent traversal and non-directory components fail.

The final directory must belong to the current effective user and have exactly `0700`, without extra
special mode bits. Ancestors must belong to root or the current user and must not be group/other
writable, except for root-owned sticky directories such as the usual temporary parent. The broader
`0750` and `0755` user-save policy does not apply to this private storage root.

Inspection neither creates nor chmods a directory. It does not enumerate entries, read Artifact bytes,
check free space, repair permissions or prove content integrity. A nonempty root can pass; its contents
are outside this gate.

## Original directory binding

`PlatformArtifactRootObservation` retains the original directory descriptor, complete supplied host
configuration, exact path spelling and a snapshot of device, inode, UID, GID and full mode. It has no
Debug, Clone, serialization or public descriptor accessor. The path and retained descriptor are checked
again before an observation is returned. Failure returns only `ArtifactRootUnavailable`, never a raw
filesystem error or the selected path.

`verify_platform_artifact_root(config, root, expected)` checks the original held descriptor and freshly
traversed visible directory against that snapshot. Changed configuration or path spelling, replacement,
unsafe ancestry or changed directory security metadata returns `ArtifactRootChanged`. It does not adopt the new
directory. The observation's `matches` method is only a passive comparison of recorded fields; it does
not perform a fresh filesystem check.

Directory timestamps and link count are not identity fields: ordinary content creation/removal must
not invalidate an otherwise unchanged root. Likewise, restoring the original directory and permissions
can make a later check match again. This is not a history of every intervening change.

## Scheduling and handoff limits

These are synchronous metadata operations, not a hard filesystem I/O deadline. The trusted caller
chooses where to run them; no hidden worker, request task or cancellation budget is created. A slow or
unavailable filesystem can block its caller.

The observation pins the final root, not every ancestor's original inode. A successful check is a
point-in-time result, not isolation from the same UID/root, a mount-namespace proof, process-capability
validation or ongoing authorization. It does not make the process low-privilege.

The held descriptor is not handed to `ArtifactStore` as a new runtime capability. A later independent
path-based store open still has a gap and must retain the store's own root guards. No service is mounted,
Session issued, storage installed or migrated, and staging roots remain a separate gate.

## Verification

Default Unix Rust regressions use only private synthetic directories. They cover read-only success,
invalid and special-file paths, symlinks, directory/ancestor permissions, exact configuration/path
binding, replacement and restoration, and normal content changes. No database fixture or privileged
ownership change is required. The default backend quality gate and CI discover these tests normally.

Run just this boundary with:

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_artifact_root_tdd
```

This verification does not establish storage schema compatibility, valid authority history, healthy
Artifact bytes, a writable deployment or actual Linux process isolation.
