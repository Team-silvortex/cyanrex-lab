# Lab 5: Understand the Verifier and Debugging

Estimated time: 50 minutes.

## Objective

- Distinguish clang errors and verifier rejects.
- Practice null checks, boundary checks, and bounded loops.
- Develop root-cause first debugging flow.

## Stage 1: clang errors

In any template, change a variable name to unknown identifier:

```c
return MISSING_SYMBOL;
```

Editor should show `use of undeclared identifier` at that line. The program is still in user-space C compile stage.

## Stage 2: null-check map return

In Lab 3 code, remove `if (!counter)`. clang may compile, but verifier still needs proof of non-null pointer.

Fix pattern:

```c
if (!counter) {
  return 0;
}
```

Null check must appear before dereference, and control flow must allow verifier to follow it.

## Stage 3: XDP packet bounds

`data` and `data_end` describe accessible packet range. Before Ethernet access check:

```c
void *data = (void *)(long)ctx->data;
void *data_end = (void *)(long)ctx->data_end;
struct ethhdr *eth = data;

if ((void *)(eth + 1) > data_end) {
  return XDP_PASS;
}
```

Even when packets are usually large enough, verifier accepts access only if all paths are provably safe.

## Stage 4: bounded loop

This loop boundary is hard for verifier:

```c
while (condition_from_packet) {
  /* ... */
}
```

Prefer compile-time bounded loops:

```c
#pragma unroll
for (int i = 0; i < 8; i++) {
  /* each access still needs bounds checks */
}
```

Modern kernels support bounded loops, but complexity still increases verifier state.

## How to read logs

1. Determine stage failure: compile, load, or attach.
2. Start from first error line, not downstream secondary errors.
3. Record verifier register and pointer types.
4. Go back to latest helper call, pointer math, or control-flow branch.
5. Re-check with minimal edits.
6. Explain what new code proves to verifier after success.

## Integration Task

Choose a passing template and reproduce/fix:

- one undeclared identifier
- one missing NULL check
- one missing packet boundary check
- one unbounded loop

For each record failing stage, key log, root cause, and why fixed.

## Acceptance

- Distinguish clang vs verifier errors within 1 minute.
- Fix is explained by safety proof, not "it runs now".
- Detach all programs before class end.

