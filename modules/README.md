# Cyanrex Modules

This directory is one of three distinct catalogues in the project:

| Catalogue | Owner | What registration means |
|---|---|---|
| Module manifests | `ModuleManager` and this directory | Discover bounded metadata and control in-memory lifecycle state |
| Task definitions | `TaskCatalog` and `engine/src/domain_packs/` | Resolve exact domain definitions and explicit typed assessment dispatch |
| Editor languages | `frontend/src/features/editor/languages.ts` | Advertise configured local editing capabilities |

None is an automatic executable-plugin installer, AI Agent runtime or permission grant. The eBPF
teaching domain package is not created by starting `module-ebpf`. See the
[platform architecture](../docs/en/architecture.md) and [domain boundary](../docs/en/task-domain-boundary.md).

Direct child directories opt into the Engine catalog by providing a `module.json` manifest that
conforms to [`module.schema.json`](module.schema.json). Manifest schema version 1 declares a stable
module name, semantic version, description, and bounded capability list.

At startup, `ModuleManager` scans `CYANREX_MODULES_DIR` or the repository-level `modules/` directory.
Malformed manifests, unsupported schema versions, duplicate capabilities, and names that do not
match their directory fail startup. Directories without a manifest are documentation-only and are
ignored by discovery.

`POST /modules/start` and `POST /modules/stop` update the single-Engine control-plane state for a
known catalog entry. They never load a library, spawn a process, or execute files from a module
directory. Adding executable plugins requires a separately reviewed isolation, signature, ownership,
and lifecycle protocol.
