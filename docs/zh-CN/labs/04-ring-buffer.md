# 实验 4：使用 Ring Buffer 传递事件

预计时间：50 分钟。

## 目标

- 定义结构化事件；
- 理解 reserve、填充、submit 生命周期；
- 观察背压和采样策略。

## 事件结构

选择 `Ringbuf Skeleton` 模板：

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

字段布局是内核和用户态之间的协议。修改字段后，读取端也必须用相同布局解释数据。

## 发送流程

```c
struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
if (!evt) {
  return 0;
}
evt->ts = bpf_ktime_get_ns();
evt->pid = bpf_get_current_pid_tgid() >> 32;
bpf_ringbuf_submit(evt, 0);
```

1. `reserve` 申请一块记录空间；
2. 返回 NULL 表示 Ring Buffer 当前没有足够空间；
3. 程序填写记录；
4. `submit` 使记录对用户态可见；
5. 如果决定放弃记录，应调用 `bpf_ringbuf_discard`。

## 步骤

1. 运行模板，并通过执行命令触发 execve。
2. 在 Events 页面观察结构化事件。
3. 将运行时间设为 20 秒，采样率设为一个保守值。
4. 比较 Ring Buffer 事件与实验 2 中 printk 输出的差异。
5. 卸载程序。

## 故意制造错误

删除 `if (!evt)` 判空后等待编辑器检查。Cyanrex 会在赋值位置提示 reservation 必须判空。
恢复代码后确认诊断消失。

再把 `bpf_ringbuf_submit(evt, 0)` 删除。代码可能仍能编译，但保留的记录没有完成生命周期，
这属于逻辑错误，说明“编译成功”不能替代程序设计审查。

## 扩展任务

增加 CPU 字段：

```c
__u32 cpu;
```

并赋值：

```c
evt->cpu = bpf_get_smp_processor_id();
```

运行后观察事件来自哪些 CPU。

## 验收

- 能解释 reserve/submit/discard；
- 能说明 Ring Buffer 满时为什么不能阻塞等待；
- 能独立增加一个事件字段并正确赋值。
