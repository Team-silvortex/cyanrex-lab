fn default_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "xdp-pass".to_string(),
            name: "XDP Pass".to_string(),
            description: "最小 XDP 程序，适合验证编译/加载链路".to_string(),
            capability: "xdp".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("xdp")
int xdp_pass(struct xdp_md *ctx) {
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "tracepoint-sys-enter".to_string(),
            name: "Tracepoint Sys Enter".to_string(),
            description: "tracepoint 事件，输出内核日志（可在 events 查看采样流）".to_string(),
            capability: "tracepoint".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(void *ctx) {
  bpf_printk("execve entered");
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "ringbuf-skeleton".to_string(),
            name: "Ringbuf Skeleton".to_string(),
            description: "ringbuf 结构模板（用户态 reader 可按此 map 进行消费）".to_string(),
            capability: "ringbuf".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct event_t {
  __u64 ts;
  __u32 pid;
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} events SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(void *ctx) {
  struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->pid = bpf_get_current_pid_tgid() >> 32;
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "ringbuf-hi-freq-sampler".to_string(),
            name: "Ringbuf High-Freq Sampler".to_string(),
            description:
                "高频 tracepoint 切面 + 内核侧采样节流（默认每 64 次上报 1 次），用于展示事件流能力且不干扰系统".to_string(),
            capability: "ringbuf".to_string(),
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct event_t {
  __u64 ts;
  __u64 count;
  __u32 pid;
  __u32 cpu;
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} events SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} per_cpu_counter SEC(".maps");

SEC("tracepoint/sched/sched_switch")
int on_sched_switch(struct trace_event_raw_sched_switch *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&per_cpu_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 63) != 0) {
    return 0;
  }

  struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->count = *counter;
  evt->pid = ctx->next_pid;
  evt->cpu = bpf_get_smp_processor_id();
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
    ]
}
