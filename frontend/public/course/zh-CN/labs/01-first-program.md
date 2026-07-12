# 实验 1：认识 eBPF 执行链路

预计时间：30 分钟。

## 目标

- 运行第一个 XDP 程序；
- 区分编译、加载和挂载；
- 学会安全卸载程序。

## 步骤

1. 在环境助手运行检查，确认总体状态为 Ready。
2. 打开 eBPF 页面，选择 `XDP Pass` 模板。
3. 等待编辑器中的 clang 状态变为 `passed`。
4. 阅读下面的核心代码：

```c
SEC("xdp")
int xdp_pass(struct xdp_md *ctx) {
  return XDP_PASS;
}
```

`SEC("xdp")` 声明程序类型。`XDP_PASS` 表示让数据包继续进入网络栈。

5. 点击“编译并运行”，观察结果区：
   - compile stderr 应为空或只有非阻塞警告；
   - load 阶段应成功；
   - 不同环境对 XDP 自动挂载的支持可能不同。
6. 查看“已挂载程序”区域，记录 pin path。
7. 点击卸载，并确认列表恢复为空。

## 修改任务

将返回值暂时改成不存在的 `XDP_UNKNOWN`。等待实时 clang 检查，记录错误行号和提示。
恢复为 `XDP_PASS`，确认红色诊断消失。

然后输入 `ctx->`，观察语义补全返回的 `data`、`data_end`、`ingress_ifindex` 等字段。

## 思考题

1. XDP 程序返回 `XDP_PASS` 时是否修改了数据包？
2. clang 检查通过是否等于 verifier 一定接受？为什么？
3. pin path 和实际 attach link 分别解决什么问题？

## 验收

- 能解释 `SEC("xdp")` 和 `XDP_PASS`；
- 能制造并修复一次 clang 错误；
- 实验结束后没有遗留挂载程序。
