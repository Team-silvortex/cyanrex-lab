# cyanrex-lab

[English](README.md)

版本：`0.6.1`

Cyanrex 正在发展为面向人、AI Agent 与计算资源的自托管协作平台。Task、内容版本、执行与评审各有独立职责；eBPF 教学是第一个领域包，而不是所有任务的定义。

当前应用仍运行教学流程。通用协作后端是显式准备层，新任务编辑器保留本地草稿，AI 编排尚未实现。

文档：[English](docs/en/README.md) · [简体中文](docs/zh-CN/README.md)
· 架构：[English](docs/en/architecture.md) · [简体中文](docs/zh-CN/architecture.md)
· 当前功能图：[English](docs/en/platform-network.md) · [简体中文](docs/zh-CN/platform-network.md)
· 能力成熟度：[English](docs/en/capability-maturity.md) · [简体中文](docs/zh-CN/capability-maturity.md)
· 进度与路线图：[English](docs/en/project-status.md) · [简体中文](docs/zh-CN/project-status.md)
· 测试：[English](docs/en/testing-guide.md) · [简体中文](docs/zh-CN/testing-guide.md)
· [更新日志](CHANGELOG.md)

cyanrex-lab 是采用 [Apache License 2.0](LICENSE) 的自由开源软件。欢迎贡献；请先阅读 [CONTRIBUTING.zh-CN.md](CONTRIBUTING.zh-CN.md)，遵守[社区行为准则](CODE_OF_CONDUCT.zh-CN.md)，并通过 [SECURITY.zh-CN.md](SECURITY.zh-CN.md) 中的私密流程报告漏洞。

## 当前开发快照

源码 0.5.9 的本地创作功能支持经审阅后追加多个文本文件，并保留原有载荷和所选编辑器：[English](docs/en/local-text-file-import.md) · [简体中文](docs/zh-CN/local-text-file-import.md)。这并未连接服务端保存或大文件交付。

截至 **2026-10-15**，源码版本为 **0.6.1**。本补丁收录显式
[Linux 启动检查](docs/zh-CN/platform-process-startup.md)、[私有配置](docs/zh-CN/platform-startup-input.md)、
[目录声明](docs/zh-CN/platform-catalog-expectation.md)、[存储诊断](docs/zh-CN/platform-storage-check.md)与
[保留存储绑定的连接](docs/zh-CN/platform-storage-connection.md)，并修正
[Session 数据库权限](docs/zh-CN/platform-session-permissions.md)。0.6.0 的 Task／设置／事件布局、
独立 Session 生命周期及存量限制、宿主目录、Artifact 根与标记准备层继续保留。
这些组件不挂载平台宿主、不签发 Session，也不证明完整 schema 兼容。
此前本地文本导入、五语言词典及显式文件下载／保存仍保留；
通用浏览器保存和分发仍不在正常应用启动路径中。
所有者私有的 [Task 元数据索引](docs/zh-CN/task-content-index.md)和未挂载的 HTTP 适配器仍包含在内。
[大文件分发准备](docs/zh-CN/large-file-distribution.md)现已包括有界流式处理、私有分块暂存、不可变上传配额、分块日志、整文件字节验证、可信发布、当前 Session 私有元数据，以及纯发布者/接收者授权契约。
已包含在 0.5.7 中的[发布者授权后续工作](docs/zh-CN/large-file-publisher-grants.md)增加了可信持久化撤销和当前 Session 的自身元数据读取。
独立的 [Session 授权管理](docs/zh-CN/large-file-grant-management.md)现在要求由宿主显式配置精确的管理者/账户世代；教师或部署角色并不赋予该权限。
独立的 [Session 发布](docs/zh-CN/large-file-session-publication.md)现在使用当前发布者权限处理真实字节，并执行原子授权插入；接收者交付和在线管理者撤销仍是独立工作。
独立的[接收者授权](docs/zh-CN/large-file-recipient-grants.md)现在持久化精确的文件/账户绑定，并提供当前自身接收者元数据读取；下载准入仍是独立工作。
独立的[发布者所有的共享](docs/zh-CN/large-file-sharing.md)现在依据当前发布者权限签发精确 Published 版本的授权，并允许原发布者独立撤销旧共享。
独立的[接收者下载清单](docs/zh-CN/large-file-download-manifest.md)现在以当前独立接收者授权核对精确的可信 Published 元数据；这并不授权后续字节读取。
独立的[已验证分块读取器](docs/zh-CN/large-file-chunk-read.md)现在在私有文件验证和原始权限重入后返回最多 4 MiB；返回的字节保留有界容量租约。
独立的 [HTTP 分块接收器](docs/zh-CN/large-file-chunk-receiver.md)现在对提供的精确响应一直验证到显式 EOF，再进行独占私有暂存；真实保存工作在取消后仍拥有其缓冲区/控制资源。它不会发起网络请求，也不会拼装或恢复整文件下载。
显式的[单分块 HTTP 客户端](docs/zh-CN/large-file-chunk-client.md)现在增加可信固定 HTTPS/字面量回环目标、私有逐次调用 cookie，以及独立的响应头/正文期限；等待者取消后，保存任务仍保留请求准入。它仍不在正常应用启动路径中。
独立的[私有整文件拼装](docs/zh-CN/large-file-chunk-assembly.md)现在将现有旧式分块复制到一个独占私有文件，并执行真实整文件摘要/读回检查；它不选择用户保存路径、不恢复丢失进度、不删除分块，也不挂载完整下载器。
读取器还会在私有 Session 工作期间保留原始源关系固定。此前的 Task 解耦、AI 宿主配置/SDK 适配器、C2-O–AA 草稿准备和认证保护仍包含在内。
参见[发布说明](CHANGELOG.md#061---2026-10-15)和[本地验证](reports/releases/0.6.1/README.md)。正常应用文件分发、已挂载的客户端交付以及重启恢复尚未连接。准备层 HTTP 适配器仍未挂载，编辑器仍仅在本地工作；本次发布或测试通过都不会切换旧认证或现有部署。

已包含在 **0.5.8** 中、此前不在 0.5.7 Tag 内的[有序私有下载](docs/zh-CN/large-file-download-client.md)，现在将顺序 HTTP 分块连接到同一根目录中的整文件拼装。共享流程容量在取消后仍跟随真实文件工作；失败会保留证据并停止。UI 保存/恢复仍未提供。

独立的[接收者清单 HTTP](docs/zh-CN/large-file-manifest-http.md)通过未挂载的 JSON GET 暴露精确授权元数据。新的[精确清单客户端](docs/zh-CN/large-file-manifest-client.md)可获取完整预期版本，并组合元数据 → 有序分块 → 私有拼装；即使空文件也必须发起元数据 GET。后续分块授权和 UI 保存/恢复仍是独立工作。
新的[显式私有保存](docs/zh-CN/large-file-save.md)将原始已确认 `f1` 复制到另一个私有目录中由宿主选定的新文件，不覆盖文件，也不采用已有回执。内部原始描述符证明保持私有；UI 文件选择和恢复仍未提供。

新的[授权交付调用](docs/zh-CN/large-file-delivery-client.md)将这些阶段连接成一个显式操作。取消后，真实分块/拼装/保存工作仍拥有同一个原始整流程租约；首个失败即停止，不重试、不采用已有结果，UI 保存、恢复和正常挂载仍是独立工作。

独立的[自有用户目录目标](docs/zh-CN/large-file-user-save.md)显式接受 0700/0750/0755，而不放宽私有暂存。原始目录 UID/GID/模式检查和经检查的保存仍然保留；文件选择、可写共享文件夹、恢复和实时挂载是独立工作。

可选的[实时交付观察器](docs/zh-CN/large-file-delivery-progress.md)报告每次调用的最新阶段及真实已确认分块前缀。缓慢或已丢弃的读取者不会增加消费队列；全部暂存完成和观察通道关闭都不等于保存文件成功、恢复或工作者终止。

可选的[协作式取消句柄](docs/zh-CN/large-file-delivery-cancellation.md)在取消被选中时停止本次调用的等待，而非已经派发的文件工作。丢弃句柄不等于取消；请求/完成竞态以返回结果为准，不自动清理或重试。

显式的[整流程 HTTPS 交付验证](docs/zh-CN/large-file-delivery-https.md)使用可丢弃的原生 TLS 对端检查这条已有链路。证书身份与当前文件授权相互独立；这些回环夹具并非已部署局域网或实时服务验收。

显式的[分发路由器组合](docs/zh-CN/large-file-distribution-router.md)从一个可信 Source/工作区派生清单与分块端点，使用独立 HTTP 预算。它仍未挂载，也不隔离特权 Engine 或签发 Session。

0.5.9 收录的 [Source/HTTPS 组合验证](docs/zh-CN/large-file-source-https.md)，在一条显式测试链中连接真实当前授权、原始文件、单一路由器与受控 TLS 交付。它不增加生产挂载、Session 签发或部署验收。

简明的[当前架构图](docs/zh-CN/architecture-map.md)将运行时接线、模块职责和变更边界与详细实现历史分开。服务分组和共享私有文件错误投影不会改变运行时挂载或权限策略。

0.5.4 C2-T 增加独立、不可变的意图登记，以及从真实未启动草稿尝试中、经当前 Session 授权的读取。它存储有界私有元数据，而非文本或凭据。原始派发器不受日志控制；重启恢复、重试和浏览器保存仍是独立工作。参见[草稿意图记录](docs/zh-CN/session-task-draft-intent.md)。

0.5.4 C2-U 为原始尝试增加一个可选、消费式的日志派发器。它在开始资源工作前提交每个步骤记录，然后在另一笔事务中重新检查当前所有权和 Session，并将资源 SQL 与其标记共同提交。新的 schema-2 日志需要显式使用；schema 1 不迁移。不确定结果会停止，不重试、不提供重启恢复，原始 C2-P 路径仍独立。参见[日志派发](docs/zh-CN/session-task-draft-dispatch.md)。

0.5.4 C2-V 在当前 Session 的精确所有者/账户绑定下，独立检查 schema-2 日志。它报告已记录的提交步骤和可选的 Unknown 目标，不读取资源、不恢复包装器，也不授权重试。参见[日志检查](docs/zh-CN/session-task-draft-journal-inspection.md)。

0.5.4 C2-W 在与日志捕获及最终复查相同的授权事务中，独立观察某个已记录步骤的真实资源。结果区分已记录状态与元数据一致性，且不返回正文；它不是回执、恢复或重试协议。参见[日志步骤观察](docs/zh-CN/session-task-draft-journal-observation.md)。

0.5.4 的 Next.js 15.5.27 依赖更新处理了新报告的缓存投毒安全公告；它不会更新正在运行的部署。参见[依赖审计策略](docs/zh-CN/security.md#依赖扫描治理)。

**已包含在 0.5.5 中的 C2-Y 后续工作**增加了独立的[目录 Task 内容 HTTP 适配器](docs/en/catalog-task-content-http.md)（[简体中文](docs/zh-CN/catalog-task-content-http.md)）。创建时选择精确配置的定义；旧手工接口保持不变。两者都是未挂载的准备层，没有 Session 签发器、自动评估、发布/日志集成、实时 OpenAPI/SDK 暴露或浏览器保存。

独立的 **已包含在 0.5.5 中的 C2-Z 后续工作**增加了[目录草稿发布](docs/en/session-catalog-task-draft-publication.md)（[简体中文](docs/zh-CN/session-catalog-task-draft-publication.md)）。纯准备阶段冻结精确完整定义；每次显式推进发布一个 Artifact，或最终创建其目录 Draft。新的不透明尝试不能进入旧手工检查点/日志/观察类型，也不提供 HTTP、评估、持久恢复、自动重试、签发器、浏览器保存或迁移。

独立的 **已包含在 0.5.5 中的 C2-AA 后续工作**增加了[目录目标观察](docs/en/session-catalog-task-draft-observation.md)（[简体中文](docs/zh-CN/session-catalog-task-draft-observation.md)）。一次当前 Session 读取将精确的未知目标与冻结定义和计划文本比较；无正文结果绝不改变尝试状态、不确认此前写入，也不允许重试。公开保存和持久恢复仍是独立工作。

| 领域 | 已实现边界 | 仍然独立 |
|---|---|---|
| 现有教学流程 | eBPF 编辑、检查、执行、学习历史与教师反馈 | 共享内核执行并不提供学生隔离 |
| Task 载荷编辑器 | 本地 Task 草稿、可选文本项和 14 个语言配置 | 此编辑器没有服务端保存、外部 LSP 或代码执行 |
| 协作准备 | 显式身份/Session、Task、Artifact、Review 存储与经授权的私有命令 | 没有实时认证切换或跨用户协作 UI |
| 草稿输入替换 | 所有者/修订检查和精确新旧 Artifact 验证 | 内容发布独立；浏览器保存/冲突处理尚未连接 |
| Task 内容 | Schema-3 Task/清单/outbox 事务、精确文本检查与独立 HTTP 边界 | 没有公开 Session 签发器、应用挂载或浏览器保存 |

代码是一个可选载荷项，而非 Task 必填字段。Task 存储 schema 2 和独立的内容 schema 3 仍是显式准备格式，不会在两者之间自动迁移；存储版本与产品版本独立演进。
请从[英文文档索引](docs/en/README.md) / [文档索引](docs/zh-CN/README.md)，以及[英文当前进度与下一步](docs/en/project-status.md#next-decision-points) / [当前进度与下一步](docs/zh-CN/project-status.md#下一阶段决策点)开始。带日期的测试和网络快照保留为历史证据。

## 下一代架构

平台将原始教学尝试中合在一起的四类工作分开：

| 概念 | 职责 | 当前边界 |
|---|---|---|
| Task | 目标、定义、所有者、精确输入与生命周期 | 私有持久化命令；没有通用完成或验收策略 |
| Artifact | 不可变内容修订与摘要 | 私有文件和事务元数据；独立于任务类型 |
| Run | 一次执行尝试及其资源 | 通用模型处于准备阶段；现有 eBPF Runner 仍是活跃执行路径 |
| Review | 绑定版本的判断与评论历史 | 私有人类命令；没有通用跨用户评审者授权 |

Identity 和 Workspace 划定资源所有权范围。当前 Session 命令在存储事务内重新检查权限；提供所有者 ID 不等于认证。领域包定义有类型的证据和评估规则，而不会成为平台的身份或持久化模型。现有教学适配器使用内置 eBPF 包；准入目录定义并不会执行其策略或自动批准 Task。

目前并存三条路径：

- **教学运行时：**浏览器 → 现有已认证 API → 教学存储与本地 eBPF Runner。
- **本地 Task 编辑：**Task 草稿 → 所选文本载荷 → 本地语言提供器 → 显式下载。
- **协作准备：**配置的持久 Session Source → 经授权的私有 Task/Artifact/Review 命令。

后两条路径没有通过公开 Task API 连接。Task 草稿下载不等于服务端发布。发布 Artifact 不会将 Task 移到其最新修订，私有 Review 也不会验收 Task。Runner Agent 是远程编译/探测工作者，而不是自主 AI Agent。

模型和迁移决策见[概念](docs/zh-CN/concepts.md)、[系统架构](docs/zh-CN/architecture.md)和[下一代设计（简体中文）](docs/zh-CN/next-architecture.md)。该设计仍是目标，不代表进程隔离、通用 Run、AI 协调、跨用户协作或实时迁移已完成。开发时间线独立保存在 [English](docs/en/development-history.md) / [简体中文](docs/zh-CN/development-history.md)中。

## 仓库结构

```text
cyanrex-lab/
├ frontend/        # Next.js shell, local task/editor features and teaching UI
├ engine/          # Rust/Axum runtime and explicit collaboration preparation
│  └ src/
│     ├ models/collaboration/ # domain-neutral identity and work contracts
│     ├ services/            # live services and separate durable stores/commands
│     └ domain_packs/        # built-in eBPF teaching definitions and rules
├ sdk-js/          # typed browser/Node.js API client
├ modules/         # versioned module catalog and manifest contract
│  ├ module-ebpf
│  ├ module-network
│  └ module-protocol
├ scripts/         # development, validation, provisioning and release tools
├ CHANGELOG.md      # auditable release history and unreleased changes
├ docs/            # English/Chinese platform, runtime, course and historical evidence guides
├ docker/          # compose and container assets
└ start.sh         # unified launcher
```

浏览器是交互层。当前特权 Rust Engine 同时承担控制与执行：实时认证、授权、持久化、编译、eBPF 加载和事件流。通用存储没有接入这一实时 AppState。在添加服务、路由或部署模式前，请先阅读架构文档；代码层面的分离不等于独立操作系统进程。

## 当前能力

本节描述正在运行的应用，以及明确标注的仅本地新增功能。通用协作命令由[平台功能图](docs/zh-CN/platform-network.md)说明，不能从下列教学 HTTP 端点推定它们已接入。

- TDD 优先的后端工作流（`engine/tests/routes_tdd.rs`）
- Axum API 服务提供：
  - 版本化模块发现与基于目录的控制端点
  - eBPF 执行流水线端点（`/ebpf/run`）
  - 仅编译的 clang 诊断（`/ebpf/check`）
  - 语义 C/eBPF 补全（`/ebpf/complete`）
  - eBPF 模板目录（`/ebpf/templates`）
  - eBPF 附着端点（`/ebpf/attachments`、`/ebpf/attachments/details`、`/ebpf/detach`）
  - eBPF 内核跟踪流（`/ws/events`，以及 `/events` 快照）
  - eBPF 附着诊断事件（`ebpf.attach_verified / ebpf.attach_missing / ebpf.attach_not_applicable`）
  - 辅助环境检查端点（`/helper/environment`）
  - 用户脚本端点（`/scripts`、`/scripts/save`、`/scripts/delete`）
  - 事件设置端点（`/settings/events`）
  - C 头文件模块端点（目录/下载/删除/选择/注入元数据）
  - 学生实验和教师总览的学习进度端点
  - 用于模块生命周期与实验交接的结构化教师管理命令总线（`/command`）
  - 生成的 OpenAPI 3.1 契约（`/openapi.json`），包含 Engine 路由/访问和 SDK 覆盖检查
- 认证系统：
  - 注册/登录/退出/会话（`HTTP cookie`）
  - OTP/TOTP 验证
  - OTP 初始化二维码流程
  - 通过 `current_password + otp` 修改密码
  - 通过 `password + otp` 删除账户
- 密码安全：
  - 不存储明文
  - Argon2 密码哈希，并兼容旧哈希验证
  - 持久化 Session token 存储为 SHA-256 摘要
- 认证持久化：
  - PostgreSQL 支持的 `users` + `sessions`
  - `CYANREX_DB_FALLBACK` 允许时提供有限的易失回退；这不保证持久写入
  - 配置为持久化的退出/密码修改/账户删除在写入未确认时拒绝继续；经 SQL 准入的登录不能静默发布仅存于内存的 Session
  - 参见[数据库中断边界](docs/zh-CN/security.md#会话与数据库故障)
- 前端页面：
  - `/dashboard`、`/ebpf`、`/tasks/new`、`/editor`、`/learn`、`/teaching`、`/helper`、`/modules`、`/events`、`/terminal`
  - `/login`、`/register`、`/join`、`/otp-setup`、`/account`
  - 由教师管理的 Terminal，提供结构化结果、模块快照和会话历史
  - 0.4.9 增加的 `/tasks/new`（也可通过 `/editor` 进入）：本地 Task 草稿拥有最多 32 个文本载荷项；代码可选，通过 14 个内置语言配置编辑。经确认的文件/整 Task JSON 导入和显式下载，不代表服务端 Task/Artifact 保存、执行或自动保存。JS/TS 工作者共享使用，并非多工作区安全沙箱；持久化与导航限制见[编辑器指南](docs/zh-CN/editor.md)。
- JavaScript SDK：
  - 覆盖面向浏览器的 Engine API 的有类型 ESM 包
  - 从 90 个 OpenAPI 组件 schema 生成公开请求/响应模型
  - 为全部 65 个非 Runner-Agent 操作生成 `client.operation(operationId, input)` 层与运行时元数据，与稳定的手工设计命名空间并存
  - `@cyanrex/sdk-js/agents` 中提供 AI 宿主配置辅助工具和显式工具格式适配器；没有提供器传输或自主委派，写入需要逐次调用的可信批准
  - 为 77 个公开客户端命名空间和方法路径提供仅增量兼容基线
  - 浏览器凭据、Node Session cookie 捕获、基于 Origin 的 CSRF 支持、取消与有类型错误
  - 根据生成 API 契约中的每个非 Runner-Agent Engine 操作检查覆盖
  - dry-run 包清单、声明闭包和 ESM 消费者导入验收检查
- 前端 i18n（国际化）：
  - 支持语言：简体中文（`zh-CN`）、英语（`en`）、西班牙语（`es`）、日语（`ja`）
  - 已集成侧边栏、认证页面与核心运行时页面
  - 语言偏好持久化到浏览器本地存储
- 事件中心：
  - 用户范围内的持久化事件存储
  - 类别划分：`kernel` / `platform`
  - 严重程度与颜色：成功=绿色，警告=黄色，错误=红色
  - 侧边栏未读标记（红点和数量）
  - 按过滤条件导出（`/events/export`）
  - 按相同过滤条件删除（`/events/delete`）
  - 每用户事件保留设置：
    - 最大保留记录数
    - 溢出策略：`drop_oldest` / `drop_new`
- 页面状态持久化（sessionStorage）：
  - 辅助报告缓存
  - 事件过滤状态
  - eBPF 编辑器与运行时控件

## 快速开始

完整文档入口：[Cyanrex 项目文档](docs/zh-CN/README.md)。现有教学流程中，教师建议先阅读
[教师快速开始](docs/zh-CN/teacher-guide.md)，学生从
[学生快速开始](docs/zh-CN/student-guide.md)进入课程。

### 1) 启动服务

```bash
./start.sh
```

常用命令：

```bash
./start.sh start --mode auto   # WSL2-native inside WSL; Docker elsewhere
./start.sh start --mode wsl    # Engine targets the WSL2 kernel
./start.sh start --mode docker # Engine targets the Docker host/VM kernel
./start.sh start --mode native # Engine targets the native Linux kernel
./start.sh start --local
./start.sh start              # fast-start (default, no forced rebuild)
./start.sh start --debug      # enable detailed startup tracing
./start.sh start --rebuild    # force rebuild when deps/Dockerfile changed
./start.sh start --pull       # pull base images before start
./start.sh start --skip-conflict-check  # skip preflight port/project conflict check
./start.sh start --instance-id room-a    # same instance starts are auto-locked by instance ID
./start.sh start --skip-start-lock       # bypass start-lock when debugging only
./start.sh diagnose          # run system-level diagnostic checks
./start.sh status
./start.sh logs
./start.sh stop
```

eBPF Engine 始终运行在 Linux 内核上。在 Windows 中，可使用 WSL2-native 模式直接访问 WSL 内核，或使用 Docker 模式构建可丢弃的教学沙箱。Docker 模式有意使用特权运行，不能暴露给不可信用户。

### 2) 打开 UI

- 前端：`http://localhost:3000`
- Engine 健康检查：`http://localhost:8080/health`
- Postgres：`127.0.0.1:15432`（不暴露到局域网）

现有教学流程使用 `/ebpf` 和 `/learn`。在当前开发树中，`/tasks/new` 打开本地 Task 载荷编辑器（`/editor` 是兼容入口）。依赖持久化前请阅读[编辑器指南](docs/zh-CN/editor.md)：它不会保存到服务端。启动应用不会初始化或迁移独立的协作存储。

### 2.5) 构建分发包

用于课堂部署或离线分发时，可创建包含预构建 Docker 镜像的打包产物：

```bash
./scripts/package-distribution.sh --version 0.5.5
```

该命令产生：

- `dist/cyanrex-lab-0.5.5-<timestamp>.tar.gz`
- `dist/cyanrex-lab-0.5.5-<timestamp>.tar.gz.sha256`

归档包含 PostgreSQL、Engine 和前端镜像。在可丢弃的 Docker 宿主上，用 `./install-smoke.sh` 对刚解出的包进行端到端验证。它检查包清单与 `release-metadata.json`，在禁用镜像仓库拉取的情况下加载离线镜像，生成临时密钥，启动所有必需服务和可选编译 Agent，执行已认证探测，然后删除临时栈和卷。发布元数据记录源码修订及 clean/dirty 状态、存在时匹配的带注释版本 Tag、镜像引用/内容 ID/构建模式以及镜像归档 SHA-256，不暴露构建宿主路径。安装验收固定包内镜像引用，并在探测服务前验证全部三个已加载 Docker 内容 ID。元数据绑定校验和，但未签名。冒烟测试拒绝覆盖已有 `.env`。安装验收使用随包提供的 Rust CLI 验证包、检查已加载镜像身份并检查 Engine 健康状态；默认打包路径不再要求 Docker 宿主安装 Python。
仅在可丢弃的特权 Linux 宿主上设置 `CYANREX_SMOKE_RUN_LIVE_KERNEL=1`，以额外运行包内的 `live-kernel-smoke.sh`：它附着内置 Aya `sched_switch` 环形缓冲程序，要求一个唯一标识的真实内核事件，分离精确 pin，并拒绝残留附着。新包通过相邻原生 `cyanrex-release smoke live-kernel` 客户端运行该流程；脚本保留 shell/Python 路径，以兼容旧包或手工组装的包。
设置 `CYANREX_KERNEL_SMOKE_REPORT=/safe/output/report.json` 可原子写入机器可读证据；脚本拒绝覆盖已有报告。

之后可针对精确的解包元数据验证该报告：

```bash
./cyanrex-release evidence verify /safe/output/report.json \
  --release-metadata ./release-metadata.json
```

新的离线包携带此原生 Rust 验证器，不携带独立 Python 发布辅助工具。Python 实现在源码 checkout 中保留，作为兼容和跨实现参考。

对于从 Tag 工作流下载的产物，将其四个文件放入专用目录，并在解包前，从匹配源码 Tag 的可信 checkout 验证完整候选包：

```bash
release_revision="$(git rev-list -n 1 v0.5.5)"
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  candidate verify /path/to/downloaded-candidate \
  --expect-version 0.5.5 --expect-revision "$release_revision" --expect-tag v0.5.5 \
  --extract-to /path/to/new-output-directory
```

该过程以流方式读取归档而不解包，拒绝链接、路径穿越、重复成员和未纳入校验和的载荷，再将精确归档元数据绑定到独立带校验和的实时内核证据。只有整个候选包通过后才开始解包；只向私有临时目录写入普通文件，保留脚本执行位，并拒绝已有输出路径。在配置发布签名之前，这些检查证明的是包的一致性，而非发布者真实性。

对于没有 Tag 验收报告、在本地构建的双文件包，可从可信源码 checkout 使用相同的已验证解包路径，但省略证据步骤：

```bash
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  package extract /path/to/archive-and-checksum \
  --output /path/to/new-output-directory
```

原 `scripts/release-package.py` 入口暂时保留以提供兼容。

目标机器上的用法：

```bash
cd /path/to/new-output-directory/cyanrex-lab-*
cp .env.example .env   # update credentials/bind settings as needed
./run.sh               # start
./stop.sh              # stop
```

包现在包含功能更丰富的 `deploy.sh` 辅助工具：

```bash
./deploy.sh up     # start (default behavior)
./deploy.sh status # show service status
./deploy.sh logs   # follow logs
./deploy.sh down   # stop services
```

`run.sh` 和 `stop.sh` 保留为兼容包装器。

如果 `.env` 缺失或仍含占位值，启动会失败，并明确提示先初始化密钥。

### 3) 私有部署教师账户

- 用户名：`admin`
- 角色：`teacher` — 个人使用时你就是教师；教学与部署共用同一身份
- 密码和 TOTP 密钥：首次启动时在 `docker/.env` 中生成
- `docker/.env` 模式为 `0600`，且被 Git 忽略；请勿发布它

首次启动前可通过环境变量覆盖：

- `CYANREX_ADMIN_USERNAME`（直接启动 Engine；受支持的启动器保留用户名 `admin`）
- `CYANREX_ADMIN_PASSWORD`
- `CYANREX_ADMIN_TOTP_SECRET`
- `CYANREX_ADMIN_USERNAMES`（旧式可选教师权限允许列表，用逗号/空格分隔）
- `CYANREX_TEACHER_USERNAMES`（为已验证现有账户授予完整教学/部署权限的可选配置）
- `CYANREX_ALLOW_MISSING_ORIGIN`（可选，默认禁用）— 允许受 CSRF 保护的状态变更路由没有 `Origin`/`Referer`
- 事件持久化调优（可选）：
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED`（默认 `true`）
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT`（默认 `80`）
  - `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT`（默认 `40`）
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS`（默认 `10000`）

注册和仅凭密码的 TOTP 初始化默认禁用。仅在有监督的实验环境中设置 `CYANREX_ALLOW_REGISTRATION=true`。核心 eBPF 和用户脚本供已认证用户使用。教师管理模块、编译器设置、Runner Agent、教学与 Terminal；学生不能使用这些管理路由。现有管理员账户/配置保持兼容，但登录和 Session 角色现在报告为 `teacher`。

单人使用只需要预置教师账户，不需要公开注册或管理员开关。公开注册拒绝保留的教师/旧管理员名称，且不能选择角色。部署教师不能删除自身。升级前请审阅两个允许列表：原教师现在获得完整部署权限。请一起升级前端和 Engine；任命、所有权和隔离限制见[教师指南](docs/zh-CN/teacher-guide.md)。

模块目录通过 v1 `module.json` 清单加入发现。Engine 启动时拒绝格式错误的清单，生命周期请求拒绝未知名称。启动/停止只更新单进程控制状态；Cyanrex 从不执行模块目录中的文件。在仓库或打包镜像之外运行时，可使用 `CYANREX_MODULES_DIR` 覆盖目录根。

## 认证 API（已实现）

- `POST /auth/register`
- `POST /auth/login`
  - 响应包含 `role`（`teacher` / `student`；`admin` 保留为旧 schema 值）
- `POST /auth/totp/bootstrap`
- `GET /auth/me`
  - 响应包含当前已认证 `role`
- `POST /auth/logout`
- `POST /auth/password/change`（需要认证 Session + OTP）
- `POST /auth/delete`（需要认证 Session + OTP）

## eBPF API（已实现）

- `POST /ebpf/check`
  - 仅编译的 clang 语法检查；从不向内核加载代码
  - 返回结构化行/列诊断
- `POST /ebpf/complete`
  - 在从一开始计数的光标位置执行 clang 语义补全
  - 返回头文件符号、宏、类型、函数与结构体字段
- `POST /ebpf/run`
  - 接受可选 `program_name`、`template_id` 和 `lab_id`
  - 接受可选 `runtime_backend`（`bpftool` | `aya`）
  - 支持用 `sampling_per_sec` 控制内核事件采样率
  - 支持用 `stream_seconds` 控制流持续时间
  - 支持 `enable_kernel_stream` 开关
  - 内核流优先使用 `ringbuf event_pipe`，回退到 `tracelog`
  - `aya` 后端当前面向 tracepoint 附着路径（对 sched switch 采样器提供一等支持）
- `GET /ebpf/attachments`
- `GET /ebpf/attachments/details`
- `POST /ebpf/detach`
- `GET /ebpf/templates`
  - 包括典型模板：`xdp`、`tracepoint`、`ringbuf skeleton`、`ringbuf high-freq sampler`
- `GET /events`
  - 事件快照
- `GET /ws/events`
  - 实时事件流
- `GET /settings/events`
  - 获取当前用户事件保留设置
- `POST /settings/events`
  - 更新用户事件保留设置（`max_records`、`overflow_policy`）
- `GET /helper/environment`
  - 运行时检查包括 `bpftool_autoattach`、`bpftool_link_show`、`btf_dump`、`bpffs_mount_type`、`runtime_context`

## Runner API（已实现）

- `GET /runner/status` — 已认证容量与显式隔离级别
- `GET /runner/overview` — 仅教师可见的活跃租约所有者与期限
- `POST /runner/agent/register` — 可选的 token 认证远程节点注册
- `POST /runner/agent/heartbeat` — 已注册节点的健康与容量心跳
- `GET /runner/agents` — 仅教师可见的远程节点清单
- `POST /runner/jobs/probe`、`/runner/jobs/compile-check`、`/runner/jobs/cancel` — 仅教师可用的远程任务生命周期
- `POST /runner/agent/jobs/claim`、`/sync`、`/result` — 已签名的 Agent 任务协议
- `GET /runner/jobs` — 仅教师可见的任务清单
- `GET /ebpf/check/backends` — 已认证、已脱敏的本地及合格 Agent 编译器清单
- `POST`、`GET /ebpf/check/remote` 和 `POST /ebpf/check/remote/cancel` — 用户范围内的异步远程诊断
- `/ebpf/run` 通过可替换的 `RunnerDriver` 边界执行
- 本地 Runner 默认最多两个全局任务、每用户一个任务，执行超时为 45 秒
- 显式报告 `shared_kernel`；配额不能代替每学生 VM 隔离
- Agent 任务使用逐节点 HMAC 凭据、防重放、租约、能力匹配、取消和有界结果
- 远程诊断需要编辑器显式选择，从不静默回退；本地检查仍为默认
- 可选编译任务使用受限 Clang，从不加载或返回 eBPF 对象；`/ebpf/run` 远程派发仍禁用
- `cyanrex-runner-agent` 将可信 Linux、WSL2 和非特权容器节点连接到该协议
- `scripts/runner-agent.sh start` 启用可选加固 Compose Agent；对应冒烟脚本验证已认证编辑器路径
- 教师可在 **部署与设置 → Runner Agent 运维** 中查看 Agent 健康/容量，并探测或取消近期任务

## 脚本 API（已实现）

- `GET /scripts`（用户范围列表）
- `POST /scripts/save`（用户范围创建）
- `POST /scripts/delete`（用户范围删除）

## 学习 API（已实现）

- `GET /learning/labs` — 当前用户的五个实验目录与进度
- `GET /learning/attempts` — 当前用户由后端记录的运行尝试
- `GET /learning/teacher/overview` — 教师/管理员课堂进度汇总
- `GET /learning/teacher/attempts?username=...&limit=...` — 仅教职人员可用的单个学生有界尝试、反馈和已提交源码审阅
- `POST /learning/teacher/feedback` — 教师/管理员在现有尝试上保存学生可见评论；需要 CSRF Origin 和 `{ username, attempt_id, comment, expected_revision }`
- 反馈为纯文本（去除首尾空白后 1–2000 个字符），评审者与时间由服务端指定。首次评论使用修订 `0`，之后使用最后读取的修订；过期编辑返回 `409`。反馈随尝试持久化，绝不改变自动完成结果或已提交源码。
- 学生可在 **学习 → 我的实验历史** 查看自身提交与教师反馈。
- 仅提供已知 `lab_id` 时，`/ebpf/run` 才记录尝试；完成结果依据真实运行、必需模板/源码模式以及附着验证计算。

## API 契约

- `GET /openapi.json` 无需认证即可提供版本化 OpenAPI 3.1 文档。
- [`engine/openapi/openapi.json`](engine/openapi/openapi.json) 从已注册 Axum 路由和维护的请求/响应 schema 生成。
- 质量门禁拒绝 Engine 与 OpenAPI 文档之间的路由或访问级别漂移，以及意外的 JavaScript SDK 覆盖或生成模型漂移。
- 打包后，SDK 消费者可从 `@cyanrex/sdk-js/openapi` 导入生成 schema，并从 `@cyanrex/sdk-js/operations` 导入操作元数据/类型。
- 冻结的 `0.3.0` 兼容基线拒绝删除操作、访问变更、收窄请求和削弱成功响应保证，但允许增量演进。

变更路由或线模型后，请重新生成并验证：

```bash
node scripts/generate-openapi.mjs
node scripts/openapi-contract.mjs
node scripts/api-compatibility.mjs
node scripts/generate-sdk-types.mjs
```

## 认证持久化

这些表属于现有运行时，而非显式持久协作 Source。其命名空间安装、账户世代与当前 Session 事务见[持久认证](docs/zh-CN/collaboration-auth-source.md)；不要让准备层安装器指向现有部署，也不要假设这些存储会自动同步。

- 表：`users`、`sessions`
- 迁移模板：`engine/migrations/0001_auth_users_sessions.sql`

## 数据持久化

- 事件中心使用 `event_records`（`engine/migrations/0002_event_records.sql`）
- 脚本存储使用 `user_scripts`（`engine/migrations/0003_user_scripts.sql`）
- 源码快照、自动反馈和实验进度使用 `learning_attempts`（`engine/migrations/0004_learning_attempts.sql`）

## 工程规则

- 本仓库采用 TDD 优先。
- 流程：`Red -> Green -> Refactor`。
- 后端路由变更先更新 `engine/tests/` 中的测试。
- API 变更需重新生成 OpenAPI 文档，并保持其组件 schema 最新。
- 只有在明确评审的兼容性重置中，才可替换兼容基线。
- 维护的源文件限制为 600 行；文档文件限制为 2000 行。
- 团队约定见 `TDD.md`。

提交变更前，运行与 CI 相同的检查：

```bash
./scripts/quality-gate.sh
```

`--format-only` 运行公共预检查和格式化，而不运行应用测试。默认、可丢弃数据库、浏览器、SDK 和特权验收边界见[测试指南](docs/zh-CN/testing-guide.md)。[历史网络](docs/zh-CN/functional-network.md)及其 [JSON 快照](docs/functional-network.json)是保留的证据，而非当前完整清单。

### CI 门禁（`ci-gate`）

- 工作流 `CI` 当前并行运行检查，再使用最终 `ci-gate` 任务汇总结果：
  - `security-audit`
  - `file-lengths`
  - `engine`
  - `frontend`
  - `sdk`
  - `permissions`
  - `distribution`
- 将分支保护设置为仅要求 `ci-gate`（而非每个矩阵式任务），这样任意任务失败都会导致必需门禁失败。
- 推荐的 GitHub 分支规则（GitHub UI）：
  - Settings → Branches → Branch protection rules
  - 为默认分支（例如 `main`）添加规则
  - 启用 **Require status checks to pass before merging**
  - 添加必需状态检查：`CI gate`
  - （可选）各子任务不单独勾选以减少噪音；`ci-gate` 已汇总失败。

### Tag 候选验收

推送新的带注释 `v<major>.<minor>.<patch>` Tag 会运行 `Release Candidate Validation`。验证不可变版本树后，它从精确提交构建离线归档，要求 `release-metadata.json` 报告相同 Tag/修订、干净源码和本地构建镜像，针对解出的归档运行 `install-smoke.sh`（包括已加载镜像内容 ID 检查），并在将通过验收的归档与外层校验和上传为保留 30 天的工作流产物前，启用特权实时内核附着/事件/分离验收。产物还保留独立带校验和的实时内核报告，绑定候选元数据、内核环境、唯一事件与精确清理；在 CI 和 Tag 验收中，已验证解包取代直接展开归档。统一候选验证器在上传前以流方式检查完整四文件产物，并可在下载后安全解包。这不会创建 GitHub Release 或签名包。
