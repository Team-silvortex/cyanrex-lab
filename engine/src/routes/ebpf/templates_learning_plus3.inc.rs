fn learning_plus_three_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "xdp-icmp-type-meter".to_string(),
            name: "XDP ICMP Type Meter".to_string(),
            description: "按 ICMP Type 统计报文数量，用于快速定位探测与异常流量类型".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 64);
  __type(key, __u8);
  __type(value, __u64);
} icmp_type_count SEC(".maps");

SEC("xdp")
int on_icmp_type_meter(struct xdp_md *ctx) {
  void *data = (void *)(long)ctx->data;
  void *data_end = (void *)(long)ctx->data_end;

  struct ethhdr *eth = data;
  if ((void *)(eth + 1) > data_end) {
    return XDP_PASS;
  }
  if (eth->h_proto != bpf_htons(ETH_P_IP)) {
    return XDP_PASS;
  }

  struct iphdr *ip = (void *)(eth + 1);
  if ((void *)(ip + 1) > data_end) {
    return XDP_PASS;
  }
  if (ip->protocol != IPPROTO_ICMP) {
    return XDP_PASS;
  }

  void *icmp_ptr = (void *)((void *)ip + (ip->ihl * 4));
  if (icmp_ptr + sizeof(struct icmphdr) > data_end) {
    return XDP_PASS;
  }
  struct icmphdr *icmp = (struct icmphdr *)icmp_ptr;

  __u8 key = icmp->type;
  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&icmp_type_count, &key);
  if (!counter) {
    bpf_map_update_elem(&icmp_type_count, &key, &first, BPF_ANY);
    return XDP_PASS;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&icmp_type_count, &key, &next, BPF_ANY);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-mmap-len-band".to_string(),
            name: "Kprobe Mmap Length Band".to_string(),
            description:
                "按 mmap 长度分桶统计调用次数，演示参数解析与异常长度分层。".to_string(),
            capability: "kprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 128);
  __type(key, __u32);
  __type(value, __u64);
} mmap_len_band SEC(".maps");

static __always_inline __u32 classify_len(unsigned long len) {
  if (len <= 1024) {
    return 0;
  }
  if (len <= 4096) {
    return 1;
  }
  if (len <= 16384) {
    return 2;
  }
  return 3;
}

SEC("kprobe/__x64_sys_mmap")
int on_mmap_len_band(struct pt_regs *ctx) {
  unsigned long len = PT_REGS_PARM2(ctx);
  __u32 key = classify_len(len);
  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&mmap_len_band, &key);
  if (!counter) {
    bpf_map_update_elem(&mmap_len_band, &key, &first, BPF_ANY);
    return 0;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&mmap_len_band, &key, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-accept-latency-beacon".to_string(),
            name: "Kretprobe Accept Latency Beacon".to_string(),
            description:
                "accept 系统调用延迟观测（超过 2ms）示例，适合演示连接建立慢路径识别。".to_string(),
            capability: "kretprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct accept_latency_event_t {
  __u64 ts;
  __u32 pid;
  __u32 uid;
  __u64 elapsed_ns;
  __s64 ret;
};

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 4096);
  __type(key, __u64);
  __type(value, __u64);
} accept_start_ns SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} accept_latency_events SEC(".maps");

SEC("kprobe/__x64_sys_accept")
int on_accept_enter(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 now = bpf_ktime_get_ns();
  bpf_map_update_elem(&accept_start_ns, &key, &now, BPF_ANY);
  return 0;
}

SEC("kretprobe/__x64_sys_accept")
int on_accept_exit(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 *start = bpf_map_lookup_elem(&accept_start_ns, &key);
  if (!start) {
    return 0;
  }

  __u64 elapsed = bpf_ktime_get_ns() - *start;
  bpf_map_delete_elem(&accept_start_ns, &key);
  if (elapsed < 2ULL * 1000ULL * 1000ULL) {
    return 0;
  }

  struct accept_latency_event_t *evt = bpf_ringbuf_reserve(&accept_latency_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->pid = (__u32)(key >> 32);
  evt->uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  evt->elapsed_ns = elapsed;
  evt->ret = PT_REGS_RC(ctx);
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "ringbuf-sched-switch-beacon".to_string(),
            name: "Ringbuf Sched Switch Beacon".to_string(),
            description: "基于 sched_switch tracepoint 的 ringbuf 采样模板，观察调度切换事件（低开销）".to_string(),
            capability: "ringbuf".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct switch_event_t {
  __u64 ts;
  __u64 sample_no;
  __u32 pid;
  __u32 uid;
  __u32 cpu;
};

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} switch_beacon_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} switch_beacon_events SEC(".maps");

SEC("tracepoint/sched/sched_switch")
int on_sched_switch_beacon(void *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&switch_beacon_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 127) != 0) {
    return 0;
  }

  struct switch_event_t *evt = bpf_ringbuf_reserve(&switch_beacon_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->sample_no = *counter;
  evt->pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  evt->uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  evt->cpu = bpf_get_smp_processor_id();
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "tracepoint-sys-enter-nanosleep-beacon".to_string(),
            name: "Tracepoint Sys_enter_nanosleep Beacon".to_string(),
            description:
                "sys_enter_nanosleep 采样模板（每 128 次）用于讲高频系统调用 tracepoint 降噪".to_string(),
            capability: "tracepoint".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct nanosleep_event_t {
  __u64 ts;
  __u64 sample_no;
  __u32 pid;
  __u32 uid;
  __u32 cpu;
};

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} nanosleep_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} nanosleep_events SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_nanosleep")
int on_trace_nanosleep(void *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&nanosleep_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 127) != 0) {
    return 0;
  }

  struct nanosleep_event_t *evt = bpf_ringbuf_reserve(&nanosleep_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->sample_no = *counter;
  evt->pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  evt->uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  evt->cpu = bpf_get_smp_processor_id();
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
    ]
}
