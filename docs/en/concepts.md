# eBPF Concept Map

## 1. How an eBPF Program Runs

```text
C source
  -> clang compiles to BPF bytecode
  -> kernel verifier checks safety
  -> loader loads programs and maps
  -> attach to hook
  -> kernel event triggers the program
  -> Map/Ring Buffer/trace outputs data
  -> userspace reads and displays
```

In Cyanrex, the result panel splits this flow into compile, load, and attach stages.
When troubleshooting, identify which stage failed first.

## 2. Hook

A hook is where an eBPF program is invoked. `SEC("...")` declares program type and attach point.

- `SEC("xdp")`: early path in NIC receive path.
- `SEC("tracepoint/category/name")`: stable kernel tracepoint.
- `SEC("kprobe/function")`: dynamic kernel function probe, higher compatibility requirements.
- `SEC(".maps")`: map declaration, not an executable program.
- `SEC("license")`: program license.

## 3. Context

The kernel passes a context pointer when calling an eBPF program, for example `struct xdp_md *ctx` in XDP.
Allowed fields depend on program type. When you type `ctx->`, Cyanrex asks clang for actual fields.

## 4. Helper

eBPF cannot call arbitrary kernel functions and can only use helpers allowed by program type.

- `bpf_ktime_get_ns()`: read monotonic clock
- `bpf_get_current_pid_tgid()`: get process/thread ID
- `bpf_map_lookup_elem()`: map lookup
- `bpf_ringbuf_reserve()`: reserve ring buffer space

Some helper calls require GPL-compatible programs, therefore examples may declare GPL license.

## 5. Map

Maps are shared containers that connect kernel eBPF state and userspace.

- Hash: key-value storage
- Array: fixed index, predictable access cost
- Per-CPU Array: one copy per CPU to reduce lock contention
- Ring Buffer: ordered transfer of variable-size events

Map lookup can return `NULL`, so check pointers before dereference.

## 6. Verifier

The verifier performs static analysis to prove safety constraints. It does not “guess” whether code is probably safe.
Common requirements include:

- pointer provenance is known
- memory access ranges are provable
- map lookup and ring buffer reserve results are null-checked
- loops have explicit bounds
- all paths terminate
- helper signatures/types are correct

Writing eBPF is not about making code look correct; it is about proving correctness to the verifier.

## 7. CO-RE and BTF

BTF describes kernel types. `vmlinux.h` can be generated from the current kernel's BTF.
CO-RE relies on type and field metadata to reduce adaptation work across kernel versions,
but it does not guarantee every program runs on every kernel.

