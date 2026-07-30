fn learning_plus_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "xdp-ttl-meter".to_string(),
            name: "XDP IPv4 TTL Meter".to_string(),
            description:
                "按 IPv4 TTL 字段统计分布（0~255），用于讲报文字段可观测化建模".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 256);
  __type(key, __u8);
  __type(value, __u64);
} ttl_count SEC(".maps");

SEC("xdp")
int on_ipv4_ttl(struct xdp_md *ctx) {
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

  __u8 key = ip->ttl;
  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&ttl_count, &key);
  if (!counter) {
    bpf_map_update_elem(&ttl_count, &key, &first, BPF_ANY);
    return XDP_PASS;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&ttl_count, &key, &next, BPF_ANY);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-tcp-rst-beacon".to_string(),
            name: "XDP TCP RST Beacon".to_string(),
            description:
                "XDP 中采样 TCP RST 报文，教学演示高频控制报文的采样上报".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct rst_event_t {
  __u64 ts;
  __u32 src;
  __u32 dst;
  __u16 sport;
  __u16 dport;
};

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} rst_sample_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} rst_events SEC(".maps");

SEC("xdp")
int on_tcp_rst(struct xdp_md *ctx) {
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
  if (ip->protocol != IPPROTO_TCP) {
    return XDP_PASS;
  }

  void *tcp_ptr = (void *)ip + (ip->ihl * 4);
  if (tcp_ptr + sizeof(struct tcphdr) > data_end) {
    return XDP_PASS;
  }
  struct tcphdr *tcp = tcp_ptr;
  __u8 flags = ((__u8 *)tcp)[13];
  if ((flags & 0x04) == 0) {
    return XDP_PASS;
  }

  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&rst_sample_counter, &key);
  if (!counter) {
    return XDP_PASS;
  }
  *counter += 1;
  if ((*counter & 63) != 0) {
    return XDP_PASS;
  }

  struct rst_event_t *evt = bpf_ringbuf_reserve(&rst_events, sizeof(*evt), 0);
  if (!evt) {
    return XDP_PASS;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->src = ip->saddr;
  evt->dst = ip->daddr;
  evt->sport = bpf_ntohs(tcp->source);
  evt->dport = bpf_ntohs(tcp->dest);
  bpf_ringbuf_submit(evt, 0);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-write-fd-band".to_string(),
            name: "Kprobe Write FD Band".to_string(),
            description:
                "按 write 文件描述符分桶统计写调用频次，教学聚焦系统调用参数下钻思路".to_string(),
            capability: "kprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 64);
  __type(key, __u32);
  __type(value, __u64);
} write_fd_band SEC(".maps");

static __always_inline __u32 fd_bucket(long fd) {
  if (fd < 0) {
    return 63;
  }
  long normalized = fd + 1;
  if (normalized > 63) {
    return 63;
  }
  return ( __u32)normalized;
}

SEC("kprobe/__x64_sys_write")
int on_write_fd_band(struct pt_regs *ctx) {
  long fd = (long)PT_REGS_PARM1(ctx);
  __u32 key = fd_bucket(fd);
  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&write_fd_band, &key);
  if (!counter) {
    bpf_map_update_elem(&write_fd_band, &key, &first, BPF_ANY);
    return 0;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&write_fd_band, &key, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-write-latency-beacon".to_string(),
            name: "Kretprobe Write Latency Beacon".to_string(),
            description:
                "对 write 延迟做入/出钩子采样（超过 2ms）并输出 ringbuf 事件".to_string(),
            capability: "kretprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct write_latency_event_t {
  __u64 ts;
  __u64 elapsed_ns;
  __u32 pid;
  __s64 ret;
};

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 2048);
  __type(key, __u64);
  __type(value, __u64);
} write_start_ns SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} write_latency_events SEC(".maps");

SEC("kprobe/__x64_sys_write")
int on_write_enter(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 now = bpf_ktime_get_ns();
  bpf_map_update_elem(&write_start_ns, &key, &now, BPF_ANY);
  return 0;
}

SEC("kretprobe/__x64_sys_write")
int on_write_exit(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 *start = bpf_map_lookup_elem(&write_start_ns, &key);
  if (!start) {
    return 0;
  }

  __u64 elapsed = bpf_ktime_get_ns() - *start;
  bpf_map_delete_elem(&write_start_ns, &key);
  if (elapsed < 2ULL * 1000ULL * 1000ULL) {
    return 0;
  }

  struct write_latency_event_t *evt = bpf_ringbuf_reserve(&write_latency_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->elapsed_ns = elapsed;
  evt->pid = (__u32)(key >> 32);
  evt->ret = PT_REGS_RC(ctx);
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "tracepoint-sys_enter_read-sample-beacon".to_string(),
            name: "Tracepoint Read Sample Beacon".to_string(),
            description:
                "tracepoint 采样示例：sys_enter_read 每 96 次上报一次事件，练习高频采样抑制".to_string(),
            capability: "tracepoint".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct read_sample_event_t {
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
} read_beacon_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} read_beacon_events SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_read")
int on_trace_read_enter(void *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&read_beacon_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 95) != 0) {
    return 0;
  }

  struct read_sample_event_t *evt = bpf_ringbuf_reserve(&read_beacon_events, sizeof(*evt), 0);
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
