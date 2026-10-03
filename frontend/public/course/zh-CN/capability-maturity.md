# 平台功能张量与成熟度

评审日期：**2026-10-03**。历史源码基线：**0.4.9**，基于提交 `5928508b95276a44de73dba04dd9cfc952fbfb5a`，
并包含现已收录于 **0.5.0** 的同页锚点导航、Unicode 文件名修复及 C2-K 内容契约、C2-L 存储、
C2-M 会话内容和 C2-N HTTP 适配。原基线提交和证据日期保持不变；本次按源码与明确范围的验证记录盘点，
不构成新的 0.5.0 发布门禁或当前部署验收。

当前结论：**本地任务编辑、现用教学运行、协作后端准备层分别成立，三者还没有汇合成通用任务保存与协作闭环。**
因此用多维坐标描述能力，用独立的边描述连接，不再把相邻模块存在当成整条功能链已经完成。

[机器清单](../platform-capability-tensor.json)是坐标、评分、源码、测试入口、证据和缺边的维护源；
本文是阅读视图。[平台链路地图](platform-network.md)保留路由与模块拓扑，
[项目进度](project-status.md)保留分日期验证结果，[测试指南](testing-guide.md)说明如何执行各类检查。

## 四维坐标与稀疏语义

记作 `T[A, F, I, M] = score`：A 是架构面，F 是功能，I 是实现切片，M 是成熟度指标。
JSON 的每个 cell 保存一个 `[A, F, I]` 坐标及 D/C/V/O 四项分值，相当于稀疏坐标格式的一组四个标量。
实现切片可以引用多个协作文件；它不是文件数量、微服务数量或部署单元。同一功能可以有多个实现坐标，
例如 D01 的内部领域目录与 D02 的现用教学门面，接线程度不同，不能合成一个“已完成”。

本轮登记 **8 个架构面、56 项功能、57 个实现坐标、73 条定向边、16 条代表链路**。
57 个坐标分为：**17 已接入、5 仅本地、23 准备层、12 规划**。这些是本轮切片数，不是覆盖率分母；
并未枚举所有端点、内部函数、失败排列或潜在功能，也不以稀疏密度估算产品完成度。

- **缺席的坐标**：未建模或不适用，不能填成 0。
- **`null`**：已建模但未评估，不等于未实现；当前 57 项已评分。
- **显式 0**：在该指标定义的范围内未达到一级，具体原因见缺口。
- **规划项**：可以已有类型契约 D=1，但连接和能力执行证据仍为 C=0、V=0。
- **边独立评分**：节点有代码和测试，不证明它们相连；`missing` 边不能借用两端分数变成已接通。

## 成熟度评分规则

这是基于可查证据的**人工序数判断**，不是实测百分比、安全等级或 SLA。四项分数不相加、不平均、
不归一化成“完成率”；链路遇到缺边即保留阻断，而不是计算节点均分。证据时间和适用范围始终单列。

| 指标 | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| D 实现深度 | 只有设计或无实现 | 显式类型与契约 | 已有功能实现 | 已实现边界防护并有回归用例 |
| C 接线程度 | 所需连接缺失 | 仅显式内部组合 | 已接当前应用、本地用户流程或运维工具 | 对应当前部署的真实跨组件验收 |
| V 验证证据 | 无该能力执行证据 | 仅核对测试源码 | 有分日期单元、模拟、浏览器或工具夹具记录 | 有明确范围的真实数据库或服务集成记录 |
| O 运行保障 | 无该能力运维流程 | 有限制与运行前提说明 | 有显式有界运维或核对控制及回归 | 对应当前部署的恢复或回滚验收 |

例如 `3 / 1 / 3 / 1` 表示后端防护和历史 SQL 边界验证较充分，但仍只在内部组合，**不表示界面能用**。
`3 / 2 / 2 / 1` 可以描述本地编辑闭环；真实 Monaco 和文件下载测试仍不证明真实 Engine 保存。
当前没有 C=3 或 O=3：没有把旧内核、LAN、SSH 或制品验收冒充当前部署验收。V=3 也不自动刷新历史证据日期。

| 架构坐标 | 范围 |
|---|---|
| `ui` | 浏览器与本地内容 |
| `authority` | 现用实例权威 |
| `identity` | 协作身份准备层 |
| `work` | 任务内容与审阅 |
| `domain` | 领域与教学 |
| `execution` | 执行与资源 |
| `delivery` | 事件与集成 |
| `operations` | 运维与分发 |

## 功能与实现坐标

每行只对列出的能力范围打分。源码链接定位实现，证据链接定位已有记录；完整测试路径保存在 JSON 的
`implementations.tests`，测试文件存在不意味着本轮执行了测试。U01/U06 包含 0.5.0 收录的修复；W10–W13
是同一版本的纯内容契约、独立存储、当前 Session 内容组合及未挂载 HTTP 准备层。源码收录不改变接线评分，
也不提供浏览器保存。

<!-- capability-tensor:start -->
| 坐标 | 功能 | 接入状态 | D / C / V / O | 实现源码 | 证据 | 下一缺口 |
| --- | --- | --- | --- | --- | --- | --- |
| U01 · ui | 会话外壳与导航 | 已接入 | 3 / 2 / 2 / 1 | [SidebarLayout.tsx](../../frontend/src/components/SidebarLayout.tsx) | [bughunt-local](../../docs/en/project-status.md) | 侧栏不授予服务端权限；锚点导航修复收录于 0.5.0。 |
| U02 · ui | 本地任务草稿归属 | 仅本地 | 3 / 2 / 2 / 1 | [taskDraft.ts](../../frontend/src/features/tasks/taskDraft.ts)<br>[TaskDraftWorkspace.tsx](../../frontend/src/features/tasks/TaskDraftWorkspace.tsx) | [bughunt-local](../../docs/en/project-status.md) | 无服务端保存、浏览器持久化、任务列表或二进制 payload。 |
| U03 · ui | 整份草稿导入导出 | 仅本地 | 3 / 2 / 2 / 1 | [TaskDraftWorkspace.tsx](../../frontend/src/features/tasks/TaskDraftWorkspace.tsx)<br>[taskDraft.ts](../../frontend/src/features/tasks/taskDraft.ts) | [bughunt-local](../../docs/en/project-status.md) | 下载只是本地导出，不是服务端提交或磁盘落盘保证。 |
| U04 · ui | 受控 payload 编辑器 | 仅本地 | 3 / 2 / 2 / 1 | [EditorWorkspace.tsx](../../frontend/src/features/editor/EditorWorkspace.tsx)<br>[useEditorSession.ts](../../frontend/src/features/editor/useEditorSession.ts) | [bughunt-local](../../docs/en/project-status.md) | 单个选中模型；切换不保留 undo，也不创建文件系统项目。 |
| U05 · ui | 本地语言辅助 | 仅本地 | 3 / 2 / 2 / 1 | [languages.ts](../../frontend/src/features/editor/languages.ts)<br>[languageServices.ts](../../frontend/src/features/editor/languageServices.ts) | [bughunt-local](../../docs/en/project-status.md) | 14 种配置不等于 14 个 LSP；共享 JS/TS worker 不是安全沙箱。 |
| U06 · ui | 有界文本导入与下载 | 仅本地 | 3 / 2 / 2 / 1 | [document.ts](../../frontend/src/features/editor/document.ts)<br>[EditorWorkspace.tsx](../../frontend/src/features/editor/EditorWorkspace.tsx) | [bughunt-local](../../docs/en/project-status.md) | 仅文本；Unicode 截断修复收录于 0.5.0，文件名不授予文件系统访问权。 |
| U07 · ui | 目标绑定操作确认 | 已接入 | 3 / 2 / 2 / 1 | [useConfirmedAction.tsx](../../frontend/src/components/useConfirmedAction.tsx)<br>[useDraftWarning.ts](../../frontend/src/features/editor/useDraftWarning.ts) | [release-049](../../docs/en/project-status.md)<br>[bughunt-local](../../docs/en/project-status.md) | 确认不等于授权或回滚；不覆盖全部历史与程序导航恢复。 |
| A01 · authority | 密码 TOTP 与教师权威 | 已接入 | 3 / 2 / 2 / 1 | [auth_service.rs](../../engine/src/services/auth_service.rs)<br>[auth.rs](../../engine/src/routes/auth.rs)<br>[application.rs](../../engine/src/application.rs) | [release-049](../../docs/en/project-status.md)<br>[bughunt-local](../../docs/en/project-status.md) | 仍为旧实例权威，不是新 Workspace Grant；尚未切换持久认证源。 |
| A02 · authority | 教师发现与邀请加入 | 已接入 | 3 / 2 / 2 / 1 | [classroom.rs](../../engine/src/services/classroom.rs)<br>[join.tsx](../../frontend/pages/join.tsx)<br>[connection.js](../../frontend/src/features/classroom/connection.js) | [release-049](../../docs/en/project-status.md) | 发现不是认证；无组播发现或当前局域网 TLS 验收。 |
| I01 · identity | 带范围的协作契约 | 准备层 | 2 / 1 / 2 / 1 | [mod.rs](../../engine/src/models/collaboration/mod.rs)<br>[legacy_workspace.rs](../../engine/src/services/legacy_workspace.rs) | [release-049](../../docs/en/project-status.md) | 带范围类型与已实现的旧权限纯投影不执行迁移、认证或授权。 |
| I02 · identity | 身份绑定与退役 | 准备层 | 3 / 1 / 3 / 1 | [identity_command.rs](../../engine/src/services/collaboration_identity_store/identity_command.rs)<br>[identity_audit.rs](../../engine/src/services/collaboration_identity_store/identity_audit.rs) | [c1-identity](../../docs/en/collaboration-identity-lifecycle.md) | 已有历史 SQL 证据；生产数据收编、旧写入隔离与恢复未实现。 |
| I03 · identity | 成员授权与策略审计 | 准备层 | 3 / 1 / 3 / 1 | [access_policy.rs](../../engine/src/services/collaboration_identity_store/access_policy.rs)<br>[policy_command.rs](../../engine/src/services/collaboration_identity_store/policy_command.rs) | [c1-policy](../../docs/en/collaboration-policy-audit.md) | 成员关系不自动授予部署权或私人内容分享权。 |
| I04 · identity | 持久认证与会话 | 准备层 | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/auth_service/durable_source/mod.rs)<br>[sessions.rs](../../engine/src/services/auth_service/durable_source/sessions.rs) | [c1-auth](../../docs/en/collaboration-auth-source.md) | 公共登录仍走旧源；新的公共认证与 CSRF 适配未接入。 |
| I05 · identity | 会话授权策略命令 | 准备层 | 3 / 1 / 3 / 1 | [session_commands.rs](../../engine/src/services/auth_service/durable_source/session_commands.rs)<br>[session_adapter.rs](../../engine/src/services/collaboration_identity_store/session_adapter.rs) | [c1-session](../../docs/en/collaboration-session-commands.md) | 仅内部事务持有授权；无公共管理流程。 |
| I06 · identity | 账号删除与密码轮换 | 准备层 | 3 / 1 / 3 / 1 | [deletion.rs](../../engine/src/services/auth_service/durable_source/deletion.rs)<br>[credentials.rs](../../engine/src/services/auth_service/durable_source/credentials.rs) | [c1-password](../../docs/en/collaboration-password-change.md) | 无在线账号迁移或恢复；取消等待不证明回滚。 |
| O01 · operations | 显式权威初始化 | 准备层 | 3 / 2 / 3 / 2 | [main.rs](../../engine/src/bin/cyanrex-provision/main.rs)<br>[bootstrap.rs](../../engine/src/services/auth_service/durable_source/bootstrap.rs) | [c1-provision](../../docs/en/collaboration-provisioning.md) | 仅本地显式空命名空间工具，不是启动安装器或旧数据导入器。 |
| O02 · operations | 权威图只读核对 | 准备层 | 3 / 2 / 3 / 2 | [mod.rs](../../engine/src/services/auth_service/durable_source/reconciliation/mod.rs)<br>[mod.rs](../../engine/src/services/collaboration_identity_store/reconciliation/mod.rs) | [c1-reconcile](../../docs/en/collaboration-reconciliation.md) | 有界只读快照，不是修复、授权或秘密送达证明。 |
| W01 · work | 修订约束任务存储 | 准备层 | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/task_store/mod.rs)<br>[replace.rs](../../engine/src/services/task_store/replace.rs) | [c2-database](../../docs/en/project-status.md) | 仅 Schema 2 新命名空间；无 Schema 1 迁移、分配或验收。 |
| W02 · work | 不可变 Artifact 修订 | 准备层 | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/artifact_store/mod.rs)<br>[blob.rs](../../engine/src/services/artifact_store/blob.rs) | [c2-database](../../docs/en/project-status.md) | 文件与 SQL 不是同一原子提交；无元数据映射、分享或垃圾回收。 |
| W03 · work | 不可变 Review 历史 | 准备层 | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/review_store/mod.rs)<br>[records.rs](../../engine/src/services/review_store/records.rs) | [c2-database](../../docs/en/project-status.md) | 直接调用方受信任；存储规则判断不证明规则已运行或任务已验收。 |
| W04 · work | 私人手工任务命令 | 准备层 | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/services/auth_service/durable_source/task_commands/mod.rs)<br>[mod.rs](../../engine/src/services/auth_service/durable_source/private_work/mod.rs) | [c2-database](../../docs/en/project-status.md) | 仅内部本人任务适配；无公共 API、payload 元数据或任务改名。 |
| W05 · work | 会话授权内容发布 | 准备层 | 3 / 1 / 3 / 1 | [artifact_commands.rs](../../engine/src/services/auth_service/durable_source/artifact_commands.rs) | [c2-database](../../docs/en/project-status.md) | 无浏览器上传适配；数据库结果不确定不授权删除文件。 |
| W06 · work | 本人任务的精确输入 | 准备层 | 3 / 1 / 3 / 1 | [inputs.rs](../../engine/src/services/auth_service/durable_source/task_commands/inputs.rs) | [c2-database](../../docs/en/project-status.md) | 固定本人修订并核对内容，不生成领域证据或 Run。 |
| W07 · work | 私人人工审阅命令 | 准备层 | 3 / 1 / 3 / 1 | [review_commands.rs](../../engine/src/services/auth_service/durable_source/review_commands.rs) | [c2-database](../../docs/en/project-status.md) | 仅本人内容；无跨用户授权、AI 审阅或规则授权适配。 |
| W08 · work | 精确目录任务准入 | 准备层 | 3 / 1 / 3 / 1 | [catalog.rs](../../engine/src/services/auth_service/durable_source/task_commands/inputs/catalog.rs) | [c2-database](../../docs/en/project-status.md) | 仅准入定义元数据；不保留或执行 provider。 |
| W09 · work | Draft 输入换版 | 准备层 | 3 / 1 / 3 / 1 | [replacement.rs](../../engine/src/services/auth_service/durable_source/task_commands/inputs/replacement.rs) | [c2-database](../../docs/en/project-status.md) | 无界面冲突恢复；内容发布独立，旧内容与审阅不变。 |
| W10 · work | 任务内容清单与快照校验 | 准备层 | 3 / 1 / 2 / 1 | [content.rs](../../engine/src/models/collaboration/content.rs)<br>[task_content.rs](../../engine/src/services/task_content.rs) | [content-contract](../../docs/en/task-content-manifest.md) | 仅给定快照纯校验。独立 W11 可持久化元数据，但本验证器不证明来源或授权。 |
| W11 · work | 任务内容原子存储 | 准备层 | 3 / 1 / 3 / 1 | [content.rs](../../engine/src/services/task_store/content.rs)<br>[content_commands.rs](../../engine/src/services/task_store/content_commands.rs)<br>[content_records.rs](../../engine/src/services/task_store/content_records.rs)<br>[schema.rs](../../engine/src/services/task_store/schema.rs)<br>[0014_collaboration_task_content.sql](../../engine/migrations/0014_collaboration_task_content.sql) | [content-storage](../../docs/en/task-content-store.md) | 仅可信 Schema 3 基础设施；W12 另行提供授权/文本组合。无目录准入、公共保存或迁移。 |
| W12 · work | 会话授权任务内容 | 准备层 | 3 / 1 / 3 / 1 | [content.rs](../../engine/src/services/auth_service/durable_source/task_commands/content.rs)<br>[mod.rs](../../engine/src/services/auth_service/durable_source/private_work/mod.rs)<br>[task_content.rs](../../engine/src/services/task_content.rs) | [session-content](../../docs/en/session-task-content.md) | 内部认证源同事务授权/文本检查；W13 为独立未挂载 HTTP 适配。无浏览器保存、目录准入、迁移或在线认证切换。 |
| W13 · work | 显式任务内容 HTTP 适配 | 准备层 | 3 / 1 / 3 / 1 | [mod.rs](../../engine/src/platform_http/mod.rs)<br>[security.rs](../../engine/src/platform_http/security.rs)<br>[handlers.rs](../../engine/src/platform_http/handlers.rs)<br>[contract.rs](../../engine/src/platform_http/contract.rs)<br>[errors.rs](../../engine/src/platform_http/errors.rs) | [content-http](../../docs/en/task-content-http.md) | 仅显式独立路由，未挂载到现用应用。无 Session 签发、公共 Artifact 发布、浏览器映射/保存、现用 OpenAPI/SDK 或迁移。 |
| D01 · domain | 版本化领域目录 | 准备层 | 3 / 1 / 2 / 1 | [task_catalog.rs](../../engine/src/services/task_catalog.rs) | [release-049](../../docs/en/project-status.md) | 受信任静态 provider；非教学 provider 仍为夹具，非产品流程。 |
| D02 · domain | 版本化领域目录 | 已接入 | 3 / 2 / 2 / 1 | [learning_catalog.rs](../../engine/src/services/learning_catalog.rs)<br>[mod.rs](../../engine/src/domain_packs/ebpf_teaching/mod.rs) | [release-049](../../docs/en/project-status.md) | 仅教学门面已接入；评估不等于通用持久 Review。 |
| D03 · domain | 实验尝试反馈与恢复 | 已接入 | 3 / 2 / 2 / 1 | [learning_store.rs](../../engine/src/services/learning_store.rs)<br>[learning.rs](../../engine/src/routes/learning.rs)<br>[teaching.tsx](../../frontend/pages/teaching.tsx) | [release-049](../../docs/en/project-status.md) | 仍是旧尝试与反馈；恢复源码不自动运行或迁移为 Task/Review。 |
| D04 · domain | 所有者范围脚本存储 | 已接入 | 3 / 2 / 2 / 1 | [script_store.rs](../../engine/src/services/script_store.rs)<br>[scripts.rs](../../engine/src/routes/scripts.rs) | [release-049](../../docs/en/project-status.md) | 沿用脚本身份与降级策略；不是通用 Artifact 存储。 |
| R01 · execution | eBPF 编译与 C 辅助 | 已接入 | 3 / 2 / 2 / 1 | [check.inc.rs](../../engine/src/routes/ebpf/check.inc.rs)<br>[semanticCompletion.ts](../../frontend/src/utils/semanticCompletion.ts) | [release-049](../../docs/en/project-status.md)<br>[bughunt-local](../../docs/en/project-status.md) | Engine C 工具可能发送源码；与本地任务语言服务分离。 |
| R02 · execution | 本地 eBPF 运行观测卸载 | 已接入 | 3 / 2 / 2 / 1 | [handlers.inc.rs](../../engine/src/routes/ebpf/handlers.inc.rs)<br>[attach.inc.rs](../../engine/src/services/ebpf_loader/attach.inc.rs)<br>[useRuntimeActions.ts](../../frontend/src/features/ebpf/useRuntimeActions.ts) | [release-049](../../docs/en/project-status.md)<br>[kernel-history](../../reports/acceptance/2026-09-09-kernel-vm/README.md) | 无当前内核验收；特权共享内核不隔离恶意学生。 |
| R03 · execution | 本地容量租约与配额 | 已接入 | 3 / 2 / 2 / 1 | [runner_manager.rs](../../engine/src/services/runner_manager.rs)<br>[runner_driver.rs](../../engine/src/services/runner_driver.rs) | [release-049](../../docs/en/project-status.md) | 内存准入；无持久 Run、重启恢复或多 Engine 挂载归属。 |
| R04 · execution | 签名 Agent 探针与编译作业 | 已接入 | 3 / 2 / 3 / 2 | [runner_agent_client.rs](../../engine/src/services/runner_agent_client.rs)<br>[runner_agent_executor.rs](../../engine/src/services/runner_agent_executor.rs)<br>[runner_job_queue.rs](../../engine/src/services/runner_job_queue.rs) | [release-049](../../docs/en/project-status.md)<br>[agent-history](../../reports/releases/0.4.3/README.md) | 无远程内核执行；作业与注册易失。自报隔离能力不证明 VM/容器隔离，也不隐式回退。 |
| E01 · delivery | 所有者遥测与有界恢复 | 已接入 | 3 / 2 / 2 / 1 | [event_bus.rs](../../engine/src/services/event_bus.rs)<br>[eventStream.ts](../../frontend/src/features/events/eventStream.ts)<br>[useEventActions.ts](../../frontend/src/features/events/useEventActions.ts) | [release-049](../../docs/en/project-status.md) | 近期历史尽力恢复；无业务游标、恰好一次或跨 Engine 排序。 |
| E02 · delivery | 资源事务 outbox 记录 | 准备层 | 3 / 1 / 3 / 1 | [records.rs](../../engine/src/services/task_store/records.rs)<br>[records.rs](../../engine/src/services/artifact_store/records.rs)<br>[records.rs](../../engine/src/services/review_store/records.rs)<br>[content_records.rs](../../engine/src/services/task_store/content_records.rs) | [c2-database](../../docs/en/project-status.md)<br>[content-storage](../../docs/en/task-content-store.md) | 仅原子记录；无投递服务或消费者重放。 |
| E03 · delivery | 公共 API 与 SDK 兼容 | 已接入 | 3 / 2 / 2 / 2 | [application.rs](../../engine/src/application.rs)<br>[openapi.json](../../engine/openapi/openapi.json)<br>[index.ts](../../sdk-js/src/index.ts) | [release-049](../../docs/en/project-status.md) | 通用准备层命令尚未公开；包注册发布与长期支持待落实。 |
| O03 · operations | 实例启动与诊断 | 已接入 | 3 / 2 / 2 / 2 | [start.sh](../../start.sh)<br>[config.rs](../../engine/src/config.rs)<br>[state.rs](../../engine/src/state.rs) | [release-049](../../docs/en/project-status.md) | 健康检查不等于内核就绪；启动不安装准备层存储。 |
| O04 · operations | 包校验与 SSH 部署 | 已接入 | 3 / 2 / 2 / 2 | [package.rs](../../engine/src/bin/cyanrex-release/package.rs)<br>[ssh_cli.rs](../../engine/src/bin/cyanrex-release/ssh_cli.rs) | [release-049](../../docs/en/project-status.md) | 面向已装包目标；无新候选制品、局域网 SSH 验收或裸机初始化。 |
| O05 · operations | 设置指标与 Agent 管理 | 已接入 | 3 / 2 / 2 / 2 | [useSettingsForm.ts](../../frontend/src/features/settings/useSettingsForm.ts)<br>[usePerformanceMetrics.ts](../../frontend/src/features/settings/usePerformanceMetrics.ts)<br>[useRunnerAgentAdmin.ts](../../frontend/src/features/runner/useRunnerAgentAdmin.ts) | [release-049](../../docs/en/project-status.md) | 沿用教师权威；写入不确定需显式核对，不盲目重试。 |
| O06 · operations | 模块头文件与结构化命令 | 已接入 | 3 / 2 / 2 / 1 | [module_manager.rs](../../engine/src/services/module_manager.rs)<br>[c_header_module.rs](../../engine/src/services/c_header_module.rs)<br>[command_dispatcher.rs](../../engine/src/services/command_dispatcher.rs) | [release-049](../../docs/en/project-status.md) | 模块状态不等于执行插件；终端不是任意 shell。 |
| P01 · work | 浏览器到服务端任务保存 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | W10-W13 提供契约、存储、Session/文本检查和未挂载 HTTP 边界；仍缺安全登录/安装、内容发布、草稿映射及浏览器保存/读取/冲突/结果处理。 |
| P02 · identity | 在线权威切换与迁移 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 先盘点数据、验证备份恢复、隔离旧写入方并审阅安装迁移回滚，再切换路由。 |
| P03 · work | 分享与跨用户审阅 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 先建立内容分享权与审阅者委派，不放宽本人范围冒充协作。 |
| P04 · domain | Artifact 证据到授权规则审阅 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 需将精确内容、策略与 provider 版本绑定到授权证据和持久判断。 |
| P05 · work | 通用任务验收 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 缺验收策略、状态与 Review 影响定义；执行成功不够。 |
| P06 · execution | 持久通用 Run 编排 | 规划 | 1 / 0 / 0 / 0 | [references.rs](../../engine/src/models/collaboration/references.rs) | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 只有 Run 引用，无调度器与存储；缺输入、租约、取消和重启核对。 |
| P07 · delivery | 可靠业务事件投递 | 规划 | 1 / 0 / 0 / 0 | [event.rs](../../engine/src/models/collaboration/event.rs) | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 只有信封与记录，无投递器、消费者去重或游标重放。 |
| P08 · execution | 托管学生隔离运行环境 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 缺无特权控制面、VM 归属、重置隔离与清理验收。 |
| P09 · ui | 外部 LSP 与项目工作区 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 缺文件与进程授权、传输、生命周期和资源隔离。 |
| P10 · execution | 有界 AI 委派 | 规划 | 1 / 0 / 0 / 0 | [identity.rs](../../engine/src/models/collaboration/identity.rs) | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | Agent 主体只是类型，不是规划器、工具授权、预算或独立审阅。 |
| P11 · delivery | 外部生态适配 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 无已实现合作项目适配器；外部项目假设未核实。 |
| P12 · domain | 可执行领域插件 | 规划 | 0 / 0 / 0 / 0 | — | [design](../../docs/en/project-status.md)<br>[target](../../docs/zh-CN/next-architecture.md) | 安装、信任、运行时加载与撤销须独立于声明清单设计。 |
<!-- capability-tensor:end -->

## 代表链路与缺失连接

`→` 表示现应用或本地接线；`⇢` 表示显式内部组合；`×→` 表示缺边。箭头描述内容、调用或依赖的交接，
**不是权限传递**。每个私人资源命令都独立复查当前 Session、账号、成员与 owner：e52–e56 将这些授权边单列；
e57–e59 明确三类存储均写 outbox。e64 增加独立内容存储的 outbox 记录；e66 现接入 W12 专用会话适配，
e67–e69 单列其存储与文本检查；e71/e72 增加 W13 显式 HTTP 组合，浏览器 e70 改指 W13 并保持缺失，
安装/权威 e73 也仍缺失。JSON 保存全部 73 条边的失败/信任边界。

| 代表链路 | 坐标顺序 | 判定与边界 |
|---|---|---|
| 本地内容编辑 | U01 → U02 → U04 → U05 | 浏览器内接通，外壳测试模拟 Engine；内容不保存到服务端。 |
| 现有教学流程 | A02 → A01 → D04 → R01 → R03 → R02 → D02 → D03 | 源码已接线；不是新执行的真实局域网内核端到端验收。 |
| 可选远程编译 | R01 → R04 | 仅编译探针；不远程加载、不返回目标文件、不隐式本地回退。 |
| 私人内容与任务换版 | I04 ⇢ W05 ⇢ W06 ⇢ W09 ⇢ E02 | 显式准备层组合；内容发布与任务更新是分开的操作。 |
| 私人人工审阅 | I04 ⇢ W05 ⇢ W07 ⇢ W03 | 仅私人所有者；无跨用户审阅或自动验收。 |
| 缺失的服务端保存桥 | U02 ×→ P01 ×→ W05 ⇢ W06 ⇢ W09 | 尽管后端组件有测试，缺失的浏览器公共桥仍阻断保存闭环。 |
| 缺失的在线权威切换 | U01 ×→ P02 ×→ I04 ⇢ W05 | 需明确安装认证决策并审阅恢复迁移。 |
| 缺失的规则审阅与验收 | W02 ×→ P04 ×→ W03 ×→ P05 | 内容、目录、执行或已存判断都不能传递出验收结论。 |
| 缺失的通用执行桥 | W01 ×→ P06 ×→ R03 ×→ P08 | 持久编排与隔离资源归属是两个独立缺口。 |
| 缺失的业务事件投递 | W09 ⇢ E02 ×→ P07 | 存储侧原子性止于 outbox；不宣称投递或重放。 |
| 显式准备层权威设置 | O01 ⇢ I04 ⇢ O02 | 本地运维工具已存在；不迁移已部署数据、不修复、不切换在线认证源。 |
| 发行包到现用运行面 | O04 → O03 → A01 | 仅源码工具链路；实际目标 apply 与验收须独立显式执行。 |
| 给定内容快照校验 | W01 ⇢ W10 | 纯 Task/元数据匹配；W02 ⇢ W10 补充字节与引用自洽检查。两条边均不读取存储、不授权 Session。 |
| 可信内容持久化 | W10 ⇢ W11 ⇢ E02 | 完整元数据在独立命名空间原子保存；没有 Session/Artifact 字节适配或浏览器保存。 |
| 授权内容编辑 | I04 ⇢ W12 ⇢ W11 ⇢ E02 | 同一事务核验当前 Session 及旧新文本；e68/e69 提供内容检查，公共/浏览器 e70 仍缺失。 |
| 显式 HTTP 到授权内容 | W13 ⇢ W12 ⇢ W11 ⇢ E02 | 独立路由经进程内 HTTP 与真实 SQL 测试；未挂载到应用，也未连接浏览器或登录签发。 |

“已接线”不等于本轮做过真实端到端验收。尤其应避免以下合并：

- Runner 不是通用持久 Run；Task 的 Cancelled 不自动取消 Runner 作业。
- Runner Agent 不是 AI Agent；自报 isolation 不是隔离证明。
- 原子 outbox 记录不是可靠事件投递；旧 EventBus 不是新业务事件总线。
- 本地语言 worker 不是外部 LSP；模块清单不是可执行插件。
- 旧教学 feedback 不是通用 Review；Review 存在不等于 Task 已验收。
- 手工 VM 内核测试不是产品托管 VM 生命周期；工作区角色也不是部署 Grant。

## 下一轮沿图推进的顺序

以下是依据缺边提出的建议，不是已实现的承诺。先闭合一个私人、非教学保存流程，再扩展共享和执行。

| 顺序 | 目标坐标或边 | 需要补齐的合同与验证 |
|---|---|---|
| 1 | P01、e36/e37/e70 | W10–W13 已提供契约至未挂载 HTTP 适配。继续定义安全 Session 签发、显式安装、公共发布、整份草稿映射及浏览器冲突处理；不默认切换旧实例。 |
| 2 | P02、e38/e39 | 准备层的显式接入方案；如涉及旧数据，先验证备份恢复、迁移与旧写入隔离。没有恢复证据不把部署成熟度升为 3。 |
| 3 | U02 → P01 → W05/W06/W09 | 显式保存、重读、重启恢复、权限撤销、并发编辑与结果不确定；发布内容失败或换版失败都不盲删或盲重试。 |
| 4 | P03、P04、P05 | 分享和审阅授权；精确内容到类型化证据，再到版本绑定规则 Review；验收策略单独定义。 |
| 5 | P06、P07、P08 | 持久 Run、可恢复投递、隔离资源分别建立边界与故障测试，不靠扩大当前 Runner 权限代替。 |

继续抓虫时按边挑测试：U02/U04/U06 看代次、修订、异步替换；I04→W* 看撤权与锁等待；
W*→E02 看提交/回执失败；P* 缺边先写合同与拒绝用例，不能“测不存在的闭环”得到零用例通过。

## 证据目录与新旧分离

| 证据 ID | 日期 | 记录及限界 |
|---|---|---|
| [content-http](task-content-http.md) | 2026-10-03 | 私有 Unix socket 上的 11 项 PostgreSQL 16 HTTP → C2-M 用例；只验证进程内路由、当前授权和存储故障，不是监听/浏览器/TLS 或在线认证切换。完整门禁和此前套件数量另见项目进度。 |
| [session-content](session-task-content.md) | 2026-10-03 | 私有 Unix socket 上的 25 项 PostgreSQL 16 会话内容用例；核验旧新文本、授权、命名空间与事务边界，不是公共/浏览器保存。完整门禁和既有存储回归另见项目进度。 |
| [content-storage](task-content-store.md) | 2026-10-03 | 私有 Unix socket 上的 21 项 PostgreSQL 16 内容存储用例；仅可信 Schema 3，不是 Session 或浏览器保存。共用存储回归数量另见项目进度。 |
| [content-contract](task-content-manifest.md) | 2026-10-03 | 12 项元数据与 14 项给定字节回归；纯内部契约，不是授权存储或公共保存。整体检查另记于项目进度。 |
| [release-049](../../docs/en/project-status.md) | 2026-10-03 | 0.4.9 发布门禁：默认 Rust、前端、脚本、SDK 和独立 59 项浏览器；395 项 opt-in 忽略，不含新数据库/内核/LAN 验收。 |
| [bughunt-local](../../docs/en/project-status.md) | 2026-10-03 | 上一轮未发布抓虫：165 前端、92 脚本、七套 72 浏览器；Engine 模拟，本地 Monaco/导入下载真实。 |
| [c2-database](../../docs/en/project-status.md) | 2026-10-03 | C2-J 的 155 项选定 Task/Artifact/Review PostgreSQL；不是全部身份与会话数据库套件。 |
| [c1-identity](../../docs/en/collaboration-identity-lifecycle.md) | 2026-09-27 | 原 C1-D 身份绑定退役及此前注册表策略 SQL；不代表当前部署。 |
| [c1-policy](../../docs/en/collaboration-policy-audit.md) | 2026-09-27 | 原 C1-C 成员和部署策略审计 SQL；无在线切换。 |
| [c1-auth](../../docs/en/collaboration-auth-source.md) | 2026-09-27 | 原 C1-E 持久认证源 SQL；公共 AuthService 未切换。 |
| [c1-session](../../docs/en/collaboration-session-commands.md) | 2026-09-27 | 原 C1-F 会话事务组合 SQL；内部准备层。 |
| [c1-password](../../docs/en/collaboration-password-change.md) | 2026-10-02 | 2026-10-02 改密及明确列出的此前回归，包含删除账号；无在线恢复验收。 |
| [c1-provision](../../docs/en/collaboration-provisioning.md) | 2026-10-02 | 本地显式 CLI 与隔离 PostgreSQL/SCRAM；不是已装平台切换。 |
| [c1-reconcile](../../docs/en/collaboration-reconciliation.md) | 2026-10-02 | C1-K 的 2026-10-02 只读核对 SQL/CLI；日期另见开发历史，非修复或迁移。 |
| [design](../../docs/en/project-status.md) | 2026-10-03 | 本次复核的当前缺口与建议步骤，仅设计依据，不是测试。 |
| [target](../../docs/zh-CN/next-architecture.md) | 2026-10-03 | 本次复核的目标架构；原 0.4.3 盘点与外部项目假设保留原范围，未新验证。 |
| [agent-history](../../reports/releases/0.4.3/README.md) | 2026-09-25 | 0.4.3 历史签名 loopback HTTP + Clang，V3 仅对该服务边界；非当前 LAN、TLS 或远程内核。 |
| [kernel-history](../../reports/acceptance/2026-09-09-kernel-vm/README.md) | 2026-09-09 | 旧手工 VM、dirty native build、candidate=null；仅参考，不提高当前内核 V 分，不证明产品 VM 管理。 |

这里的日期归属于相应记录；design/target 的日期是本次复核日期，非新测试日期。
首次张量盘点新增清单和一致性校验；C2-K 纯契约及 C2-L 至 C2-N 隔离数据库执行单独记录。历史前端、浏览器、发行与数据库数量不合并成新验收。
V3 允许范围明确的历史真实数据库/服务证据；R04 的旧 loopback 服务证据只支持其命名协议范围，
内核历史则仅保留为参考，不能代表当前完整运行、挂载和部署路径。

## 维护与校验

变更能力时，先更新 JSON 的坐标、源码/测试引用、对应证据日期和缺口，再更新本页及英文阅读视图。
已连接边应说明授权拥有者、错误/取消结果和持久化责任；改分必须说明哪个门槛获得了新证据。
不要修改历史 0.3.8 的 [functional-network.json](../functional-network.json) 来伪装本轮覆盖。

```bash
node scripts/check-capability-tensor.mjs
node --test scripts/tests/capabilityTensor*.test.mjs
node frontend/scripts/sync-course-docs.mjs
node frontend/scripts/sync-course-docs.mjs --check
```

`renderTensorRows(data, locale)` 从清单生成上面的评分表；替换两份文档中
`capability-tensor:start/end` 标记之间的内容。公共脚本测试会拒绝表格与机器数据漂移。
结构校验拒绝悬空引用、重复坐标、越界路径、错误评分、将规划边写成已连接，以及用模拟/历史证据冒充
C3/O3；评分真实性仍需人工审阅，它不运行被引用的测试、不检测实现内容变化，也不是安全认证。

常用切片：`architecture == work` 看资源主线；`C == 1 && V == 3` 找已测但尚未接入的准备层；
`edges.state == missing` 找阻断；按 `implementation` 反查源码和测试，再按证据 ID 查运行范围。
不要对未知坐标补零，也不要跨这些切片计算平均完成率。
