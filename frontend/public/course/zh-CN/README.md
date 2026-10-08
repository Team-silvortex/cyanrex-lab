# Cyanrex 项目文档

Cyanrex 正从 eBPF 教学应用转向面向人类、AI Agent 与计算资源的自部署协作平台。现有教学运行时
继续可用；通用身份、内容、任务和审阅服务属于显式配置的准备层，任务 payload 编辑器目前只保存
本地草稿。自主 AI 与通用 Run 编排尚未实现。

本索引覆盖整个项目，不只覆盖首个教学领域。源码版本为 0.5.4，新增内部草稿意图记录、日志化派发
及独立日志/资源检查（C2-T–W），并更新 Next.js 至 15.5.27。此前 C2-O–S 准备层、AI 宿主适配与
安全边界保持不变；源码收录不接通通用浏览器保存或恢复。
以下文档区分源码发布内容、现用运行能力和目标架构，
模块存在或测试通过不表示已有在线部署。

0.5.4 收录的 [C2-T 意图记录](session-task-draft-intent.md)另加会话授权的登记与读取；
[C2-U 日志化发布](session-task-draft-dispatch.md)新增具有逐资源写前步骤的显式存活包装器，
不恢复尝试，也不接通浏览器保存。
[C2-V 日志检查](session-task-draft-journal-inspection.md)另以当前权限读取已记录步骤，
不检查资源内容、不复活包装器，也不授权重试。
[C2-W 日志步骤观察](session-task-draft-journal-observation.md)另在同一授权事务中读取一个
已记录目标的实际内容；不含正文的比较结果不是回执或恢复协议。

## 按问题选择入口

| 你想了解什么 | 先读这里 |
|---|---|
| 项目定位与整体边界 | [系统架构](architecture.md)、[平台与教学概念](concepts.md) |
| 当前全部模块及连接关系 | [当前平台功能地图](platform-network.md) |
| 按架构、功能、实现和成熟度切片 | [功能张量与证据评分](capability-maturity.md) |
| 哪些已完成以及下一步 | [项目状态](project-status.md)、[目标架构](next-architecture.md) |
| 编辑笔记、代码或配置等任务内容 | [任务 payload 编辑器](editor.md) |
| 配置 AI Agent 宿主并适配明确选出的工具 | [AI Agent SDK 接入](ai-agent-integration.md) |
| 查看内部服务端内容结构与精确快照校验 | [任务内容清单](task-content-manifest.md) |
| 查看独立内容持久化与原子编辑 | [任务内容存储](task-content-store.md) |
| 查看同一事务内的当前会话与内容字节检查 | [会话授权任务内容](session-task-content.md) |
| 查看显式构造且尚未挂载的 HTTP 边界 | [任务内容 HTTP 适配](task-content-http.md) |
| 查看严格草稿导入与纯发布映射 | [任务草稿发布计划](task-draft-publication.md) |
| 查看会话逐步发布及部分结果 | [会话授权草稿发布](session-task-draft-publication.md) |
| 观察未知步骤的目标而不恢复尝试 | [草稿目标只读观察](session-task-draft-observation.md) |
| 导出报告进度为数据而不复活尝试 | [草稿元数据检查点](session-task-draft-checkpoint.md) |
| 另行提供当前权限检查一个检查点目标 | [检查点目标显式检查](session-task-draft-inspection.md) |
| 登记和读取不可变意图而不执行发布 | [会话草稿意图记录](session-task-draft-intent.md) |
| 先登记步骤，再在资源事务内共同提交完成标记 | [日志化会话草稿发布](session-task-draft-dispatch.md) |
| 检查日志记录进度而不恢复尝试 | [会话草稿日志检查](session-task-draft-journal-inspection.md) |
| 保留原日志身份并观察一个已记录步骤的资源 | [日志步骤观察](session-task-draft-journal-observation.md) |
| 使用或讲授现有 eBPF 流程 | [学生指南](student-guide.md)、[教师指南](teacher-guide.md)及下方实验 |
| 部署或运维可信实例 | [安全](security.md)、[课堂接入与 SSH](classroom-connection.md)、[Runner Agent](runner-agent.md)、[故障排查](troubleshooting.md) |
| 开发或验证改动 | [贡献指南](../../CONTRIBUTING.md)、[当前测试指南](testing-guide.md)、[SDK](../../sdk-js/README.md)、[工具](../../scripts/README.md) |
| 查阅历史证据 | [开发历史](development-history.md)、[历史验收](acceptance.md)、[历史测试网络](testing-network.md)、[0.3.8 功能基线](functional-network.md) |

## 平台契约与实现依据

先理解概念模型，再选择具体适配器。直接存储接口的所有者过滤不等于认证；公共教学 API 仍使用原有
运行时，通用 Session 命令不会自动成为 HTTP 接口，也不会替换在线登录。

| 边界 | 设计与实现文档 |
|---|---|
| 基础与作用域身份 | [基础契约](collaboration-foundation.md)、[身份注册表](collaboration-identity-store.md)、[成员与策略](collaboration-access-store.md) |
| 审计生命周期与命令 | [策略审计](collaboration-policy-audit.md)、[身份生命周期](collaboration-identity-lifecycle.md)、[当前会话命令](collaboration-session-commands.md) |
| 持久认证 | [账号与会话来源](collaboration-auth-source.md)、[账号删除](collaboration-account-deletion.md)、[密码修改](collaboration-password-change.md) |
| 显式初始化与观察 | [全新引导](collaboration-bootstrap.md)、[本地初始化 CLI](collaboration-provisioning.md)、[只读对账](collaboration-reconciliation.md) |
| 任务与领域定义 | [任务领域解耦](task-domain-boundary.md)、[任务实例](task-instance-store.md)、[目录准入](session-catalog-tasks.md) |
| 不可变内容 | [制品修订](artifact-revision-store.md)、[私人制品命令](session-artifact-commands.md) |
| 任务内容访问与修改 | [私人手工任务](session-task-commands.md)、[准确输入](session-task-inputs.md)、[Draft 输入换版](session-task-revisions.md) |
| 判断与历史 | [审阅记录](review-record-store.md)、[会话授权的私人人工审阅](session-review-commands.md) |

准备层不会迁移现有教学记录。Task Schema 2 仅支持全新安装，存储格式版本不等于软件版本。代码是
可选任务内容，不是 Task 的必填字段。发布内容、替换任务输入、执行 Run 和记录 Review 是不同操作，
彼此不能推导成功，也不因此扩大跨用户权限。

## 现有运行时参考

教学包是首个实际运行的领域，保留教师管理部署、学生学习记录和内核执行边界。以下操作指南应与
尚未接入现用运行时的通用平台命令区分阅读。

- [学习存储与评语](learning-storage.md)、[事件流与恢复](event-stream.md)。
- [桌面与局域网隔离目标](classroom-isolation.md)：设计边界，不代表 VM 初始化已完成。
- [根目录快速开始与运行时 API](../../README.md#quick-start)。
- [模块清单](../../modules/README.md)：声明式目录，不等于可执行领域包。
- [故障排查](troubleshooting.md)：区分本地草稿、现有运行时和准备层存储的问题。

## 推荐阅读顺序

### 教师

1. [教师快速开始](teacher-guide.md)
2. [项目进度状态](project-status.md)
3. [系统架构](architecture.md)
4. [Runner Agent 使用指南](runner-agent.md)
5. [课程知识地图](concepts.md)
6. [安全与课堂部署](security.md)
7. 浏览全部实验并提前试跑
   - eBPF 页面中的课程路径：
     - `learning/foundations/beginner/fundamentals`
     - `learning/foundations/intermediate/protocols`
     - `learning-plus/cases/advanced/forensics`
     - `learning-plus/track/practice/operators`

### 学生

1. [学生快速开始](student-guide.md)
2. [课程知识地图](concepts.md)
3. 按顺序完成实验：
   - [实验 1：认识执行链路](labs/01-first-program.md)
   - [实验 2：观察 execve 系统调用](labs/02-trace-execve.md)
   - [实验 3：使用 eBPF Map 计数](labs/03-map-counter.md)
   - [实验 4：使用 Ring Buffer 传递事件](labs/04-ring-buffer.md)
   - [实验 5：读懂 Verifier 与调试程序](labs/05-verifier-debugging.md)
4. 遇到问题时查看[故障排查手册](troubleshooting.md)

## 课程完成标准

完成本课程后，学习者应能：

- 解释用户态、eBPF 程序、内核 hook 和 verifier 的关系；
- 根据场景选择 XDP、tracepoint 等 hook；
- 使用 Map 保存状态，使用 Ring Buffer 上报事件；
- 理解边界检查、空指针检查和有界循环为什么必要；
- 根据 clang 与 verifier 日志定位常见错误；
- 安全卸载程序并确认实验环境恢复干净。

## 运行模式

| 模式 | 实际目标内核 | 推荐场景 |
|---|---|---|
| WSL2 | WSL2 Linux 内核 | Windows 个人学习 |
| Docker | Linux 宿主机或 Docker Desktop VM 内核 | 快速试用、统一课堂环境 |
| Native Linux | 当前 Linux 内核 | 深入实验、最佳兼容性 |

eBPF 永远运行在 Linux 内核中。Windows 和 macOS 的 Docker 模式观察的是虚拟机内核，
不是桌面操作系统本身。

## 可选运行参数（高级）

事件量很大时，可在 `docker/.env` 调整持久化告警行为，减少教学现场告警噪音并便于排障：

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED`（默认：`true`）
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT`（默认：`80`）
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT`（默认：`40`）
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS`（默认：`10000`）

## CI 与合并门禁

- CI 流程已加入聚合任务 `CI gate`（位于 `.github/workflows/ci.yml`）。
- `CI gate` 会依赖 `security-audit`、`file-lengths`、`engine`、`frontend`、`sdk`、`permissions` 和
  `distribution`，并在任一任务失败时直接失败。
- 建议在分支保护中只配置必需检查项为 **`CI gate`**，这样合并统一受该门禁控制。
- 推送注解版本 Tag 会触发 `Release Candidate Validation`：它把干净的 Tag 提交绑定到新构建的
  离线发行包，完成精确镜像安装及真实 Aya 挂载/事件/detach 验收后，将结果保留为 30 天的
  工作流制品，并附带独立校验和保护、绑定候选产物的内核证据；统一离线验证器会在不解包的
  情况下流式检查归档、拒绝危险成员、复验包内所有文件，并把元数据与该证据交叉绑定；需要
  解包时，只有完整验证通过后才会以非覆盖方式逐文件写入。CI 与 Tag 验收均已使用该路径，
  不再直接展开 `tar`。该流程不会创建或签名 GitHub Release。
