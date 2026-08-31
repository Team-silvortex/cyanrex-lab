# module-ebpf

This directory is discovered through its versioned `module.json` manifest. Its declared integration
surface covers:

- attach eBPF program
- detach eBPF program
- stream syscall/network/latency events

Starting the catalog entry changes Engine control state only; it does not execute code from this
directory. The existing Engine eBPF services remain the authoritative implementation.
