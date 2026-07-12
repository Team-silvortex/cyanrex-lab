# 实验 2：观察 execve 系统调用

预计时间：35 分钟。

## 目标

- 使用 tracepoint 观察内核事件；
- 理解 `bpf_printk` 的用途和局限；
- 在事件中心确认程序产生的输出。

## 步骤

1. 选择 `Tracepoint Sys Enter` 模板。
2. 找到：

```c
SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(void *ctx) {
  bpf_printk("execve entered");
  return 0;
}
```

3. 预测什么操作会触发这个程序。
4. 等待 clang 状态为 `passed`，点击“编译并运行”。
5. 在另一个终端执行几次：

```bash
/usr/bin/true
/usr/bin/id
```

6. 打开 Events 页面，筛选 kernel 类别，观察采样事件。
7. 返回 eBPF 页面并卸载程序。

## 修改任务

将日志修改为：

```c
__u64 id = bpf_get_current_pid_tgid();
bpf_printk("execve pid=%d", (__u32)(id >> 32));
```

重新运行后比较事件内容。这里高 32 位是 TGID，通常对应用户看到的进程 ID。

## 讨论

`bpf_printk` 适合教学和临时调试，但不适合高频生产事件：

- 格式和吞吐能力有限；
- 依赖 tracing 管道；
- 高频输出会增加系统开销；
- 多个程序的输出可能混在一起。

结构化、高频数据应使用 Ring Buffer。

## 思考题

1. Tracepoint 为什么通常比 kprobe 更稳定？
2. 为什么程序 context 在这个最小示例里可以写成 `void *`？
3. 如果事件没有出现，应先检查 compile、load、attach 还是 Events 过滤条件？

## 验收

- 能触发至少一个 execve 事件；
- 能说明 PID/TGID 的基本区别；
- 能解释为什么 `bpf_printk` 不适合高频数据通道。
