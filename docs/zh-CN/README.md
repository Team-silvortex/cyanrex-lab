# Cyanrex 项目文档

[English](../en/README.md)

Cyanrex 是以领域中立 Task 为基础的自托管协作与教学平台。任务 payload 可以是笔记、文本文件、
代码或配置；代码和 Run 都是可选项。eBPF 教学是首个已接通的领域，不是所有任务的定义。

按下方角色路径开始使用。[项目状态](project-status.md)区分当前能力与下一步决策；
[完整参考索引](reference-index.md)集中列出实现专题和历史记录。

## 先选对工作链路

| 工作链路 | 当前入口与边界 |
|---|---|
| 教授或学习 eBPF | 现有教学应用连接账号、学习记录、评语与 Linux 执行。 |
| 创作通用任务内容 | 浏览器编辑器管理本地草稿与经确认的文本导入，不将草稿保存到平台服务端。 |
| 使用独立平台后端 | 显式配置的低权限 Linux 宿主在回环地址提供受限登录、Session 和本人私人 Task 路由，可选原生 TLS。 |
| 组合资源或文件服务 | 独立存储、命令与传输适配器需要显式调用者，模块存在不等于已挂载教学应用。 |
| 探索目标平台 | 通用 Run、自主 AI 编排、浏览器到平台保存及桌面文件互传仍属于后续开发目标。 |

独立宿主不替换教学认证。其 Task 路由可创建空手动 Draft、列举并重开本人内容，或以已有准确文本
引用替换 Draft 清单；不发布新的 Artifact 字节，也不修改 Task 状态。

## 教授一门课程

先读[教师指南](teacher-guide.md)，再用[课堂接入与 SSH](classroom-connection.md)了解教师管理部署
及学生加入。个人实例的所有者默认是教师；这一权限不会转移到其加入的其他课堂。

课前准备：

1. 选择可信的 Linux 执行环境，按下方运维路径完成部署。
2. 从学生视角浏览实验并提前试跑。
3. 私下保管部署凭据，按受支持的学生加入流程分发访问权限。
4. 复用环境前确认清理流程与附件所有权。

教师权威覆盖现有教学和部署管理，不赋予读取其他所有者的通用 Task、Artifact、Review
或接收者授权文件字节的权限。

## 学习或加入课堂

按[学生指南](student-guide.md)了解账号、安全确认、学习记录和当前界面。
用[概念地图](concepts.md)联系平台模型与首个 eBPF 领域。

入门实验构成一条教学路径：

1. [理解执行链路](labs/01-first-program.md)。
2. [观察 execve](labs/02-trace-execve.md)。
3. [使用 Map 计数](labs/03-map-counter.md)。
4. [使用 Ring Buffer 传递事件](labs/04-ring-buffer.md)。
5. [阅读 verifier 诊断并调试](labs/05-verifier-debugging.md)。

eBPF 始终运行于 Linux 内核。Docker Desktop 和 WSL2 使用 Linux 虚拟机或子系统内核，
不是 Windows 或 macOS 桌面内核。应用账号不是内核隔离边界。

## 部署与运维

[运行时参考](runtime-reference.md)集中说明启动器、离线包、账号、运行时 API 和持久化。
连接客户端或启用特权执行前，先读[安全与部署边界](security.md)；
遇到环境、账号、存储或运行故障时，查阅[故障排查](troubleshooting.md)。

按实际运行方式选择指南：

- [Runner Agent](runner-agent.md)说明现有计算工作节点及其显式权限。
- [普通回环宿主](platform-loopback-host.md)说明独立低权限平台进程。
- [原生 HTTPS 回环宿主](platform-loopback-tls-host.md)增加私有 TLS 身份与 Secure Cookie。

回环宿主不等于已验收局域网或浏览器部署。客户端信任、私有存储、进程身份和准确配置仍须按所选
宿主指南检查。服务通知及存储预检与其他宿主细节一起收录于参考索引，不是正常应用的额外自动启动步骤。

## 开发协作平台

先读[当前架构图](architecture-map.md)，定位运行接线与职责归属。
[详细架构](architecture.md)解释实现决策；[目标架构](next-architecture.md)说明演进方向，
不能当作全部已完成能力。

修改模块或连接时：

1. 在[平台功能地图](platform-network.md)定位现有链路。
2. 用[能力与证据矩阵](capability-maturity.md)核对边界，不将分日期评分视为部署验收。
3. 从参考索引选择宿主、身份、Task、Artifact、Review 或文件专题。
4. 保持本地状态、可信直接存储、当前 Session 命令和 HTTP 投影之间的职责分离。

目录定义不赋予执行或评估权限。发布 Artifact、替换 Task 内容、启动 Run、记录 Review
是不同操作。教学 Engine 仍同时承担控制与特权 Linux 执行；源码职责拆分不等于多租户沙箱。

## 编辑任务内容或接入 SDK

[任务 payload 编辑器](editor.md)说明任务所属的本地文档及编辑器能力。
[本地文本导入](local-text-file-import.md)经确认追加一批文件，不替换已有草稿。
本地文件名、标识和语言提示不是服务端引用或权限。

[JavaScript SDK](../../sdk-js/README.md)覆盖其受支持的 Engine 契约；
[AI Agent 接入指南](ai-agent-integration.md)说明连接配置与明确选出的工具适配。
这些辅助能力不提供自主编排，也不是通用平台保存 API。

[模块目录](../../modules/README.md)包含声明式领域元数据。
清单条目本身不是可执行插件，也不表示新增了已部署运行时。

## 查看大文件分发进展

[大文件专题](reference-index.md#大文件准备层)区分提案与已实现的暂存、授权、传输和保存组件。
受控交付可显式组合；正常应用分发、保存对话框、续传和桌面互传尚未接通。

选择组件时应核对其授权和验证边界，不将下载完成、文件保存、发布元数据和可复用权限
视为同一个结果。

## 验证与贡献

改变行为前先读[贡献指南](../../CONTRIBUTING.zh-CN.md)及 [TDD 规范](../../TDD.zh-CN.md)。
[测试指南](testing-guide.md)区分默认检查与显式 SQL、浏览器、传输、SDK 和特权内核验收；
[脚本与工具](../../scripts/README.zh-CN.md)提供相应命令入口。

默认 Cargo 测试跳过显式数据库和内核用例。拦截 Engine 请求的浏览器夹具只验证其界面或组件边界，
不证明在线权限或服务端保存已接通。

## 查找专题或历史结果

参考索引按宿主、身份、内容、大文件、教学运行时和验证分组收录完整专题，也包含状态快照归档及
沿链路抓虫记录。分日期报告保留当时版本、失败与限制，不是当前测试结果。

[更新日志](../../CHANGELOG.md)记录发布内容及 Unreleased 修复；
[翻译覆盖](localization.md)说明五种界面语言、双语指南和明确的课程正文回退。
