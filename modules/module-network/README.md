# module-network

This directory is discovered through its versioned `module.json` manifest. Its declared integration
surface covers:

- capture network events
- publish normalized event payloads to engine event bus

Starting the catalog entry changes Engine control state only; it does not execute code from this
directory.
