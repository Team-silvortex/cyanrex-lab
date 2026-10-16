# Private platform startup configuration check

The **2026-10-16 Unreleased** [format 3](platform-marker-expectation.md) requires both complete
catalog and five-marker identity declarations. Formats 1/2 retain their original diagnostic meaning;
neither may include `marker_identity`, even null. The private-file gates and 16 KiB limit are unchanged.

Included in source **0.6.1** on **2026-10-15**; see [release validation](../../reports/releases/0.6.1/README.md).
The implementation-stage wording below keeps its original scope; source inclusion is not deployment acceptance.

This **0.6.0** follow-up, Unreleased on **2026-10-15**, adds a Linux-only `check-config` command under
[platform host proposal 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3). It reads an explicitly
selected private JSON file after the [process startup gate](platform-process-startup.md), validates its
shape and existing pure configuration contracts, then discards the input. It does not connect to a
database, open the configured Artifact root, create workers or start a host.

## Run the check

Only after independently reviewing the service IDs and complete supplementary-group policy, use:

```bash
cargo run --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform -- \
  check-config --uid 1000 --gid 1000 --groups 1000 --config /private/platform/input.json
```

Use `--groups none` for an explicitly empty group set. Argument order is fixed. Missing, reordered,
duplicate or extra arguments, malformed IDs and relative configuration paths exit 64. A process,
file or configuration failure exits 1 with the same generic message, without echoing paths, values
or parser/OS errors. Success exits 0 and prints only
`platform configuration check passed; no host started`.

The process gate runs before the file is opened. Its new `verify_before_runtime` requires the original
identity, zero current capabilities, no-new-privileges and the single main thread, both before reading
and after parsing, including failed parsing. These snapshots do not prove that no earlier worker,
secret, connection or listener existed. The separate `check-process` command retains its worker check.

## Private input file

The explicit absolute UTF-8 path is limited to 4096 bytes. Empty, dot or parent components, repeated
or trailing separators and control characters are rejected. Descriptor-relative traversal uses
`O_NOFOLLOW` at every component. Ancestors must belong to root or the effective UID and must not be
group/other writable; a root-owned sticky ancestor such as `/tmp` is allowed only before the direct
parent. That parent must belong to the effective UID with mode `0700`.

The leaf must be an effective-UID-owned regular file with one hard link, exactly `0400` or `0600`,
and at most **16 KiB**. Nonblocking opening prevents a FIFO open from waiting for a writer; type and
metadata are checked before reading. Reading probes one extra byte. Device/inode, ownership, mode,
link count, length and modification/change timestamps are compared afterward; the visible parent and
leaf are reopened without following symlinks and compared with the original held descriptors.

The command never changes permissions, creates input paths or repairs unsafe files. It does not lock
out the same UID or root, and metadata comparisons are not a concurrency lock. Synchronous filesystem
I/O, including NFS/FUSE, has no hard time bound. Discarding secret-bearing buffers is not secure zeroization.

## Required JSON records

All records must be objects with every field present. Unknown or duplicate fields, positional arrays,
wrong types, invalid UTF-8 and trailing JSON content are rejected. Version 1 has this shape; the values
below are synthetic, not discovered or approved deployment coordinates:

```json
{
  "format_version": 1,
  "origin": "https://platform.example:8443",
  "authority_id": "00000000-0000-0000-0000-000000001069",
  "workspace_id": "00000000-0000-0000-0000-00000000106a",
  "namespaces": {
    "source": "source_private", "task": "task_private", "artifacts": "artifacts_private"
  },
  "database": {
    "database": "platform_db", "role": "platform_runtime",
    "system_identifier": 123, "database_oid": 456,
    "transport": {"kind": "loopback", "address": "127.0.0.1"},
    "port": 5432, "password": "replace-with-explicit-database-password"
  },
  "session_limits": {"per_account": 4, "total": 16},
  "artifact_root": "/private/platform/artifacts"
}
```

Alternatively, transport is `{"kind":"unix","directory":"/private/postgres"}`. The existing
[local endpoint contract](platform-database-connection.md) requires a nonzero port and either literal
loopback IP or a canonical Unix directory whose complete socket filename fits 100 bytes. Password is
required, may explicitly be empty, is limited to 4096 UTF-8 bytes and cannot contain NUL.

The original validators enforce canonical trusted Origin, nonzero authority/workspace UUIDs, three
distinct private namespaces, explicit physical database coordinates and Session limits. `artifact_root`
receives only the absolute canonical UTF-8 path syntax check; a nonexistent directory can pass.
No directory identity, storage marker, role privilege, audit history or usable database is established.

This check neither reads nor changes PG variables, pgpass, service files or `.env`, and creates no
SQLx options. For example, PGOPTIONS pollution does not affect this pure check, while the separately
constructed connector still rejects captured PGOPTIONS. Success provides neither an original catalog
observation nor authority to adopt a first connection. Actual trusted baseline acquisition, database
authentication, Artifact root inspection and host composition remain separate work.

## Explicit catalog declaration in version 2

[Version 2](platform-catalog-expectation.md) retains every version-1 field and requires
`catalog_identity` with the role OID, database-owner OID and each namespace's OID/owner OID.
Version 1 forbids that field even when null; version 2 never discovers or fills omitted identities.
All new records retain strict map-only, duplicate/unknown-field and 16 KiB checks.
`check-config` validates the declaration only; it does not authenticate or observe those objects.

## Internal mode ownership

The 2026-10-16 internal refactor centralizes strict missing/present record decoding and selects
exactly one private storage-check mode: version 1 current diagnostics, version 2 catalog declaration,
or version 3 catalog plus marker declaration. It does not add JSON fields, relax rejection rules or
perform I/O during parsing. Declaration records construct typed expectations; only the selector owns
version/presence rules. The CLI and library retain separate runtime/inspection/close responsibilities;
see [platform preparation ownership](architecture-map.md#platform-preparation-ownership).

## Verification

```bash
cargo test --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform
cargo test --manifest-path engine/Cargo.toml --locked --test platform_config_input_tdd
cargo test --manifest-path engine/Cargo.toml --locked --test platform_process_startup_tdd
```

Default tests cover strict records and byte boundaries, real private files, generic errors and old
process-command compatibility. Bounded child fixtures clear only their own capabilities; production
does not. Inotify with positive controls checks that a rejected UID never opens the supplied FIFO and
that polluted password/service/`.env` files remain unread. Fixtures reclaim their own children and paths
before final assertions and preserve parent identity, flag and environment. These are local check-only
tests, not database, TLS, deployed-LAN, Session issuance or browser-save acceptance.

A later separate [check-storage command](platform-storage-check.md) consumes the same validated
version 1, 2 or 3 input for one read-only root/database diagnostic. Version 2 compares the explicit
catalog declaration first; version 3 additionally requires the original five marker identities before row reads. `check-config` still discards it without
opening those targets; neither command creates a retained host baseline or starts a host.
