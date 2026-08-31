# Lab 2: Observe `execve` System Call

Estimated time: 35 minutes.

## Objective

- Observe kernel events with tracepoint.
- Understand purpose and limits of `bpf_printk`.
- Verify emitted output in Events page.

## Steps

1. Choose `Tracepoint Sys Enter` template.
2. Locate:

```c
SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(void *ctx) {
  bpf_printk("execve entered");
  return 0;
}
```

3. Predict which operations should trigger this program.
4. Wait for clang status to become `passed`, then click **Compile and Run**.
5. In another terminal, run:

```bash
/usr/bin/true
/usr/bin/id
```

6. Open Events page, filter by kernel category, and inspect sampled events.
7. Return to eBPF page and detach.

## Mutation Task

Change log to:

```c
__u64 id = bpf_get_current_pid_tgid();
bpf_printk("execve pid=%d", (__u32)(id >> 32));
```

Run again and compare event content. The upper 32 bits are TGID, usually visible as user-visible process id.

## Discussion

`bpf_printk` is useful for teaching and temporary debugging, but not suitable as high-frequency production channel:

- limited format and throughput
- relies on tracing pipeline
- high output rate increases overhead
- outputs from multiple programs may be mixed

Use Ring Buffer for structured high-frequency data.

## Questions

1. Why is tracepoint generally more stable than kprobe?
2. Why can program context be `void *` in this minimal example?
3. If events never appear, should you check compile, load, attach, or event filters first?

## Acceptance

- Trigger at least one execve event.
- Explain basic PID/TGID difference.
- Explain why `bpf_printk` is not suitable for high-frequency streaming.

