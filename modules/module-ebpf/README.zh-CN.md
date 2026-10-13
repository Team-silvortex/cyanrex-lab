# module-ebpf

本目录通过其版本化 `module.json` 清单被发现。它声明的集成范围包括：

- attach eBPF 程序
- detach eBPF 程序
- 流式传输 syscall／network／latency 事件

启动该目录条目只改变 Engine 控制状态，不执行本目录中的代码。
既有 Engine eBPF 服务仍是具有权威性的实现。
