fn learning_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "xdp-ipv4-protocol-meter".to_string(),
            name: "XDP IPv4 Protocol Meter".to_string(),
            description:
                "按 IPv4 协议号计数，演示安全地读取 L2/L3 并更新 hash map，便于讲采集维度建模".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 16);
  __type(key, __u16);
  __type(value, __u64);
} ipv4_proto_count SEC(".maps");

SEC("xdp")
int on_ip_protocol(struct xdp_md *ctx) {
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

  __u16 key = ip->protocol;
  __u64 first = 1;
  __u64 *count = bpf_map_lookup_elem(&ipv4_proto_count, &key);
  if (!count) {
    bpf_map_update_elem(&ipv4_proto_count, &key, &first, BPF_ANY);
    return XDP_PASS;
  }
  __u64 next = *count + 1;
  bpf_map_update_elem(&ipv4_proto_count, &key, &next, BPF_ANY);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-large-packet-counter".to_string(),
            name: "XDP Large Packet Counter".to_string(),
            description:
                "统计大报文（> 1024 字节）流量，演示长度判断 + 按 CPU 缓存友好的统计思路".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} large_packet_count SEC(".maps");

SEC("xdp")
int on_large_packet(struct xdp_md *ctx) {
  void *data = (void *)(long)ctx->data;
  void *data_end = (void *)(long)ctx->data_end;
  __u64 packet_len = (unsigned long)data_end - (unsigned long)data;

  if (packet_len <= 1024) {
    return XDP_PASS;
  }

  __u32 key = 0;
  __u64 *count = bpf_map_lookup_elem(&large_packet_count, &key);
  if (!count) {
    return XDP_PASS;
  }

  (*count) += 1;
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-openat-by-uid".to_string(),
            name: "Kprobe Openat by UID".to_string(),
            description:
                "按 UID 聚合 openat 调用频率，观察用户维度系统调用行为与噪音差异".to_string(),
            capability: "kprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 2048);
  __type(key, __u32);
  __type(value, __u64);
} openat_by_uid SEC(".maps");

SEC("kprobe/__x64_sys_openat")
int on_openat_by_uid(struct pt_regs *ctx) {
  __u32 uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  __u64 first = 1;
  __u64 *counter = bpf_map_lookup_elem(&openat_by_uid, &uid);
  if (!counter) {
    bpf_map_update_elem(&openat_by_uid, &uid, &first, BPF_ANY);
    return 0;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&openat_by_uid, &uid, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-read-latency-beacon".to_string(),
            name: "Kretprobe Read Latency Beacon".to_string(),
            description:
                "对 `read` 系统调用做轻量延迟采样（超过 2ms 则上报），同时演示入/出钩子配对时序".to_string(),
            capability: "kretprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 2048);
  __type(key, __u64);
  __type(value, __u64);
} read_start_ns SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} read_events SEC(".maps");

struct read_latency_event_t {
  __u64 ts;
  __u32 pid;
  __u64 elapsed_ns;
  __s64 ret;
};

SEC("kprobe/__x64_sys_read")
int on_read_enter(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 now = bpf_ktime_get_ns();
  bpf_map_update_elem(&read_start_ns, &key, &now, BPF_ANY);
  return 0;
}

SEC("kretprobe/__x64_sys_read")
int on_read_exit(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 *start = bpf_map_lookup_elem(&read_start_ns, &key);
  if (!start) {
    return 0;
  }

  __u64 elapsed = bpf_ktime_get_ns() - *start;
  bpf_map_delete_elem(&read_start_ns, &key);

  if (elapsed < 2ULL * 1000ULL * 1000ULL) {
    return 0;
  }

  struct read_latency_event_t *evt = bpf_ringbuf_reserve(&read_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->pid = (__u32)(key >> 32);
  evt->elapsed_ns = elapsed;
  evt->ret = PT_REGS_RC(ctx);
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "ringbuf-openat-latency-beacon".to_string(),
            name: "Ringbuf Openat Latency Beacon".to_string(),
            description:
                "在 ringbuf 里采集 openat 延迟样本（超过 1ms），用于教学演示事件流字段与过滤策略".to_string(),
            capability: "ringbuf".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 4096);
  __type(key, __u64);
  __type(value, __u64);
} openat_start_ts SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} openat_events SEC(".maps");

struct openat_event_t {
  __u64 ts;
  __u32 pid;
  __u32 uid;
  __u64 elapsed_ns;
  __s64 ret;
};

SEC("kprobe/__x64_sys_openat")
int on_openat_enter(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 now = bpf_ktime_get_ns();
  bpf_map_update_elem(&openat_start_ts, &key, &now, BPF_ANY);
  return 0;
}

SEC("kretprobe/__x64_sys_openat")
int on_openat_exit(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 *start = bpf_map_lookup_elem(&openat_start_ts, &key);
  if (!start) {
    return 0;
  }

  __u64 elapsed = bpf_ktime_get_ns() - *start;
  bpf_map_delete_elem(&openat_start_ts, &key);

  if (elapsed < 1000000ULL) {
    return 0;
  }

  struct openat_event_t *evt = bpf_ringbuf_reserve(&openat_events, sizeof(*evt), 0);
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
            id: "xdp-dns-udp4-meter".to_string(),
            name: "XDP DNS UDP4 Meter".to_string(),
            description:
                "统计 UDP/53 报文按目的端口分布（只读 IPv4/L4）".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 64);
  __type(key, __u16);
  __type(value, __u64);
} dns_port_count SEC(".maps");

SEC("xdp")
int on_dns_udp4(struct xdp_md *ctx) {
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

  void *udp_ptr = (void *)((void *)ip + (ip->ihl * 4));
  if (udp_ptr + sizeof(struct udphdr) > data_end) {
    return XDP_PASS;
  }
  struct udphdr *udp = udp_ptr;
  if (udp->dest == bpf_htons(53)) {
    __u16 key = bpf_ntohs(udp->dest);
    __u64 first = 1;
    __u64 *count = bpf_map_lookup_elem(&dns_port_count, &key);
    if (!count) {
      bpf_map_update_elem(&dns_port_count, &key, &first, BPF_ANY);
      return XDP_PASS;
    }

    __u64 next = *count + 1;
    bpf_map_update_elem(&dns_port_count, &key, &next, BPF_ANY);
  }
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-ipv4-frag-sampler".to_string(),
            name: "XDP IPv4 Fragment Sampler".to_string(),
            description: "对 IPv4 分片包做采样上报，演示协议位域判断与安全的报文边界检查".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct frag_event_t {
  __u32 dst;
  __u32 src;
  __u16 frag_off;
  __u64 ts;
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} frag_events SEC(".maps");

SEC("xdp")
int on_ipv4_fragment(struct xdp_md *ctx) {
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
  __u16 frag = bpf_ntohs(ip->frag_off);
  if ((frag & 0x3fff) == 0) {
    return XDP_PASS;
  }

  struct frag_event_t *evt = bpf_ringbuf_reserve(&frag_events, sizeof(*evt), 0);
  if (!evt) {
    return XDP_PASS;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->src = ip->saddr;
  evt->dst = ip->daddr;
  evt->frag_off = frag;
  bpf_ringbuf_submit(evt, 0);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-sendto-bytes-band".to_string(),
            name: "Kprobe Sendto Bytes Band".to_string(),
            description:
                "按 sendto 长度分桶统计调用次数，帮助从频率和流量角度快速定位流量特征".to_string(),
            capability: "kprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 64);
  __type(key, __u32);
  __type(value, __u64);
} sendto_band SEC(".maps");

static __always_inline __u32 classify_send_len(void *ctx) {
  unsigned long len = PT_REGS_PARM3(ctx);
  __u32 band = (len / 1024) + 1;
  if (band > 64) {
    return 64;
  }
  return band;
}

SEC("kprobe/__x64_sys_sendto")
int on_sendto_band(struct pt_regs *ctx) {
  __u32 key = classify_send_len(ctx);
  __u64 one = 1;
  __u64 *counter = bpf_map_lookup_elem(&sendto_band, &key);
  if (!counter) {
    bpf_map_update_elem(&sendto_band, &key, &one, BPF_ANY);
    return 0;
  }

  __u64 next = *counter + 1;
  bpf_map_update_elem(&sendto_band, &key, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-connect-latency-beacon".to_string(),
            name: "Kretprobe Connect Latency Beacon".to_string(),
            description:
                "对 connect 系统调用做入/出钩子延迟采样，重点观察异常慢连接事件".to_string(),
            capability: "kretprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct connect_event_t {
  __u64 ts;
  __u64 elapsed_ns;
  __u32 pid;
  __u32 uid;
  __s64 ret;
};

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 4096);
  __type(key, __u64);
  __type(value, __u64);
} connect_start_ts SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} connect_events SEC(".maps");

SEC("kprobe/__x64_sys_connect")
int on_connect_enter(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 now = bpf_ktime_get_ns();
  bpf_map_update_elem(&connect_start_ts, &key, &now, BPF_ANY);
  return 0;
}

SEC("kretprobe/__x64_sys_connect")
int on_connect_exit(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 *start = bpf_map_lookup_elem(&connect_start_ts, &key);
  if (!start) {
    return 0;
  }

  __u64 elapsed = bpf_ktime_get_ns() - *start;
  bpf_map_delete_elem(&connect_start_ts, &key);
  if (elapsed < 3ULL * 1000ULL * 1000ULL) {
    return 0;
  }

  struct connect_event_t *evt = bpf_ringbuf_reserve(&connect_events, sizeof(*evt), 0);
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
            id: "tracepoint-openat-sample-beacon".to_string(),
            name: "Tracepoint Openat Sample Beacon".to_string(),
            description:
                "tracepoint 采样模板：每 64 次 sys_enter_openat 上报一次事件（不读取参数，教学用途）".to_string(),
            capability: "tracepoint".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct openat_sample_event_t {
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
} openat_beacon_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} openat_beacon_events SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_openat")
int on_trace_openat(void *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&openat_beacon_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 63) != 0) {
    return 0;
  }

  struct openat_sample_event_t *evt = bpf_ringbuf_reserve(&openat_beacon_events, sizeof(*evt), 0);
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
