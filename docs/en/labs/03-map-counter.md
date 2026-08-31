# Lab 3: Count with an eBPF Map

Estimated time: 45 minutes.

## Objective

- Understand map key/value model.
- Use Per-CPU Array to reduce contention.
- Handle map lookup pointer safely.

## Starting Code

Choose `Ringbuf High-Freq Sampler` and focus on the counter map:

```c
struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} per_cpu_counter SEC(".maps");
```

It stores one `__u64` counter per CPU. The fixed key is `0`.

## Steps

1. Find map lookup code:

```c
__u32 key = 0;
__u64 *counter = bpf_map_lookup_elem(&per_cpu_counter, &key);
if (!counter) {
  return 0;
}
*counter += 1;
```

2. Explain why `counter` is a pointer and why null check is required.
3. Run for 10–20 seconds; observe `count` in Events page.
4. Compare counts from different CPUs. Each CPU is independent in Per-CPU map.
5. Detach program.

## Intentionally introduce error

Temporarily delete:

```c
if (!counter) {
  return 0;
}
```

Observe diagnostics and verifier/load result. Even if the Array key is fixed, helper return value is still nullable; verifier requires explicit path that proves pointer is checked.

Restore null check.

## Mutation Task

Change sampling condition from once every 64 to once every 16:

```c
if ((*counter & 63) != 0)
```

to

```c
if ((*counter & 15) != 0)
```

Compare event count and system load over same runtime. Bitmask works because both 16 and 64 are powers of two.

## Questions

1. What are consistency/performance tradeoffs between Array and Per-CPU Array?
2. Why should high-frequency hooks avoid sending every event to userspace?
3. If you need PID-level counting, choose Array or Hash?

## Acceptance

- Can draw `key -> per-CPU value` relationship.
- Keep correct NULL check.
- Explain sampling impact on throughput.

