# Cyanrex 模块

本目录是项目中三个不同注册目录之一：

| 注册目录 | 持有方 | 注册的含义 |
|---|---|---|
| 模块清单 | `ModuleManager` 和本目录 | 发现有界元数据，控制内存中的生命周期状态 |
| Task 定义 | `TaskCatalog` 和 `engine/src/domain_packs/` | 解析精确领域定义，显式分派类型化评估 |
| 编辑器语言 | `frontend/src/features/editor/languages.ts` | 声明已配置的本地编辑能力 |

这些目录都不是自动可执行插件安装器、AI Agent 运行时或权限授予。启动 `module-ebpf`
不会创建 eBPF 教学领域包。见[平台架构](../docs/zh-CN/architecture.md)及
[领域边界](../docs/zh-CN/task-domain-boundary.md)。

直接子目录通过提供符合 [`module.schema.json`](module.schema.json) 的 `module.json`
清单加入 Engine 目录。Manifest schema version 1 声明稳定模块名、语义版本、描述和有界能力列表。

启动时，`ModuleManager` 扫描 `CYANREX_MODULES_DIR` 或仓库根级 `modules/` 目录。
清单格式错误、不受支持的 schema 版本、重复能力和不匹配目录的名称都会使启动失败。
没有清单的目录仅用于文档，发现过程会忽略它们。

`POST /modules/start` 和 `POST /modules/stop` 更新已知目录条目在单个 Engine 中的控制面状态。
它们绝不加载库、派生进程或执行模块目录中的文件。添加可执行插件需要另行评审隔离、签名、
所有权和生命周期协议。
