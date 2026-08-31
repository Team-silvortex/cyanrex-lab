# module-protocol

The shared catalog contract is defined by [`../module.schema.json`](../module.schema.json). A direct
child directory becomes discoverable only when it contains a conforming `module.json` whose `name`
matches the directory.

The v1 contract covers discovery metadata and in-process control state. It deliberately excludes
executables, shell commands, dynamic libraries, remote loading, and event transport. Those require a
future isolation and ownership protocol before they can become trusted runtime capabilities.
