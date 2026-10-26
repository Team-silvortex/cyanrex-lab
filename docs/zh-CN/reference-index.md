# Cyanrex 专题参考索引

[English](../en/reference-index.md)

按角色开始使用时先读[文档入口](README.md)。本索引集中收录实现专题、运行指南及历史证据，
保留原文件路径；Task 的 payload、代码与执行均非必需。

## 如何区分文档的状态

| 状态 | 阅读含义 |
|---|---|
| 现有运行链路 | 教学运行时或本地创作已接通的能力，仍遵守各自权限与保存边界。 |
| 仅显式宿主 | 需要单独配置和运行低权限平台宿主，不是正常教学应用的默认入口。 |
| 显式准备层 | 模块、命令或适配器可由受信调用者组合，不表示已挂载 UI、SDK 或在线服务。 |
| 设计目标 | 说明计划与期望边界，不能作为已经实现或部署的依据。 |
| 历史证据 | 只支持记录中的版本、日期和验证范围，不代表当前已重跑。 |

组内专题可能保留其最初实施阶段的标签；实际宿主接线以当前入口和项目状态为准。

## 项目与架构

当前架构图解释现有接线，详细架构解释实现依据，目标架构说明设计方向。分日期成熟度评分属于证据，不是部署结论。

- [当前项目状态与下一步](project-status.md)
- [当前架构与变更边界](architecture-map.md)
- [详细系统架构](architecture.md)
- [目标平台架构](next-architecture.md)
- [当前功能与模块网络](platform-network.md)
- [能力张量与分日期成熟度证据](capability-maturity.md)
- [平台与 eBPF 概念](concepts.md)
- [桌面与局域网课堂隔离设计](classroom-isolation.md)

## 平台宿主与存储

独立回环宿主须经显式私有配置调用，不将通用后端挂载到教学应用、不迁移认证，也不代表局域网或浏览器验收。

### 显式宿主入口

- [普通回环 Session 与私人 Task 宿主](platform-loopback-host.md)
- [原生 HTTPS 回环宿主](platform-loopback-tls-host.md)
- [可选服务状态通知](platform-host-notifications.md)
- [原绑定存储上的受限登录](platform-login-connection.md)
- [确认登录与专用 Cookie 投影](platform-login-http.md)
- [共享登录与 Session 路由组合](platform-session-router.md)
- [本人私人 Task 索引连接](platform-task-index-connection.md)
- [准确任务内容重开](platform-task-content-connection.md)
- [空手动 Draft 创建](platform-task-create-connection.md)
- [预期修订 Draft 清单换版](platform-task-replace-connection.md)

### 启动与原存储绑定检查

- [Linux 进程身份与权限检查](platform-process-startup.md)
- [私有启动输入校验](platform-startup-input.md)
- [宿主目录预检](platform-host-preflight.md)
- [原目录身份声明](platform-catalog-expectation.md)
- [原存储标记声明](platform-marker-expectation.md)
- [私有 Artifact 根目录观察](platform-artifact-root.md)
- [存储版本与作用域观察](platform-storage-markers.md)
- [显式本地数据库连接器](platform-database-connection.md)
- [一次性只读存储诊断](platform-storage-check.md)
- [保留原存储绑定的新连接](platform-storage-connection.md)
- [声明式存储打开与能力交接](platform-storage-open.md)

## 身份与 Session 权威

这些契约与显式服务区分可信存储和当前 Session 授权。只有所选宿主适配器接通其支持的命令；教学登录仍走原运行链路。

- [协作基础契约与原有基线](collaboration-foundation.md)
- [持久协作身份准备](collaboration-identity-store.md)
- [成员与部署策略准备](collaboration-access-store.md)
- [策略命令归属与事务审计](collaboration-policy-audit.md)
- [身份绑定与停用](collaboration-identity-lifecycle.md)
- [持久账号代际与 Session 来源](collaboration-auth-source.md)
- [会话授权协作事务](collaboration-session-commands.md)
- [受限账号删除](collaboration-account-deletion.md)
- [密码修改与会话撤销](collaboration-password-change.md)
- [全新权威原子初始化](collaboration-bootstrap.md)
- [受控本地初始化 CLI](collaboration-provisioning.md)
- [只读生命周期对账](collaboration-reconciliation.md)
- [显式 Session 存量限制](durable-session-stock.md)
- [当前 Session 查询与退出](platform-session-lifecycle.md)
- [Session 生命周期数据库权限](platform-session-permissions.md)
- [原绑定存储上的已有 Session](platform-session-connection.md)

## 任务制品与审阅

代码与执行均可选。资源发布、输入换版、Run 和 Review 是不同操作。下列独立适配器属于准备边界，不是通用浏览器保存链路。

### 核心契约与资源命令

- [领域中立任务定义与类型化评估](task-domain-boundary.md)
- [持久任务实例](task-instance-store.md)
- [不可变制品修订](artifact-revision-store.md)
- [绑定修订的审阅记录](review-record-store.md)
- [私人手动任务命令](session-task-commands.md)
- [私人制品命令](session-artifact-commands.md)
- [准确私人任务输入](session-task-inputs.md)
- [Draft 输入换版](session-task-revisions.md)
- [目录任务准入](session-catalog-tasks.md)
- [私人人工审阅命令](session-review-commands.md)

### 内容与本地创作

- [任务内容清单与准确引用](task-content-manifest.md)
- [内容持久化与原子编辑](task-content-store.md)
- [本人私人内容元数据索引](task-content-index.md)
- [当前 Session 内容与字节核验](session-task-content.md)
- [本地任务 payload 编辑器](editor.md)
- [经确认的本地多文本导入](local-text-file-import.md)

### 独立 HTTP 与发布适配器

- [显式手动内容 HTTP 适配](task-content-http.md)
- [私人内容索引 HTTP 适配](task-content-index-http.md)
- [准确目录内容 HTTP 适配](catalog-task-content-http.md)
- [文本制品发布与准确修订 HTTP](text-artifact-http.md)
- [原绑定文本发布、HTTP 与显式宿主组合](platform-text-artifact-connection.md)
- [严格草稿映射与未挂载内容保存尝试](task-draft-publication.md)
- [手动逐步草稿发布](session-task-draft-publication.md)
- [未知草稿目标只读观察](session-task-draft-observation.md)
- [草稿元数据检查点](session-task-draft-checkpoint.md)
- [检查点目标准确检查](session-task-draft-inspection.md)
- [不可变会话草稿意图记录](session-task-draft-intent.md)
- [日志化草稿发布派发](session-task-draft-dispatch.md)
- [草稿日志只读检查](session-task-draft-journal-inspection.md)
- [已记录步骤资源观察](session-task-draft-journal-observation.md)
- [冻结目录草稿发布尝试](session-catalog-task-draft-publication.md)
- [目录尝试目标观察](session-catalog-task-draft-observation.md)

## 大文件准备层

提案先覆盖集中分发，再规划桌面互传。已实现组件可显式组合，包含受控 Source 与 HTTPS 交付检查；正常应用分发、界面保存、续传和桌面互传尚未接通。

### 上传与发布

- [分发提案与范围](large-file-distribution.md)
- [清单与流式核心](large-file-core.md)
- [私有分块暂存](large-file-staging.md)
- [上传意图与逻辑配额](large-file-upload-intent.md)
- [上传绑定私有暂存回执](large-file-upload-staging.md)
- [持久分块写前日志](large-file-chunk-journal.md)
- [当前上传整文件字节核验](large-file-whole-verification.md)
- [持久整文件发布元数据](large-file-publication.md)
- [当前 Session 私人元数据读取](large-file-session-metadata.md)
- [当前 Session 授权发布](large-file-session-publication.md)

### 授权与受控读取

- [纯发布者与接收者授权契约](large-file-access-contract.md)
- [持久发布者授权账本](large-file-publisher-grants.md)
- [显式管理者授权命令](large-file-grant-management.md)
- [持久准确文件接收授权](large-file-recipient-grants.md)
- [发布者本人分享与撤销](large-file-sharing.md)
- [已授权准确下载清单观察](large-file-download-manifest.md)
- [已授权已核验分块快照](large-file-chunk-read.md)

### 传输与保存

- [私人单块二进制 HTTP 适配](large-file-chunk-http.md)
- [已核验响应接收与私有暂存](large-file-chunk-receiver.md)
- [固定 Origin 受控 HTTP 分块客户端](large-file-chunk-client.md)
- [有序私有整文件拼装](large-file-chunk-assembly.md)
- [顺序下载客户端](large-file-download-client.md)
- [接收者清单 HTTP 适配](large-file-manifest-http.md)
- [准确清单客户端与授权下载](large-file-manifest-client.md)
- [显式私有文件保存](large-file-save.md)
- [授权下载与私有保存](large-file-delivery-client.md)
- [显式本人普通目录目标](large-file-user-save.md)
- [最新交付进度观察](large-file-delivery-progress.md)
- [调用等待的协作式取消](large-file-delivery-cancellation.md)
- [受控 HTTPS 交付核验](large-file-delivery-https.md)
- [显式清单与分块路由组合](large-file-distribution-router.md)
- [真实 Source 授权与 HTTPS 交付](large-file-source-https.md)

## 教学运行时与运维

这些指南说明已接通的教学领域及运维控制，不将通用私人资源变为教师可读作业，也不隔离不可信内核执行。

- [教师工作流与部署权威](teacher-guide.md)
- [学生与个人学习流程](student-guide.md)
- [启动器离线包 API 与持久化](runtime-reference.md)
- [SSH 部署与学生课堂加入](classroom-connection.md)
- [Runner Agent 运维](runner-agent.md)
- [安全与部署边界](security.md)
- [按运维边界排障](troubleshooting.md)
- [学习记录与查询成本](learning-storage.md)
- [事件流与恢复](event-stream.md)
- [界面翻译与课程回退](localization.md)
- [实验一执行链路](labs/01-first-program.md)
- [实验二 execve 观察](labs/02-trace-execve.md)
- [实验三 Map 计数](labs/03-map-counter.md)
- [实验四 Ring Buffer 事件](labs/04-ring-buffer.md)
- [实验五 verifier 诊断](labs/05-verifier-debugging.md)

## SDK 与开发工具

SDK 与 AI Agent 辅助能力适配其支持的 Engine 接口，不是自主编排、通用平台保存集成或执行授权。

- [AI Agent 配置与显式工具适配](ai-agent-integration.md)
- [JavaScript SDK](../../sdk-js/README.md)
- [声明式模块目录](../../modules/README.md)
- [贡献流程与边界](../../CONTRIBUTING.zh-CN.md)
- [TDD 规范](../../TDD.zh-CN.md)
- [验证与运维脚本](../../scripts/README.zh-CN.md)

## 测试与历史

当前测试说明与分日期结果分开阅读。归档保留原实施阶段、版本和失败数量；历史通过不等于本次已重新验收。

### 当前验证入口

- [当前测试矩阵与显式边界套件](testing-guide.md)
- [发布内容与 Unreleased 变更](../../CHANGELOG.md)

### 状态与证据归档

- [原 0.6.3 状态与边界快照](history/project-status-snapshot-0-6-3.md)
- [十月八日至十一日验证记录](history/project-status-validation-2026-10-08-11.md)
- [十月七日及以前验证记录](history/project-status-validation-2026-10-07-and-earlier.md)
- [开发历史](development-history.md)
- [历史验收清单](acceptance.md)
- [历史模块与边界测试网络](testing-network.md)
- [历史功能网络基线](functional-network.md)

### 沿链路抓虫记录

- [01 学习与持久化](functional-network-bug-hunt-01.md)
- [02 Runner 与 Agent 生命周期](functional-network-bug-hunt-02.md)
- [03 浏览器编译诊断](functional-network-bug-hunt-03.md)
- [04 补全与头文件检查](functional-network-bug-hunt-04.md)
- [05 手动运行与附件清理](functional-network-bug-hunt-05.md)
- [06 调试会话与事件恢复](functional-network-bug-hunt-06.md)
- [07 事件历史与确认](functional-network-bug-hunt-07.md)
- [08 事件保留与持久化顺序](functional-network-bug-hunt-08.md)
- [09 冷启动准入与写入者生命周期](functional-network-bug-hunt-09.md)
- [10 真实读取与修复后重试](functional-network-bug-hunt-10.md)
- [11 变更确认与取消](functional-network-bug-hunt-11.md)
- [12 保留与溢出策略确认](functional-network-bug-hunt-12.md)
- [13 设置读取与部分保存](functional-network-bug-hunt-13.md)
- [14 指标生命周期与面板隔离](functional-network-bug-hunt-14.md)
- [15 Runner 清单与取消](functional-network-bug-hunt-15.md)
