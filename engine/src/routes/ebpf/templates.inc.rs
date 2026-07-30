fn base_templates() -> Vec<EbpfTemplate> {
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
            id: "tracepoint-execve-counter".to_string(),
            name: "Tracepoint Execve Counter".to_string(),
            description:
                "按 PID 统计 execve 调用次数（展示 Hash Map + 空指针检查 + 原子更新思路）".to_string(),
            capability: "tracepoint".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 4096);
  __type(key, __u32);
  __type(value, __u64);
} exec_count_by_pid SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve_enter(void *ctx) {
  __u32 pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  __u64 first = 1;
  __u64 *count = bpf_map_lookup_elem(&exec_count_by_pid, &pid);
  if (!count) {
    bpf_map_update_elem(&exec_count_by_pid, &pid, &first, BPF_ANY);
    return 0;
  }
  __u64 next = *count + 1;
  bpf_map_update_elem(&exec_count_by_pid, &pid, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-openat-counter".to_string(),
            name: "Kprobe Openat Counter".to_string(),
            description:
                "kprobe 入口示例：统计每个进程 openat 系统调用次数（演示 pt_regs 钩子与哈希统计）".to_string(),
            capability: "kprobe".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 2048);
  __type(key, __u32);
  __type(value, __u64);
} openat_count_by_pid SEC(".maps");

SEC("kprobe/__x64_sys_openat")
int on_openat_enter(struct pt_regs *ctx) {
  __u32 pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  __u64 first = 1;
  __u64 *count = bpf_map_lookup_elem(&openat_count_by_pid, &pid);
  if (!count) {
    bpf_map_update_elem(&openat_count_by_pid, &pid, &first, BPF_ANY);
    return 0;
  }
  __u64 next = *count + 1;
  bpf_map_update_elem(&openat_count_by_pid, &pid, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-openat-argv".to_string(),
            name: "Kprobe Openat Args".to_string(),
            description:
                "kprobe 获取 openat 调用参数：读取 pathname 指针并展示参数读法（只读，不阻塞流程）".to_string(),
            capability: "kprobe".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u32);
} total_openat SEC(".maps");

SEC("kprobe/__x64_sys_openat")
int on_openat_args(struct pt_regs *ctx) {
  __u32 key = 0;
  __u32 *count = bpf_map_lookup_elem(&total_openat, &key);
  if (!count) {
    return 0;
  }

  (*count) += 1;

  const char *pathname = (const char *)PT_REGS_PARM2(ctx);
  char buf[48];
  int bytes = bpf_probe_read_user_str(buf, sizeof(buf), pathname);
  if (bytes > 0) {
    /* 课堂示例：仅在源码层演示参数读取路径；实际生产建议加长度与安全策略。 */
    bpf_printk("openat[%d] args path=%s flags=0x%x", bpf_get_current_pid_tgid() >> 32, buf,
               (__u32)PT_REGS_PARM3(ctx));
  }
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-openat-ret".to_string(),
            name: "Kretprobe Openat Return".to_string(),
            description: "kretprobe 返还值示例：区分 openat 成功/失败返回并统计".to_string(),
            capability: "kretprobe".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 2);
  __type(key, __u32);
  __type(value, __u64);
} openat_ret_count SEC(".maps");

SEC("kretprobe/__x64_sys_openat")
int on_openat_ret(struct pt_regs *ctx) {
  __s64 ret = PT_REGS_RC(ctx);
  __u32 key = ret >= 0 ? 0 : 1;
  __u64 *count = bpf_map_lookup_elem(&openat_ret_count, &key);
  if (!count) {
    return 0;
  }
  *count += 1;
  if (ret < 0) {
    bpf_printk("openat failed: %lld", ret);
  }
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kprobe-connect-counter".to_string(),
            name: "Kprobe Connect Counter".to_string(),
            description:
                "kprobe 示例：统计 connect 系统调用的调用次数（适合讲网络调试中的系统调用入侵面）".to_string(),
            capability: "kprobe".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 1024);
  __type(key, __u32);
  __type(value, __u64);
} connect_count_by_pid SEC(".maps");

SEC("kprobe/__x64_sys_connect")
int on_connect_enter(struct pt_regs *ctx) {
  __u32 pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  __u64 first = 1;
  __u64 *count = bpf_map_lookup_elem(&connect_count_by_pid, &pid);
  if (!count) {
    bpf_map_update_elem(&connect_count_by_pid, &pid, &first, BPF_ANY);
    return 0;
  }
  __u64 next = *count + 1;
  bpf_map_update_elem(&connect_count_by_pid, &pid, &next, BPF_ANY);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "kretprobe-connect-ret".to_string(),
            name: "Kretprobe Connect Return".to_string(),
            description:
                "kretprobe 返还值示例：区分 connect 成功/失败，并做基础错误码计数".to_string(),
            capability: "kretprobe".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 3);
  __type(key, __u32);
  __type(value, __u64);
} connect_ret_count SEC(".maps");

SEC("kretprobe/__x64_sys_connect")
int on_connect_ret(struct pt_regs *ctx) {
  __s64 ret = PT_REGS_RC(ctx);
  __u32 key = ret == 0 ? 0 : (ret == -115 ? 1 : 2); // 成功/超时/其他失败
  __u64 *count = bpf_map_lookup_elem(&connect_ret_count, &key);
  if (!count) {
    return 0;
  }
  *count += 1;
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
            id: "xdp-packet-counter".to_string(),
            name: "XDP Packet Counter".to_string(),
            description: "XDP 教学版：统计每 CPU 收到的数据包数（使用 per-cpu 数组，便于性能分析和速率看板）".to_string(),
            capability: "xdp".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __type(key, __u32);
  __type(value, __u64);
} rx_packets_by_cpu SEC(".maps");

SEC("xdp")
int xdp_packet_counter(struct xdp_md *ctx) {
  __u32 key = 0;
  __u64 *count = bpf_map_lookup_elem(&rx_packets_by_cpu, &key);
  if (!count) {
    return XDP_PASS;
  }
  *count += 1;
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-tcp4-counter".to_string(),
            name: "XDP TCPv4 Counter".to_string(),
            description: "XDP 过滤版：仅统计 IPv4 + TCP 报文数量（教学：解析 L2/L3 字段 + 过滤条件）".to_string(),
            capability: "xdp".to_string(),
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 64);
  __type(key, __u16);
  __type(value, __u64);
} tcp4_proto_count SEC(".maps");

SEC("xdp")
int xdp_tcp4_counter(struct xdp_md *ctx) {
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

  __u16 key = ip->protocol;
  __u64 first = 1;
  __u64 *cnt = bpf_map_lookup_elem(&tcp4_proto_count, &key);
  if (!cnt) {
    bpf_map_update_elem(&tcp4_proto_count, &key, &first, BPF_ANY);
    return XDP_PASS;
  }
  __u64 next = *cnt + 1;
  bpf_map_update_elem(&tcp4_proto_count, &key, &next, BPF_ANY);
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "xdp-icmp-pass-scope".to_string(),
            name: "XDP ICMP Scope".to_string(),
            description:
                "XDP 解析样例：识别 ICMP 报文并统计，其他报文仍放行（示例：最小协议解析）".to_string(),
            capability: "xdp".to_string(),
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 4);
  __type(key, __u16);
  __type(value, __u64);
} icmp_proto_count SEC(".maps");

SEC("xdp")
int xdp_icmp_scope(struct xdp_md *ctx) {
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

  __u16 key = ip->protocol;
  __u64 first = 1;
  __u64 *count = bpf_map_lookup_elem(&icmp_proto_count, &key);
  if (!count) {
    bpf_map_update_elem(&icmp_proto_count, &key, &first, BPF_ANY);
    return XDP_PASS;
  }
  __u64 next = *count + 1;
  bpf_map_update_elem(&icmp_proto_count, &key, &next, BPF_ANY);
  return XDP_PASS;
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
        EbpfTemplate {
            id: "ringbuf-syscall-beacon".to_string(),
            name: "Ringbuf Syscall Beacon".to_string(),
            description:
                "轻量采样模板：每 32 次 execve 事件上报一次，附带当前 PID 和采样计数".to_string(),
            capability: "ringbuf".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct event_t {
  __u64 ts;
  __u64 sample_no;
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
} sample_counter SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve_sample(struct trace_event_raw_sys_enter *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&sample_counter, &key);
  if (!counter) {
    return 0;
  }
  *counter += 1;
  if ((*counter & 31) != 0) {
    return 0;
  }

  struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->sample_no = *counter;
  evt->pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  evt->cpu = bpf_get_smp_processor_id();
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "ringbuf-process-beacon".to_string(),
            name: "Ringbuf Process Beacon".to_string(),
            description:
                "进程级事件模板：每次 sched_process_exit 上报一次事件，带 pid/uid/进程名与时间戳".to_string(),
            capability: "ringbuf".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

struct event_t {
  __u64 ts;
  __u32 pid;
  __u32 uid;
  char comm[16];
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} events SEC(".maps");

SEC("tracepoint/sched/sched_process_exit")
int on_sched_process_exit(void *ctx) {
  struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
  evt->uid = (__u32)(bpf_get_current_uid_gid() & 0xffffffff);
  bpf_get_current_comm(evt->comm, sizeof(evt->comm));
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
    ]
}

include!("templates_more.inc.rs");

fn default_templates() -> Vec<EbpfTemplate> {
    let mut templates = base_templates();
    templates.extend(extra_templates());
    templates
}
