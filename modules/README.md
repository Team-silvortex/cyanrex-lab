# Cyanrex Modules

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
