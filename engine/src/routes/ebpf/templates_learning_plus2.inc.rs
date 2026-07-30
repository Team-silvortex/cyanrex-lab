fn learning_plus_two_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "xdp-ipv4-tos-meter".to_string(),
            name: "XDP IPv4 TOS Meter".to_string(),
            description: "按 IPv4 TOS 字段统计分布，用于教学理解报文优先级与差异化路由。".to_string(),
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
} tos_count SEC(".maps");

SEC("xdp")
int on_ipv4_tos_meter(struct xdp_md *ctx) {
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

  __u8 key = ip->tos;
  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&tos_count, &key);
  if (!counter) {
    bpf_map_update_elem(&tos_count, &key, &first, BPF_ANY);
    return XDP_PASS;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&tos_count, &key, &next, BPF_ANY);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-udp-large-payload-meter".to_string(),
            name: "XDP UDP Large Payload Meter".to_string(),
            description:
                "统计 UDP 大报文（>1400 字节）流量，展示边界检查后再取 UDP 头部字段的思路。".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 4);
  __type(key, __u8);
  __type(value, __u64);
} udp_payload_count SEC(".maps");

SEC("xdp")
int on_udp_large_payload(struct xdp_md *ctx) {
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
  if (ip->protocol != IPPROTO_UDP) {
    return XDP_PASS;
  }

  __u8 key = 1;
  void *udp_ptr = (void *)((void *)ip + (ip->ihl * 4));
  if (udp_ptr + sizeof(struct udphdr) > data_end) {
    return XDP_PASS;
  }
  __u64 total_len = (unsigned long)data_end - (unsigned long)data;
  if (total_len <= 1400) {
    return XDP_PASS;
  }

  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&udp_payload_count, &key);
  if (!counter) {
    bpf_map_update_elem(&udp_payload_count, &key, &first, BPF_ANY);
    return XDP_PASS;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&udp_payload_count, &key, &next, BPF_ANY);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-close-by-uid".to_string(),
            name: "Kprobe Close By UID".to_string(),
            description:
                "按 UID 聚合 close 调用频率，快速区分不同用户身份下的文件关闭行为。".to_string(),
            capability: "kprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 2048);
  __type(key, __u32);
  __type(value, __u64);
} close_by_uid SEC(".maps");

SEC("kprobe/__x64_sys_close")
int on_close_by_uid(struct pt_regs *ctx) {
  __u32 uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&close_by_uid, &uid);
  if (!counter) {
    bpf_map_update_elem(&close_by_uid, &uid, &first, BPF_ANY);
    return 0;
  }
  __u64 next = *counter + 1;
  bpf_map_update_elem(&close_by_uid, &uid, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-close-latency-beacon".to_string(),
            name: "Kretprobe Close Latency Beacon".to_string(),
            description:
                "对 close 入/出钩子采样（超过 1ms），教学 ringbuf 事件结构化输出。".to_string(),
            capability: "kretprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct close_event_t {
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
} close_start_ns SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} close_latency_events SEC(".maps");

SEC("kprobe/__x64_sys_close")
int on_close_enter(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 now = bpf_ktime_get_ns();
  bpf_map_update_elem(&close_start_ns, &key, &now, BPF_ANY);
  return 0;
}

SEC("kretprobe/__x64_sys_close")
int on_close_exit(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 *start = bpf_map_lookup_elem(&close_start_ns, &key);
  if (!start) {
    return 0;
  }

  __u64 elapsed = bpf_ktime_get_ns() - *start;
  bpf_map_delete_elem(&close_start_ns, &key);
  if (elapsed < 1000000ULL) {
    return 0;
  }

  struct close_event_t *evt = bpf_ringbuf_reserve(&close_latency_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->elapsed_ns = elapsed;
  evt->pid = (__u32)(key >> 32);
  evt->uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  evt->ret = PT_REGS_RC(ctx);
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "tracepoint-sched-process-exec-beacon".to_string(),
            name: "Tracepoint Process Exec Beacon".to_string(),
            description:
                "tracepoint 示例：sched_process_exec 采样上报，展示低侵入观测高频事件。".to_string(),
            capability: "tracepoint".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct exec_event_t {
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
} exec_beacon_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} exec_beacon_events SEC(".maps");

SEC("tracepoint/sched/sched_process_exec")
int on_sched_process_exec_beacon(void *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&exec_beacon_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 63) != 0) {
    return 0;
  }

  struct exec_event_t *evt = bpf_ringbuf_reserve(&exec_beacon_events, sizeof(*evt), 0);
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
