fn extra_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "ringbuf-process-fork-beacon".to_string(),
            name: "Ringbuf Process Fork Beacon".to_string(),
            description:
                "进程创建观测：tracepoint 下采集 fork 事件（当前 PID、cpu 与 comm）".to_string(),
            capability: "ringbuf".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct event_t {
  __u64 ts;
  __u32 pid;
  __u32 ppid;
  __u32 cpu;
  char comm[16];
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} events SEC(".maps");

SEC("tracepoint/sched/sched_process_fork")
int on_sched_process_fork(void *ctx) {
  struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  evt->ppid = 0;
  evt->cpu = bpf_get_smp_processor_id();
  bpf_get_current_comm(evt->comm, sizeof(evt->comm));
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-dns-block-sample".to_string(),
            name: "XDP DNS Drop Sample".to_string(),
            description:
                "生产风格示例：解析 L2/L3/L4，仅对 UDP/53 报文做样例丢弃并计数".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct dns_key_t {
  __u8 ip_version;
  __u16 proto;
  __u16 port_be;
};

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 8);
  __type(key, struct dns_key_t);
  __type(value, __u64);
} dns_drop_count SEC(".maps");

static __always_inline int is_dns_query(void *data, void *data_end) {
  struct ethhdr *eth = data;
  if ((void *)(eth + 1) > data_end) {
    return 0;
  }
  if (eth->h_proto != bpf_htons(ETH_P_IP)) {
    return 0;
  }

  struct iphdr *ip = (void *)(eth + 1);
  if ((void *)(ip + 1) > data_end) {
    return 0;
  }

  if (ip->protocol != IPPROTO_UDP) {
    return 0;
  }

  unsigned char *udp_ptr = (void *)((void *)ip + (ip->ihl * 4));
  if (udp_ptr + sizeof(struct udphdr) > (unsigned char *)data_end) {
    return 0;
  }

  struct udphdr *udp = (struct udphdr *)udp_ptr;
  return udp->dest == bpf_htons(53);
}

SEC("xdp")
int xdp_dns_block(struct xdp_md *ctx) {
  void *data = (void *)(long)ctx->data;
  void *data_end = (void *)(long)ctx->data_end;

  if (!is_dns_query(data, data_end)) {
    return XDP_PASS;
  }

  struct dns_key_t key = {
      .ip_version = 4,
      .proto = bpf_htons(IPPROTO_UDP),
      .port_be = bpf_htons(53),
  };

  __u64 first = 1;
  __u64 *count = bpf_map_lookup_elem(&dns_drop_count, &key);
  if (!count) {
    bpf_map_update_elem(&dns_drop_count, &key, &first, BPF_ANY);
  } else {
    __u64 next = *count + 1;
    bpf_map_update_elem(&dns_drop_count, &key, &next, BPF_ANY);
  }

  return XDP_DROP;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-openat-fail-alert".to_string(),
            name: "Kretprobe Openat Fail Alert".to_string(),
            description:
                "异常告警示例：openat 返回失败时上报失败码与当前进程（便于快速识别异常路径）".to_string(),
            capability: "kretprobe".to_string(),
            category: None,
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct fail_event_t {
  __u64 ts;
  __u32 pid;
  __u32 uid;
  __s64 ret;
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} fail_events SEC(".maps");

SEC("kretprobe/__x64_sys_openat")
int on_openat_fail_alert(struct pt_regs *ctx) {
  __s64 ret = PT_REGS_RC(ctx);
  if (ret >= 0) {
    return 0;
  }

  struct fail_event_t *evt = bpf_ringbuf_reserve(&fail_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  evt->uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  evt->ret = ret;
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-tcp-sample-beacon".to_string(),
            name: "XDP TCP Sample Beacon".to_string(),
            description:
                "轻量采样：仅对 TCP 报文每 128 条上报一次事件，保留处理链路不中断".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct sample_event_t {
  __u64 ts;
  __u64 sample_no;
  __u32 cpu;
  __u32 ifindex;
};

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} sample_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} sample_events SEC(".maps");

SEC("xdp")
int on_tcp_beacon(struct xdp_md *ctx) {
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

  __u32 key = 0;
  __u64 *count = bpf_map_lookup_elem(&sample_counter, &key);
  if (!count) {
    return XDP_PASS;
  }
  *count += 1;
  if ((*count & 127) != 0) {
    return XDP_PASS;
  }

  struct sample_event_t *evt = bpf_ringbuf_reserve(&sample_events, sizeof(*evt), 0);
  if (!evt) {
    return XDP_PASS;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->sample_no = *count;
  evt->cpu = bpf_get_smp_processor_id();
  evt->ifindex = ctx->ingress_ifindex;
  bpf_ringbuf_submit(evt, 0);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-tcp-syn-beacon".to_string(),
            name: "XDP TCP SYN Beacon".to_string(),
            description:
                "抓取 TCP SYN 报文的教学模板，按每 64 次 SYN 触发一次采样上报，便于观察握手起点".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct syn_event_t {
  __u64 ts;
  __u32 src;
  __u32 dst;
  __u16 sport;
  __u16 dport;
  __u32 cpu;
  __u64 sample_no;
};

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} syn_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} syn_events SEC(".maps");

static __always_inline int parse_tcp_syn(void *data, void *data_end, __u64 sample_no) {
  struct ethhdr *eth = data;
  if ((void *)(eth + 1) > data_end) {
    return 0;
  }
  if (eth->h_proto != bpf_htons(ETH_P_IP)) {
    return 0;
  }

  struct iphdr *ip = (void *)(eth + 1);
  if ((void *)(ip + 1) > data_end) {
    return 0;
  }
  if (ip->protocol != IPPROTO_TCP) {
    return 0;
  }

  void *tcp_ptr = (void *)ip + (ip->ihl * 4);
  if (tcp_ptr + sizeof(struct tcphdr) > data_end) {
    return 0;
  }
  struct tcphdr *tcp = tcp_ptr;
  __u8 flags = ((__u8 *)tcp)[13];
  if ((flags & 0x02) == 0) {
    return 0;
  }

  struct syn_event_t *evt = bpf_ringbuf_reserve(&syn_events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->src = ip->saddr;
  evt->dst = ip->daddr;
  evt->sport = bpf_ntohs(tcp->source);
  evt->dport = bpf_ntohs(tcp->dest);
  evt->cpu = bpf_get_smp_processor_id();

  evt->sample_no = sample_no;
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

SEC("xdp")
int on_tcp_syn(struct xdp_md *ctx) {
  void *data = (void *)(long)ctx->data;
  void *data_end = (void *)(long)ctx->data_end;

  __u32 key = 0;
  __u64 *sample = bpf_map_lookup_elem(&syn_counter, &key);
  if (!sample) {
    return XDP_PASS;
  }

  *sample += 1;
  if ((*sample & 63) != 0) {
    return XDP_PASS;
  }

  parse_tcp_syn(data, data_end, *sample);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-connect-latency-alert".to_string(),
            name: "Kretprobe Connect Latency Alert".to_string(),
            description:
                "测量 connect 系统调用延迟：超过 5ms 或返回失败时上报事件（用于排查慢连接/阻塞）".to_string(),
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
int on_connect_ret(struct pt_regs *ctx) {
  __u64 key = bpf_get_current_pid_tgid();
  __u64 *start = bpf_map_lookup_elem(&connect_start_ts, &key);
  if (!start) {
    return 0;
  }

  __u64 elapsed = bpf_ktime_get_ns() - *start;
  bpf_map_delete_elem(&connect_start_ts, &key);

  __s64 ret = PT_REGS_RC(ctx);
  if (ret >= 0 && elapsed < 5ULL * 1000ULL * 1000ULL) {
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
  evt->ret = ret;
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "tracepoint-execve-sample-beacon".to_string(),
            name: "Tracepoint Execve Sample Beacon".to_string(),
            description:
                "tracepoint 采样模板：sys_enter_execve 每 64 次上报一次 PID/UID 事件，适合降低高频触发噪音".to_string(),
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
} exec_events SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_execve")
int on_trace_execve(void *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&exec_beacon_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 63) != 0) {
    return 0;
  }

  struct exec_event_t *evt = bpf_ringbuf_reserve(&exec_events, sizeof(*evt), 0);
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
            id: "xdp-icmpv4-sample-beacon".to_string(),
            name: "XDP ICMPv4 Sample Beacon".to_string(),
            description:
                "仅对 IPv4 ICMP 报文采样上报（每 128 条）并保留 PASS，不影响转发主流程，适合观测探测流量".to_string(),
            capability: "xdp".to_string(),
            category: None,
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct icmp_event_t {
  __u64 ts;
  __u32 src;
  __u32 dst;
  __u8 type;
  __u8 code;
  __u8 ttl;
  __u64 sample_no;
};

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} icmp_counter SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} icmp_events SEC(".maps");

SEC("xdp")
int on_icmpv4_beacon(struct xdp_md *ctx) {
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

  void *icmp_ptr = (void *)ip + (ip->ihl * 4);
  if (icmp_ptr + sizeof(struct icmphdr) > data_end) {
    return XDP_PASS;
  }
  struct icmphdr *icmp = icmp_ptr;

  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&icmp_counter, &key);
  if (!counter) {
    return XDP_PASS;
  }
  *counter += 1;
  if ((*counter & 127) != 0) {
    return XDP_PASS;
  }

  struct icmp_event_t *evt = bpf_ringbuf_reserve(&icmp_events, sizeof(*evt), 0);
  if (!evt) {
    return XDP_PASS;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->src = ip->saddr;
  evt->dst = ip->daddr;
  evt->type = icmp->type;
  evt->code = icmp->code;
  evt->ttl = ip->ttl;
  evt->sample_no = *counter;
  bpf_ringbuf_submit(evt, 0);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
    ]
}
