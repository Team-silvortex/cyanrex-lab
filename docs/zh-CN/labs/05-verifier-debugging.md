# 实验 5：读懂 Verifier 与调试程序

预计时间：50 分钟。

## 目标

- 区分 clang 错误和 verifier 拒绝；
- 练习空指针、边界和循环证明；
- 建立从第一条根因开始排错的习惯。

## 第一关：clang 错误

在任意模板中把变量名改成不存在的名称：

```c
return MISSING_SYMBOL;
```

编辑器应在对应行显示 `use of undeclared identifier`。此时程序尚未进入内核，属于 C 编译阶段。

## 第二关：Map 返回值判空

在实验 3 的代码中删除 `if (!counter)`。clang 可能接受这段 C，但 verifier 需要确认指针非 NULL。

修复模式：

```c
if (!counter) {
  return 0;
}
```

判空必须出现在解引用之前，并且控制流要让 verifier 能追踪。

## 第三关：XDP 数据包边界

XDP context 中的 `data` 和 `data_end` 描述当前可访问的数据包范围。访问以太网头之前应检查：

```c
void *data = (void *)(long)ctx->data;
void *data_end = (void *)(long)ctx->data_end;
struct ethhdr *eth = data;

if ((void *)(eth + 1) > data_end) {
  return XDP_PASS;
}
```

即使真实数据包通常足够长，verifier 也只接受所有可能路径都安全的访问。

## 第四关：有界循环

下面的循环边界无法由 verifier 轻易证明：

```c
while (condition_from_packet) {
  /* ... */
}
```

优先改成编译期可见的小上界：

```c
#pragma unroll
for (int i = 0; i < 8; i++) {
  /* 每次访问仍需做边界检查 */
}
```

现代内核支持有界循环，但循环复杂度仍会增加 verifier 状态数量。

## 日志阅读方法

1. 先判断失败阶段：compile、load 或 attach；
2. 从第一条 error 开始，不要先处理后续连锁错误；
3. 记录 verifier 提到的寄存器和指针类型；
4. 回到最近一次 helper 调用、指针运算或控制流分支；
5. 进行最小修改后重新检查；
6. 成功后解释“新增代码向 verifier 证明了什么”。

## 综合任务

选择一个已通过的模板，依次制造并修复：

- 一个未声明标识符；
- 一个缺失的 NULL 检查；
- 一个缺失的数据包边界检查；
- 一个边界不明确的循环。

为每个错误记录：失败阶段、关键日志、根因和修复理由。

## 验收

- 能在 1 分钟内区分 clang 和 verifier 错误；
- 修复不是“试到能运行”，而是能说明安全证明；
- 实验结束后卸载所有程序。
