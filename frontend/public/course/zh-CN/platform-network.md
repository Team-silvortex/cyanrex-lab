# 当前平台功能与模块网络

源码核对日期：**2026-10-03**。范围：**源码版本 0.4.9**，包括协作命令与本地任务 payload
编辑器。这是一份持续维护的当前源码地图，不是发布验收报告，也不代表正在运行的部署已更新。

Cyanrex 正从 eBPF 教学应用转向以任务为中心的协作平台。目前并存三部分：已连接的教学运行链路、
独立准备中的通用后端，以及仅在本地工作的任务编辑器。代码只是任务 payload 的一种可选内容，
任务和平台都不以代码为前提。后续重点是连接这些部分，同时不把旧教师角色或浏览器自报身份直接
变成通用平台权限。

## 状态说明

| 状态 | 本文含义 |
|---|---|
| 已接入运行链路 | 当前 Engine 或前端已实际组装；运行仍取决于相应部署条件。 |
| 后端准备层 | 已实现内部服务或显式本地运维工具并有测试，但未接入公共路由或常规启动。 |
| 仅本地 | 浏览器功能；内容不保存到 Engine，也不执行。 |
| 待接边界 | 当前源码尚未实现的连接或能力。 |

实际组装入口是 [`state.rs`](../../engine/src/state.rs)、
[`application.rs`](../../engine/src/application.rs)、
[`SidebarLayout.tsx`](../../frontend/src/components/SidebarLayout.tsx) 与
[`pages` 目录](../../frontend/pages)。新 Task、Artifact、Review 和 durable source 服务尚未进入
当前 `AppState`，也没有注册通用 Task、Artifact、Review 的公共 API。

## 浏览器入口

排除 Next.js 的 `_app`，目前共有 **18 个页面路由**。页面存在不等于获得权限：侧栏负责导航和会话
可见性，服务端动作仍由 Engine 守卫授权。

| 路由 | 当前职责 | 连接状态 |
|---|---|---|
| `/` | 跳转到仪表盘 | 已接入运行链路 |
| `/dashboard` | 工作区导航和概览 | 已接入运行链路 |
| `/login` | 密码与 TOTP 登录、安全返回路径 | 旧认证链路 |
| `/register` | 配置允许时的公开账户注册 | 旧认证链路 |
| `/otp-setup` | 密码认证后的 TOTP 配置 | 旧认证链路 |
| `/join` | 检查教师发现信息与兼容性、确认来源、消费邀请 | 旧课堂接入链路 |
| `/account` | 带确认的密码修改与账户删除 | 旧认证链路 |
| `/ebpf` | 源码、模板、保存脚本、检查、补全、运行与历史尝试恢复 | 已连接教学和 Linux 执行 |
| `/learn` | 实验进度、尝试与学习资料 | 学习 API 与文档入口 |
| `/learn/[...slug]` | 展示同步的中英文课程文档 | 静态课程内容，可跳转实验 |
| `/teaching` | 教师查看学生概览、尝试与反馈 | 旧教学授权和存储 |
| `/modules` | 教师管理头文件目录、下载、选用和删除 | 头文件服务，不是可执行插件安装 |
| `/events` | 历史、筛选、流、未读、导出与删除 | 按所有者隔离的事件 API 与 WebSocket |
| `/helper` | 环境和 Runner 就绪情况 | Engine 只读报告 |
| `/settings` | 事件和编译设置、指标、Agent 与课堂管理面板 | 现有设置、Runner 和课堂 API |
| `/terminal` | 结构化管理命令 | 类型化分发器，不是任意 shell |
| `/tasks/new` | 任务标题、可选文本 payload、本地导入导出与编辑 | 仅本地；外层仍检查登录 |
| `/editor` | 同一任务草稿容器的兼容入口 | 仅本地；不是第二套编辑器数据模型 |

本地草稿持有已接受的内容和本地修订号。受控编辑器持有 Monaco 模型与本地语言工具，不持有服务端
任务身份或执行权限。14 种语言配置包括 JavaScript/TypeScript 语义工具、JSON/CSS/HTML 结构化工具、
基础语言支持与纯文本；这不等于通用外部 LSP 服务、文件系统工程、Rust Analyzer、Pyright 或 clangd。
详见[编辑器说明](editor.md)。

## 已连接的运行模块

| 模块 | 主要源码 | 功能链与限制 |
|---|---|---|
| 运行组装与配置 | [`config.rs`](../../engine/src/config.rs)、[`state.rs`](../../engine/src/state.rs)、[`application.rs`](../../engine/src/application.rs) | 环境配置 → 服务组装 → 受保护的 HTTP/WebSocket 路由；不初始化协作准备层存储。 |
| 认证与教师权威 | [`auth_service.rs`](../../engine/src/services/auth_service.rs)、[`auth.rs`](../../engine/src/routes/auth.rs) | 账户 → 密码/TOTP → HTTP-only 会话 → CSRF/角色守卫；单人部署的初始教师也负责部署管理，这是现有权威模型。 |
| 课堂连接 | [`classroom.rs`](../../engine/src/services/classroom.rs)、[`课堂路由`](../../engine/src/routes/classroom.rs) | 教师描述/链接 → 独立协议和能力检查 → 绑定学生名的邀请 → 正常账户/会话流程；发现不等于认证教师或授权执行。 |
| 保存脚本 | [`script_store.rs`](../../engine/src/services/script_store.rs)、[`脚本路由`](../../engine/src/routes/scripts.rs) | 已认证所有者 → 保存/列举/删除 → PostgreSQL 或配置的本地持久化 → 显式加载到编辑器；保存脚本还不是通用 Artifact。 |
| 模块、头文件与命令 | [`module_manager.rs`](../../engine/src/services/module_manager.rs)、[`c_header_module.rs`](../../engine/src/services/c_header_module.rs)、[`command_dispatcher.rs`](../../engine/src/services/command_dispatcher.rs) | 清单发现 → 目录/控制状态；校验后的头文件 → 选用元数据 → 编译器；结构化终端命令 → 允许的模块操作/导航；模块启停不是动态代码加载。 |
| eBPF 工具与运行时 | [`eBPF 路由`](../../engine/src/routes/ebpf.rs)、[`ebpf_loader`](../../engine/src/services/ebpf_loader)、[`runner_manager`](../../engine/src/services/runner_manager.rs) | 源码/头文件 → 编译诊断或有边界的 Runner 操作 → Linux 加载/挂载 → 精确卸载与观测；内核操作有特权，不是对抗恶意学生的沙箱。 |
| Runner Agent | [`runner_agent_client.rs`](../../engine/src/services/runner_agent_client.rs)、[`runner_job_queue.rs`](../../engine/src/services/runner_job_queue.rs)、[`Agent 程序`](../../engine/src/bin/cyanrex-runner-agent.rs) | 签名注册/心跳 → 带租约作业 → 探测或编译报告；远程编译检查不引入远程内核执行，声明的隔离类型不是 VM/容器安全边界证明。 |
| 教学与学习 | [`learning_catalog.rs`](../../engine/src/services/learning_catalog.rs)、[`learning_store`](../../engine/src/services/learning_store)、[`学习路由`](../../engine/src/routes/learning.rs) | 实验 → 运行观测 → 规则评估 → 尝试/源码快照 → 教师反馈 → 所有者历史恢复；恢复只加载源码，不自动重跑。 |
| 事件与设置 | [`event_bus.rs`](../../engine/src/services/event_bus.rs)、[`事件路由`](../../engine/src/routes/events.rs)、[`设置路由`](../../engine/src/routes/settings.rs) | 运行发布 → 所有者历史/持久化 → WebSocket 交付或重新同步 → 浏览器；导出、删除与设置各有验证及确认写入边界。此 EventBus 不是通用存储的事务 outbox。 |
| 分发与部署 | [`cyanrex-release`](../../engine/src/bin/cyanrex-release)、[`docker`](../../docker)、[`脚本说明`](../../scripts/README.md) | 校验包与来源 → 审阅 SSH 目标及已安装包 → 显式 apply/验收；Rust 原生工具优先，保留兼容 Python/shell 工具；不隐含通用存储自动迁移。 |
| 公共集成契约 | [`openapi.json`](../../engine/openapi/openapi.json)、[`sdk-js`](../../sdk-js/README.md) | 已注册路由 → OpenAPI → 生成类型/操作 → SDK 使用方；签名 Agent 协议不进入浏览器 SDK，通用准备层命令也未进入公共契约。 |

现有教学门面已通过 [`TaskCatalog`](../../engine/src/services/task_catalog.rs) 连接内置
[`EbpfTeachingPack`](../../engine/src/domain_packs/ebpf_teaching/mod.rs)，抽取版本化定义与类型化规则
证据，但未改变旧教学 HTTP/存储链路。Domain Pack 是受信任的内置代码，不是下载式插件、执行调度器
或授权提供者。

## 协作后端准备层

共享模型区分 Authority、Principal、Workspace、成员关系、部署权限、Task、不可变 Artifact 修订及
Review。它们使用带范围的引用，不是用户名、路径、编辑器 ID 或客户端角色声明。
概念起点见[协作基础](collaboration-foundation.md)。

| 模块 | 主要源码 | 已实现连接与剩余限制 |
|---|---|---|
| 契约与旧系统投影 | [`models/collaboration`](../../engine/src/models/collaboration)、[`legacy_workspace.rs`](../../engine/src/services/legacy_workspace.rs) | 领域中立值类型与部分旧权限的纯预览；不是迁移或运行时授权。 |
| 持久化身份与策略 | [`collaboration_identity_store`](../../engine/src/services/collaboration_identity_store) | 显式命名空间 → 绑定、有效成员关系、部署授权与审计修订；不会隐式提升教师权限或收编环境中的数据库。 |
| 持久化认证与命令 | [`durable_source`](../../engine/src/services/auth_service/durable_source) | 账户世代/会话 → 同一事务内的审计身份、策略与生命周期命令；公共登录与账户端点仍使用旧 `AuthService`。 |
| 初始化与核对 | [`cyanrex-provision`](../../engine/src/bin/cyanrex-provision)、[`reconciliation`](../../engine/src/services/collaboration_identity_store/reconciliation) | 显式目标绑定的空命名空间初始化、私密交付配置材料及有界只读图核对；不是启动钩子、修复工具或旧数据导入器。 |
| Task 实例存储 | [`task_store`](../../engine/src/services/task_store) | 定义快照 + 精确输入 → 修订约束的生命周期 + 原子 outbox；当前新安装存储 schema 为 2，拒绝 schema 1，不做迁移。 |
| Artifact 修订存储 | [`artifact_store`](../../engine/src/services/artifact_store) | 私有不可变文件 + 元数据/outbox → 精确范围、修订和摘要读取；发布独立于任务变更，数据库写入失败可能留下私有文件。 |
| Review 存储 | [`review_store`](../../engine/src/services/review_store) | 精确目标/证据修订 → 不可变判断与评论历史；人工和规则判断分离，都不自动验收 Task。 |
| 会话授权的私人工作 | [`task_commands`](../../engine/src/services/auth_service/durable_source/task_commands)、[`private_work`](../../engine/src/services/auth_service/durable_source/private_work) | 新鲜 Session + 当前账户/成员关系 → 本人 Task、Artifact 与人工 Review，验证命名空间及写后状态；教师也不能借此读取他人私人工作。 |
| 目录准入与 Draft 换版 | [目录命令](session-catalog-tasks.md)、[换版命令](session-task-revisions.md) | 精确的服务端准入定义元数据；Draft 换版验证旧、新输入并只增加一个任务修订；不运行领域 provider，不生成证据或验收工作。 |

## 跨模块链路与尚未连接的边界

| 链路 | 当前状态 |
|---|---|
| 浏览器 → 旧 Session → 教学/脚本/事件操作 | 已连接；权限和所有权由 Engine 执行，不依赖侧栏隐藏。 |
| 源码 → 本地 Runner → 观测 → 教学包 → 尝试 → 教师反馈 | 已连接教学链路；规则评估结果不是通用持久化 Review。 |
| 浏览器远程检查 → 所有者作业 → 签名 Agent → 编译报告 | 已连接可选编译链路；Agent 失败不授予自动回退到本地执行的权限。 |
| 本地 TaskDraft → 选中 payload → 受控编辑器 → 本地导出 | 完全在浏览器内连接；登录/未读请求属于外层界面，不是保存 payload。 |
| 持久化 Session → 本人 Artifact 修订 → 本人 Task 精确输入 → Draft 换版 | 后端准备链已实现；尚无公共 API 或浏览器适配器。 |
| 持久化 Session → 精确的本人 Artifact 目标/证据 → 人工 Review 历史 | 后端准备链已实现；尚无跨用户审阅权限与验收。 |
| 旧账户/脚本/尝试 → 持久化身份/Artifact/Task/Review | 迁移、旧写入方隔离与切换待实现；没有隐式双写或数据收编。 |
| 浏览器保存任务 → 发布 Artifact → 创建/换版 Task → 冲突恢复 | 公共认证/CSRF 契约、payload 元数据及冲突/结果核对待实现；本地导出不是服务端持久化。 |
| 通用 Task → 执行计划 → 类型化证据 → 规则 Review → 验收 | 组合流程待实现；目录元数据与进程内规则评估不能单独建立此链路。 |
| 教师管理的隔离学生运行环境、托管 VM 生命周期、外部 LSP 进程 | 待实现；现有 Linux 执行和本地语言 worker 不构成这些隔离边界。 |

尤其需要区分：将 Task 变为 `Cancelled` 不会取消 Runner 作业，任务输入换版不会改写旧 Artifact 或
Review。这些影响必须通过显式适配器连接，并单独定义权限与失败语义。

## 地图维护方式

新增页面、组装字段、公共路由、provider、持久化存储或跨模块适配器时，同步更新本文及
[英文版](../en/platform-network.md)。标明连接是公开、内部还是仅本地，并说明失败边界。通过
[当前测试指南](testing-guide.md) 查找测试，不以旧通过数量代替验证。

[0.3.8 功能网络](functional-network.md) 及其机器清单保留历史源码快照；
[0.3.7 测试网络](testing-network.md) 是带独立修复证据的历史报告。不要刷新它们的数量与源码指纹，
让其看起来覆盖当前源码版本。分日期验收证据见[项目进度](project-status.md)，设计约束见
[整体架构](architecture.md)。
