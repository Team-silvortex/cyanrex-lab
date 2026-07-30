# Lab 1: Understand the eBPF Execution Pipeline

Estimated time: 30 minutes.

## Objective

- Run the first XDP program.
- Distinguish compile, load, and attach.
- Learn how to safely detach a program.

## Steps

1. In Environment Helper, run check and ensure overall status is Ready.
2. Open eBPF page and choose `XDP Pass` template.
3. Wait until clang status becomes `passed`.
4. Read the core code:

```c
SEC("xdp")
int xdp_pass(struct xdp_md *ctx) {
  return XDP_PASS;
}
```

`SEC("xdp")` declares the program type. `XDP_PASS` means the packet continues through the network stack.

5. Click **Compile and Run**, then observe result:
   - compile stderr should be empty or only non-blocking warnings
   - load stage should succeed
   - automatic XDP attach support differs by environment
6. Check "Attached Programs" list and note pin path.
7. Detach program and confirm the list is empty.

## Mutation Task

Temporarily change return value to invalid `XDP_UNKNOWN`. Wait for live clang check and record error line and message.
Restore to `XDP_PASS`, then confirm red diagnostics disappear.

Then type `ctx->` and observe completion suggestions for `data`, `data_end`, `ingress_ifindex`, etc.

## Questions

1. Does `XDP_PASS` modify the packet?
2. Does clang pass guarantee verifier acceptance? Why or why not?
3. What problem does pin path solve, and what does attach link solve?

## Acceptance

- Can explain `SEC("xdp")` and `XDP_PASS`.
- Can intentionally trigger and fix a clang error.
- No attached programs remain after lab.

