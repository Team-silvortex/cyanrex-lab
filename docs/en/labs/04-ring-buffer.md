# Lab 4: Send Events with Ring Buffer

Estimated time: 50 minutes.

## Objective

- Define structured event types.
- Understand reserve/fill/submit lifecycle.
- Observe backpressure and sampling strategy.

## Event Structure

Choose `Ringbuf Skeleton` template:

```c
struct event_t {
  __u64 ts;
  __u32 pid;
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} events SEC(".maps");
```

The field layout is the protocol between kernel and userspace. When changing fields, reader logic must use same layout.

## Send Flow

```c
struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
if (!evt) {
  return 0;
}
evt->ts = bpf_ktime_get_ns();
evt->pid = bpf_get_current_pid_tgid() >> 32;
bpf_ringbuf_submit(evt, 0);
```

1. `reserve` allocates event space.
2. `NULL` means no space in ring buffer.
3. Fill record.
4. `submit` makes record visible to userspace.
5. If discarding, call `bpf_ringbuf_discard`.

## Steps

1. Run template and trigger execve using shell commands.
2. Observe structured events in Events page.
3. Set run time to 20 seconds and set sampling conservatively.
4. Compare ring buffer events with lab 2 printk output.
5. Detach program.

## Intentionally introduce error

Remove `if (!evt)` null-check and wait for editor diagnostics. Cyanrex should flag reserve result usage without null-check.
Restore check and confirm diagnostics vanish.

Then remove `bpf_ringbuf_submit(evt, 0)`. Code may still compile, but reserved record is never finalized in lifecycle,
which is a logic error. “Compile passed” does not equal correct design.

## Extension Task

Add CPU field:

```c
__u32 cpu;
```

and assignment:

```c
evt->cpu = bpf_get_smp_processor_id();
```

Run and observe events from which CPUs.

## Acceptance

- Explain reserve/submit/discard.
- Explain why ring buffer cannot wait/block when full.
- Add a new event field correctly and assign it.

