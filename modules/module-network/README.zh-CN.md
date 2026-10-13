# module-network

本目录通过其版本化 `module.json` 清单被发现。它声明的集成范围包括：

- 捕获网络事件
- 向 Engine event bus 发布规范化事件 payload

启动该目录条目只改变 Engine 控制状态，不执行本目录中的代码。
