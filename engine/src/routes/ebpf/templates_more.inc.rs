fn extra_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "ringbuf-process-fork-beacon".to_string(),
            name: "Ringbuf Process Fork Beacon".to_string(),
            description:
                "进程创建观测：tracepoint 下采集 fork 事件（当前 PID、cpu 与 comm）".to_string(),
            capability: "ringbuf".to_string(),
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
    ]
}
