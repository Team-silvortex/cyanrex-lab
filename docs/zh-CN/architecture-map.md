# 当前架构与变更边界

源码 **0.6.3** 已收录独立[绑定登录](platform-login-connection.md)，经原存储为已有账号受限签发
Session；独立[登录 HTTP](platform-login-http.md)投影 Cookie，[共享 Session 路由](platform-session-router.md)组合同一绑定；显式[回环宿主](platform-loopback-host.md)接通[私人任务索引](platform-task-index-connection.md)、[只读重开](platform-task-content-connection.md)、[手动空 Draft 创建](platform-task-create-connection.md)及[预期修订清单换版](platform-task-replace-connection.md)，可选[原生 TLS](platform-loopback-tls-host.md)和[服务状态通知](platform-host-notifications.md)，不写状态、不接局域网或浏览器正文／SDK 保存。

**2026-10-16 收录于 0.6.2** 的[绑定会话适配](platform-session-connection.md)将
`platform_host/session_connection.rs` 接到共用的私有 Session 事务助手及显式 HTTP 后端。
保留存储和新鲜 Source 校验共用每条命令的事务，不增加连接池回退、签发或正常应用组合。

另行的[格式 3 诊断](platform-marker-expectation.md)在行读取前核完整目录／五标记声明，
**0.6.3** 收录的独立[声明式存储打开](platform-storage-open.md)在核声明及确认关闭后返回保留绑定，不启动宿主或迁移存储；早期实施阶段标签保留原范围与证据。

源码 **0.5.9** 已收录，API **0.3.0**，梳理日期 **2026-10-13**。原整理属于0.5.8标签后的
跟进，源码收录不改变存储Schema或服务挂载。

Cyanrex 有教学应用、本地 payload 编辑器及显式组合的后端，源码 **0.6.4** 收录
[原绑定文本发布及 HTTP](platform-text-artifact-connection.md)，明文／TLS CLI 显式启用；旧构造器保持七路由／六通道且不发布，新组合为八路由／七通道；代码和 Run 都可选。
默认关闭的 Session／Task 壳支持读取私人 Task 及仅保存手动 Draft 显示信息；正文／正常应用／SDK 保存和通用任务执行仍未接通，本机浏览器 TLS 尚未验收。模块分组描述职责，不是进程隔离；
现用 Engine 仍同时承担控制与 Linux 特权执行。

当前职责先读本图，实施决策见[详细架构](architecture.md)，功能边见[平台网络](platform-network.md)，
分日期评分见[成熟度](capability-maturity.md)。[项目状态](project-status.md)与[测试指南](testing-guide.md)
分别保留进度和证据范围；这里不推导总完成度。

<a id="runtime-wiring"></a>

## 运行接线

0.5.9 收录的[本地文本文件导入](local-text-file-import.md)只在 UI 边界增加经确认的整批准备，
保留核心 Task 契约不依赖编辑器的方向，不增加服务端链路。

正常Engine组合入口只有 `engine/src/main.rs`：`engine/src/state.rs` 构建AppState，
`engine/src/application.rs` 组合路由，公共／私有／教师守卫仍在 `engine/src/routes/`。
在索引中声明或导出准备层模块，不会把它加入启动序列。

| 路径 | 当前实际连接 | 独立边界 |
|---|---|---|
| 教学应用 | 浏览器 → 原AuthService／路由守卫 → 教学、事件、部署与Runner服务 | 一次实验运行不自动创建通用Task或Artifact |
| 显式协作后端 | 可信宿主 → 原绑定当前 Session 命令 → 私人资源存储 | 不构造教学 AppState；回环 Session／Task／文本能力显式组合 |
| 本地payload编辑 | `/tasks/new`及兼容`/editor` → TaskDraftWorkspace 组合 useTaskDraftController → 受控文本编辑器 → 本地JSON | 本地ID与语言提示不是服务端引用或执行权限 |
| 显式文件交付 | 当前准确文件grant → Source清单／分块核验 → 所给router → 受控客户端 → 所选保存 | 已有真实Source／HTTPS联验，正常应用／UI／SDK仍未接通 |

教师管理现用教学与部署，单人模式同样如此；这不转化为读取他人私人Task、Artifact、Review
或获接收授权字节的通用权限。在线AuthService与 `engine/src/services/auth_service/durable_source/` 的 `DurableAuthSource`／原绑定助手
是不同组合路径，不是可以直接互换的两种登录实现。

## 职责与依赖方向

| 职责 | 源码入口 | 依赖规则 |
|---|---|---|
| 共享契约 | `engine/src/models/collaboration/` | 准确身份、引用、Task／Artifact／Review／文件值；解码只核结构，不授权 |
| 定义与纯内容规划 | `engine/src/services/task_catalog/definitions.rs`、`engine/src/services/task_content.rs`、`engine/src/services/task_draft_import/` | TaskDefinitionCatalog是冻结元数据，不要求评估器、数据库、发现或执行 |
| 强类型领域评估 | `engine/src/services/task_catalog.rs`、`engine/src/domain_packs/ebpf_teaching/` | TaskCatalog<P>持有提供器证据及评估，内置教学包不进入中立契约 |
| 当前会话命令 | `engine/src/services/auth_service/durable_source/`、`engine/src/services/collaboration_identity_store/` | 派生准确当前执行者、审计成员及原始pin；资源检查和最终权限留在Source事务 |
| 资源持久化 | `engine/src/services/task_store/`、`engine/src/services/artifact_store/`、`engine/src/services/review_store/` | 可信直接存储不认证给定所有者，Session适配借用其事务机制 |
| HTTP投影 | `engine/src/platform_http/` | 核请求并投影显式命令，仅独立登录适配器交付确认后的 Session Cookie；不重复权限、不安装 Schema 或监听 |
| 本地宿主交互 | `frontend/src/features/tasks/useTaskDraftController.ts`、`frontend/src/features/editor/`、`sdk-js/src/agents/` | 任务本地控制器拥有payload／生命周期，视图渲染／选文件，编辑器拥有显示／工具；SDK仍受原Engine权限与宿主批准约束 |

中立契约与冻结定义不依赖教学提供器。领域证据和普通Task payload不能互换：代码、笔记和配置
都可以只是内容，不要求评估或Run。Review记录、规则结果和Task状态变化是独立操作，不自动互转。

`prepared_resource_sql`共用私人SQL设置与表检查，不决定权限；`legacy_workspace`只做离线权限
预览，不是在线认证或迁移。较宽的durable_source目录目前承载账号／Session来源、事务授权和
显式资源命令三类职责；同目录不代表可把命令事务抽成一个提前权限判断。

## 平台准备层职责

**2026-10-16 收录于 0.6.2** 的内部整理保留原公开 API 及输入格式：

- `engine/src/platform_origin.rs` 持有纯规范 Origin 策略，宿主配置及 HTTP 配置分别依赖它、
  各自映射错误；纯宿主配置不再依赖 HTTP 适配器。Origin 接受范围不变。
- `bin/cyanrex-platform/config/` 持有严格输入解码和纯声明构造。私有 `Presence<T>` 统一区分
  字段缺失与显式提供；null、重复键、位置数组仍被拒绝。只有 `storage_identity.rs` 选择
  合法的版本／声明组合，构造 `StorageExpectation::{Current,Catalog,Markers}`，去除两个
  独立可选声明及其不合法的同时存在状态。
- CLI 持有进程／私有文件／根门禁、单 runtime 及穷尽模式分派；runtime 销毁后，即使失败
  也复核根与进程。`platform_host/storage_check.rs` 持有单次连接、数据库总时限，以及
  检查成功或失败后的显式关闭；各 inspector 继续拥有自己的 SQL 和快照边界。

格式 1／2 仍在原来的两个只读 repeatable-read 事务中依次检查目录及当前标记；格式 3 保留
单个声明标记事务，在行读取前核全部五个原始身份。不把这些诊断与 Session 命令抽成同一
事务包装：保留会话连接、业务事务、不确定结果和关闭预算各有不同。没有新增运行宿主、
存储收编、重试或公开配置模式。

## HTTP 配置与准入

同日收录的跟进把 HTTP 纯配置规则归入私有 `engine/src/platform_http/config.rs`：先检查
规范 Origin，再检查所给各容量均为 **1–16**。Task、目录任务、索引、文本 Artifact、Session、
清单及分块状态只委托这段校验，不共用准入对象。文件分发组合仍先检查两项容量，再匹配
缓存工作区，错误优先级不变。

七类状态仍各自拥有 Origin 字符串，并各自创建 `Arc<Semaphore>`；克隆共享本状态的名额，
独立构造的状态互不占用。默认容量仍为 **4**，分发的清单／分块独立状态仍为 **4/4**。
助手不创建信号量、不读取存储，也不规定命令时限；响应正文、字节所有者、Source 工作区及
Session 事务继续保留原释放／授权边界。不增加全局 HTTP 配额、公开配置 API 或运行挂载。

## 服务导航

`engine/src/services/mod.rs`把原 **44** 个声明按职责分组，名称、可见性及Unix条件保持不变。
不增加模块命名空间、公开API或在线服务。

| 索引分组 | 主要职责 |
|---|---|
| 在线实例服务 | 原登录、课堂／配置及宿主管理 |
| 教学与本地执行 | 教学兼容、学习／脚本、编译与内核运行 |
| 签名Runner Agent服务 | 认证计算协议、库存、执行与作业，不是AI身份 |
| 事件遥测 | 在线EventBus及私人持久化／过滤／策略辅助 |
| 准备层身份与SQL支持 | 显式身份注册、离线legacy预览及私人存储机制 |
| 准备层Task与内容 | 纯目录／内容计划，以及显式Task／Artifact／Review／journal存储 |
| 准备层文件分发 | 流／描述符核验、上传／发布／grant账本、receiver与client |

前端 `frontend/src/transport/privateRequest.ts`拥有浏览器携带凭据的等待与取消，功能模块保留
响应解码和准确回执；传输不是服务端事务。`sdk-js/src/generated/`跟随在线OpenAPI，不随每个
Rust导出增长。`sdk-js/src/agents/bridge.ts`只适配明确选出的原工具；模型传输、密钥及逐写批准
留在宿主。AI Agent与签名Runner Agent是不同能力。

## 文件权限与资源所有权

文件交付包含不同边界，不是一种通用“传输权限”：

1. 上传意图、私有暂存字节、journal确认和Published元数据是不同状态；匹配文件或Unknown
   结果不能被采用为成功回执。
2. 发布者权限控制当前发布／分享，独立的准确接收grant控制清单／分块；教师身份或发布者
   撤权不能代替它。
3. Source元数据观察不是可复用字节许可。每块在有界私有核验前后独立核当前权限，不持有
   SQL锁跨文件工作。
4. HTTP适配只投影Source结果。`engine/src/platform_http/errors.rs`持有清单与分块共用的私人
   元数据错误映射，保留私有404和已派发unconfirmed；分块自己的输入／容量错误仍独立。
5. 客户端传输、帧receiver、暂存、拼装与所选保存各持自己的证明。
   `engine/src/services/file_chunk_client/`保留兼容名称，但内部已包括完整受控交付，不只单块。

| 名额 | 实际所有者及释放边界 |
|---|---|
| 元数据HTTP | 实际响应正文完成／drop，不只handler返回 |
| binary HTTP与Source字节 | 经HTTP投影时，最后一个原所有者Bytes引用，包括clone／slice |
| 客户端request／buffer／flow | 对应传输或实际工作；已派发文件作业在等待取消后仍持flow |
| 暂存worker | 实际排队／运行作业，不是观察者寿命 |

直接Source返回的SessionFileChunkRead自己持字节名额；尚无已派发文件作业的元数据等待取消时，
flow可以释放。这些名额不能互换，也不是全局内存／磁盘配额；合并会提前释放或在容量一时自锁。
Cancelled终止被选择的等待／续跑，不杀已派发OS工作；进度关闭或部分百分比不证明保存或回滚。
实际联合执行证据见[真实Source／HTTPS](large-file-source-https.md)。

<a id="change-boundaries"></a>

## 变更边界

小整理可以共享纯辅助、明确归属、去除准确投影重复，但必须保留各state-family预算、完整版本
匹配、错误优先级、Source事务pin、不确定结果和原API名称。本轮只共用文件元数据错误投影。
该“本轮”指 2026-10-13 整理；上文后续的平台准备层整理另行共用严格记录存在性及纯 Origin
策略，不合并运行时寿命。

私有 `engine/src/platform_http/file_selectors.rs` 现共用严格的 512 字节原始查询／十进制解析
机制；适配器各自保留六／七字段、强类型 ID／摘要、可信 scope、范围检查及不同响应所有权。
后续候选仍是 manual／catalogue 草稿共用纯目标准备、旧学习结果到教学证据的强类型转换；
各自准入、fingerprint、评估和事务仍须分开。

新运行组合需要独立提案审阅：安全持久Session签发、无特权宿主边界、显式存储配置，再到公共
UI／SDK契约和真实局域网验收。导出路由或loopback用例通过不能替代这些步骤。恢复、自动重试／
清理、桌面互传和通用调度是另外的变更。

## 文档职责

本图描述当前职责；详细架构保留此前决策，平台网络描述功能边，张量持有分日期评分，发布报告
持有对应验收。执行尾属于历史证据，不随整理重写。`docs/`是源，`frontend/public/course/`只同步，
不单独撰写。边界依据：[任务领域解耦](task-domain-boundary.md)、[AI宿主适配](ai-agent-integration.md)、
[安全](security.md)、[文件分发](large-file-distribution.md)。

## 2026年10月13日验证

- 整理前新增五条结构守卫一条通过、四条因缺少分组／导航及入口描述不完整而失败，不冒称
  生产故障Red；最终聚焦架构与相邻边界检查 **27** 条全部通过。
- 平台HTTP单元 **25** 条通过，包括新增三条投影测试。两个元数据适配各核 **51** 个明确
  预期错误variant；分块的输入／容量／不可用三分支独立核验。状态码、准确脱敏JSON和
  unconfirmed结果保持原样。
- 新鲜原生回归 **9条唯一用例**通过：三条原Source、三条真实Source／HTTPS联验、三条原
  controlled模拟TLS；不同集合不重复相加。
- 完整后端／通用门 **1,100 Rust通过**、**861显式忽略**、**98结果组**、**361通用检查**，
  零失败、零Rust告警。
- 摘要检查保留 **479个无关Engine源码文件**、**1,168个受保护文件**、**106历史状态块**、
  **58旧执行尾**及已发布报告。版本／API、OpenAPI、依赖、张量评分及此前Source／HTTPS
  未提交改动保持不变。
- 本轮独占、仅socket临时PostgreSQL已停止，准确data／socket目录已删除，回收 **40,228 KiB**；
  日志留在 `/tmp/cyanrex-architecture-WRV3tmc3`。

以上是整理／回归结果，不是新前端／SDK构建、已部署局域网／内核验收、安全审计、正常挂载或发布。

## 2026年10月16日验证

- 新增五条架构守卫先因缺少目标结构全部失败，整理后全部通过；不冒称生产故障 Red。
  三条公开构造器 Origin 测试及三种格式／15 次子进程 CLI 矩阵在整理前后均通过；新增纯
  测试还覆盖严格存在性及强类型模式。
- 完整后端／通用门通过 **1,202 项 Rust 测试**、**918 项显式忽略**、**117 个结果组**及
  **431 项通用检查**，零失败、零 Rust 告警。
- 新鲜隔离 PostgreSQL 回归覆盖九个相邻 runner，共 **41 条唯一用例**全部通过；另行复跑
  的 CLI 矩阵不重复计数。所有夹具数据库／角色均已回收，仅 socket 的实例已停止，所属
  data／socket 目录已删除（约 **55 MiB**），日志保留在
  `/tmp/cyanrex-host-architecture-mS4w8RRE`。
- 摘要检查保留 **2,812 个受保护文件**，包括此前未提交工作、CI、依赖、版本／API 基线及
  历史报告。双语源文档与课程副本已同步。

以上验证内部准备层整理，不代表重跑前端／SDK 构建、新宿主组合、已部署局域网验收、发布或推送。
