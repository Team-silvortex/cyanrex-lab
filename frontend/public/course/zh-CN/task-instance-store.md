# ADR-014 持久任务实例

状态：**C2-B 已收录于源码版本 0.4.8，C2-J 存储改动收录于 0.4.9**。决定日期：**2026-10-03**。

通用平台现在可以持久保存手工任务或目录定义的任务，不依赖教学角色、编译器或 Run。任务固定保存
定义与评估策略元数据，状态变更与事件记录在同一事务提交。这是显式启用的 PostgreSQL 准备层，
不是在线工作流、授权服务，也不替换现有教学存储。

## 契约与职责

| 位置 | 职责 |
|---|---|
| `models/collaboration/work.rs` 与 `references.rs` | 带作用域的 Task 引用、快照和生命周期 |
| `services/task_store/draft.rs` | 校验手工输入或保存可信目录中的准确版本定义 |
| `services/task_store/commands.rs` | 创建及带修订条件的状态变更 |
| `services/task_store/replace.rs` | 仅 crate 内部调用、带修订条件的 Draft 输入换版 |
| `services/task_store/records.rs` | 有界记录与任务、事件链头的一致性 |
| `services/task_store/schema.rs` | 显式空命名空间安装及作用域检查 |
| `migrations/0011_collaboration_tasks.sql` | 元数据、任务与事务事件表 |

`TaskStore::new(pool, scope)` 不执行 I/O，也不读取环境配置。调用方必须显式提供专用连接池及
authority/Workspace 引用；读取不分配身份。存储层既不创建，也不证明 Principal 或 Workspace 存在。

`TaskDraft::manual` 不要求领域包或 Run。`TaskDraft::from_catalog` 按完整的包/定义引用查询可信
`TaskCatalog`，保存定义、证据 Schema 与评估策略版本。草稿字段私有且不支持反序列化，适配器不能
把任意客户端元数据当作已注册定义。缺失版本直接失败，不回落到最新版本。重新读取已有任务不要求
原提供器仍已安装，但快照不保存评估器的可执行代码。

任务包含一个所有者、调用方指定的稳定 ID、标题、可选定义元数据、输入 Artifact 引用和正的 JSON
安全修订号。本切片不支持重新分配、直接编辑内容、依赖或定义升级；每次状态变更都保留原标题、定义和
输入。标题非空、无控制字符，最多 256 个 UTF-8 字节；输入最多 32 个，同一制品/修订坐标不能重复，
即使附带不同摘要也不接受。

[C2-J](session-task-revisions.md)通过仅限 crate 内部的存储方法与当前 Session 适配器，
允许显式替换 Draft 输入。它不编辑 Artifact 字节或其他任务字段，使用绑定修订的独立事件，
不冒充状态转换。

输入引用必须属于选定 Workspace。格式和作用域检查**不证明**内容存在、摘要正确或调用方有权读取；
[C2-C](artifact-revision-store.md)现已提供经过校验的内容读取，但该草稿构造器不调用制品服务，也不
组合授权，单靠该构造器不能证明执行或审阅证据已验证。独立的
[C2-G Session 适配器](session-task-inputs.md)现已在 Task 事务内检查当前私人输入权限与确切内容，
[C2-I](session-catalog-tasks.md)在同一事务中准入服务端配置的准确定义及 0–32 个输入，读取/状态转换
比较完整定义，空输入也核对 Artifact 元数据。这些适配器不执行提供器策略或验证类型化 Evidence；
直接构造器仍不认证内容或 Review 证据。

## 独立于执行和审阅的生命周期

创建时状态为 `draft`、修订号为 1。每次成功转换只增加一个修订。

| 当前状态 | 允许转入 |
|---|---|
| `draft` | `ready`、`cancelled` |
| `ready` | `in_progress`、`blocked`、`cancelled` |
| `in_progress` | `blocked`、`in_review`、`cancelled` |
| `blocked` | `ready`、`in_progress`、`cancelled` |
| `in_review` | `in_progress`、`cancelled` |
| `cancelled` | 无 |

重复状态、过时修订及终态后的变更都会拒绝；修订溢出不会写入。`in_review` 仅记录工作意图，不证明
已有 Review 或审阅结论。在具备经过认证、绑定准确修订的 Review 与验收策略之前，不开放
`accepted`/`completed`。自动 `TaskAssessment::Passed` 或 Run 成功不能批准任务。取消 Task 只结束
工作项，不会停止 Run、解除挂载或释放运行资源。

## 授权和安装边界

Rust 方法只允许已完成认证、授权的可信适配器传入 Principal 引用。按所有者过滤**不是身份认证**：
允许不可信调用方自报他人所有者引用不在支持范围内。当前没有组合 C1 Session、成员关系、退役或撤权
检查。后续公共命令须让授权与变更共享事务边界，不能先检查权限、待权限可能变化后再调用存储层。

`install_empty_namespace` 是唯一安装入口，只接受 `search_path` 中唯一解析到的专用 Schema，
拒绝 `public` 和系统 Schema。事务级建议锁串行化任务安装器；已有关系、函数或类型均拒绝，不接管
重复或部分安装。DDL 和 authority/Workspace 元数据一起提交。没有启动钩子、旧教学记录迁移、CLI
入口或自动内存/文件降级。

源码版本 0.4.9 核对 Task 存储 Schema 版本 2 与选定作用域，拒绝 0.4.8 中 C2-B 使用的 Schema 1。
只支持全新空命名空间安装，不自动迁移；共享 `CoreSchemaVersion` 仍为 1。
0.5.0 收录的独立 [C2-L 内容存储](task-content-store.md)使用 Schema 3 与必填清单；本 Schema 2 句柄及其
Session 适配器会拒绝该命名空间，不会修改它。
三张表必须是永久普通表，不能使用行级安全策略、分区、
继承或被同名临时表遮蔽。关系锁固定检查对象；写入先锁元数据，再锁任务，事件写入后再次核对作用域。
这些是针对性的兼容与损坏检查，不是完整的列/约束/函数指纹，也不能防御拥有数据库特权的操作者。

## 事务与失败语义

创建、变更状态或 Draft 输入换版时，任务与一个唯一绑定修订的 outbox 条目在同一事务写入。事件包含生成的稳定事件 ID、
所有者 actor、完整结果快照，以及 `cyanrex.task.created`、`cyanrex.task.status_changed` 或
0.4.9 新增的 `cyanrex.task.inputs_replaced` 类型。Draft 修订 1 对应创建，更高 Draft 修订对应换版，
因为状态图不允许转回 Draft。
写入行数和读回结果必须吻合，确认提交后才返回成功；服务不会修改已有事件。事件插入失败或被抑制，
都不能发布成功的任务变更。

写入按“元数据 → 任务”的顺序加锁，再以独立语句查询最新事件，保证等待行锁的写入者能同时观察并发
胜出者的修订和事件。读取使用同一个只读可重复读快照；任务身份、所有者、修订与快照必须和最新事件
吻合，事件类型与 actor 也要正确。这是**链头一致性**，不是全历史对账、防协同篡改证明，也不从读取
快照推导当前授权。

任务与事件快照严格解码，每个 JSON 上限 32 KiB；即使数据库检查约束被移除，读取仍限制取回的内容
大小。操作总时限 10 秒，语句时限 5 秒，锁等待时限 2 秒；错误不暴露原始 SQL 或连接信息，也不会
降级到易失存储后返回成功。

重复创建返回冲突，不是幂等成功重放；过时状态命令需要按当前修订解决冲突。提交附近发生超时、取消
或连接失败，不证明已回滚，不能盲目重试。当前没有命令回执、对账 API、事件分发器、消费游标、全局
提交顺序或恰好一次投递保证；该 outbox 不接入旧遥测事件中心。

## 验证与剩余工作

原 C2-B 测试包含 4 条默认用例，验证草稿构造、状态图、存储不可用及禁止在线/领域组合；
15 条显式 PostgreSQL 用例覆盖
重新打开、准确版本、作用域、所有者过滤、并发胜出者、过时修订、安装拒绝、事件失败/抑制、篡改、延迟
提交失败、提交前取消、事件缺失/RLS、临时表遮蔽、写后作用域变化、一致读取、元数据锁等待及修订溢出。

`scripts/test-task-storage.sh` 要求显式的一次性数据库，并在确认用例存在后，按准确名称执行全部
15 条数据库用例。CI 调用同一脚本，另有回归测试核对显式清单与 Rust 用例一致。实际执行的检查及
限制见[项目状态](project-status.md)。不能在已部署数据库运行这些故障注入测试。

[C2-C](artifact-revision-store.md)已补有界的不可变 Artifact 存储与固定 Task 引用夹具。
[C2-D](review-record-store.md)增加绑定修订的 Review 判断与历史，但尚无经过认证的来源或验收。
[C2-E](session-task-commands.md)现已组合当前 Session、审计成员和私人纯手工 Task 命令；此处直接
存储入口仍要求可信归属，没有旧写入栅栏。[C2-F](session-artifact-commands.md)增加 Session
授权的私人 Artifact 访问；[C2-G](session-task-inputs.md)继续组合准确私人输入与 Task
创建、读取及状态转换。[C2-H](session-review-commands.md)增加带准确所属证据的私人
人工 Review 命令；[C2-I](session-catalog-tasks.md)只增加目录元数据准入，不保留提供器。
策略执行、跨用户审阅权与验收仍待完成。在在线切换前，验证
旧 attempt 离线映射、所有权和教学可见性不扩大。[本地任务 payload 界面](editor.md)已经存在，
但公共任务编辑 API 和浏览器保存/冲突处理尚未接通。这些能力、可靠 Run 编排及非教学多人协作
仍是独立里程碑；C2、C-M2 与 C-M5 尚未整体验收完成。
