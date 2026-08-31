# eBPF 课程知识地图

## 1. 一条程序如何运行

```text
C 源码
  -> clang 编译为 BPF 字节码
  -> 内核 verifier 验证安全性
  -> loader 加载程序和 Map
  -> attach 到 hook
  -> 内核事件触发程序
  -> Map/Ring Buffer/trace 输出数据
  -> 用户态读取并展示
```

Cyanrex 的结果区将上述过程拆成 compile、load 和 attach，排错时先判断失败发生在哪一层。

## 2. Hook

Hook 是 eBPF 程序被调用的位置。代码中的 `SEC("...")` 描述程序类型和挂载点。

- `SEC("xdp")`：网卡驱动接收路径的早期阶段；
- `SEC("tracepoint/category/name")`：稳定的内核 tracepoint；
- `SEC("kprobe/function")`：动态探测内核函数，兼容性要求更高；
- `SEC(".maps")`：Map 定义，不是可执行程序；
- `SEC("license")`：程序许可证。

## 3. Context

内核调用 eBPF 程序时会传入 context。例如 XDP 使用 `struct xdp_md *ctx`。
Context 能访问哪些字段由程序类型决定。输入 `ctx->` 时，Cyanrex 会请求 clang 给出真实字段。

## 4. Helper

eBPF 不能随意调用内核函数，只能调用当前程序类型允许的 helper，例如：

- `bpf_ktime_get_ns()`：读取单调时钟；
- `bpf_get_current_pid_tgid()`：读取进程/线程标识；
- `bpf_map_lookup_elem()`：查询 Map；
- `bpf_ringbuf_reserve()`：预留 Ring Buffer 记录。

有些 helper 只允许 GPL 兼容程序使用，因此示例会声明 GPL license。

## 5. Map

Map 是内核 eBPF 程序和用户态之间共享的状态容器。

- Hash：按 key 保存 value；
- Array：固定索引，访问成本稳定；
- Per-CPU Array：每个 CPU 独立保存数据，减少锁竞争；
- Ring Buffer：按时间顺序传递变长事件。

Map 查询可能返回 NULL，使用返回值前必须判空。

## 6. Verifier

Verifier 通过静态分析证明程序满足安全约束。它不会“猜测程序大概安全”。常见要求：

- 指针来源已知；
- 内存访问范围可证明；
- Map 查询和 Ring Buffer 预留结果已判空；
- 循环次数有明确上界；
- 所有执行路径都能终止；
- helper 参数类型符合约定。

写 eBPF 的关键不是让代码在人看来正确，而是让安全性能够被 verifier 证明。

## 7. CO-RE 与 BTF

BTF 描述内核类型。`vmlinux.h` 可以由当前内核 BTF 生成。CO-RE 程序借助类型和字段信息，
减少不同内核版本之间的适配成本，但它不保证任意程序能在所有内核上运行。
