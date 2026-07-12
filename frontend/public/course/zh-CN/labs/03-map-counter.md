# 实验 3：使用 eBPF Map 计数

预计时间：45 分钟。

## 目标

- 理解 Map 的 key/value 模型；
- 使用 Per-CPU Array 降低竞争；
- 正确处理 Map 查询返回的指针。

## 起始代码

选择 `Ringbuf High-Freq Sampler`，先关注计数 Map：

```c
struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} per_cpu_counter SEC(".maps");
```

它为每个 CPU 保存一个 `__u64` 计数器。程序使用固定 key `0`。

## 步骤

1. 找到 Map 查询代码：

```c
__u32 key = 0;
__u64 *counter = bpf_map_lookup_elem(&per_cpu_counter, &key);
if (!counter) {
  return 0;
}
*counter += 1;
```

2. 解释为什么 `counter` 是指针，以及为什么必须判空。
3. 运行程序 10～20 秒，在 Events 页面观察 `count`。
4. 比较不同 CPU 事件中的计数值。Per-CPU Map 中每个 CPU 的计数相互独立。
5. 卸载程序。

## 故意制造错误

临时删除：

```c
if (!counter) {
  return 0;
}
```

观察静态提示和 verifier/加载结果。即使 Array key 看起来总是有效，helper 的接口仍然返回可空指针，
程序必须让 verifier 看到明确的判空路径。

恢复判空代码。

## 修改任务

将采样条件从每 64 次一次：

```c
if ((*counter & 63) != 0)
```

改成每 16 次一次：

```c
if ((*counter & 15) != 0)
```

比较相同时间内事件数量和系统负载。按位与写法成立是因为 16 和 64 都是 2 的幂。

## 思考题

1. 普通 Array 与 Per-CPU Array 的一致性和性能取舍是什么？
2. 为什么高频 hook 不应该每次都向用户态发送事件？
3. 如果需要按 PID 计数，应该选择 Array 还是 Hash？

## 验收

- 能画出 `key -> per-CPU value` 的关系；
- 保留正确的 NULL 检查；
- 能解释采样对吞吐量的影响。
