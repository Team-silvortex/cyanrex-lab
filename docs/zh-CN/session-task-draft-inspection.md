# 检查点目标的显式检查

状态：**C2-S 内部准备层，已收录于 0.5.3**。决策日期：**2026-10-07**。
这项仅支持 Unix 的操作将 [C2-R 检查点](session-task-draft-checkpoint.md)报告的一个未知目标与当前有权
读取的存储比较。不需要仍存的发布尝试，但调用者必须另行提供受信任工作区和当前有效 Session。
比较不认证检查点历史、不恢复尝试，也不确认写入。

## 显式输入与当前权限

`source.inspect_task_draft_checkpoint(token, &trusted_workspace, &checkpoint)` 是解析之后的
独立操作。检查点必须报告 `unconfirmed`，其范围须在读取前与显式工作区及来源权威一致。
报告计数按 C2-R 规则只选择一个 Artifact 引用或 Task；检查点不能选择数据库、命名空间或路径。

返回类型为 `Result<SessionDraftCheckpointObservation, SessionDraftCheckpointInspectionError>`。
准入错误为 `NoUnconfirmedStep` 或 `ScopeMismatch`；读取失败保留已有
`Artifact(SessionArtifactError)` 或 `Task(SessionTaskError)` 变体。

已有 Session 读取独立复核给定 token、当前账号及经审计成员、所有者访问权、命名空间和最终
会话有效性。检查点字段不是凭据或权限，其中没有原 Session 指纹、owner 或来源代次，故不能
证明当前调用者就是原发布者。另一个有效 Session 按其自身当前权限判断，不会被收编为原会话。

这不同于仍要求原不透明尝试存活、原 Session 指纹一致的 [C2-Q](session-task-draft-observation.md)。
C2-S 不复活该尝试，也不绕过其停止状态。C2-R 解析和 `reported_unconfirmed_step()` 仍是纯操作，
不会触发本次检查。

## 一次读取与元数据比较

操作仅调用一个已有 C2-F `read_session_artifact` 或 C2-M `get_session_content_task`，
不新增直接 SQL 或与事务分离的授权预检。返回所选 `step` 与
`result: SessionDraftCheckpointObservedOutcome`，不含内容字节：

| 结果 | 本次读取的含义 |
|---|---|
| `MatchesCheckpointMetadata` | 授权目标与检查点元数据及预期创建形式一致 |
| `DiffersFromCheckpointMetadata` | 可读目标与预期不同，后续合法 Task 编辑也可能导致此结果 |
| `NotVisible` | 已有授权读取没有返回可见目标 |

已有读取错误保留原类型，包括正文损坏或摘要不匹配，不能转为 `NotVisible`。不可见不证明
已回滚、没有进行中的写者或可以安全重试；不同调用有不同观察时点。其他写者也可能在这些 ID
下产生匹配数据。

Artifact 检查读取分配的精确修订，可能是 head 前进之后的历史修订。匹配要求精确引用、声明长度、
检查点标题、序号 1、无 parent、种类 `cyanrex.task-text` 及媒体类型 `text/plain`。文本验证检查
实际摘要、长度、UTF-8、允许的控制字符及 256 KiB 文本上限。filename 和 language 是清单标签，
不是 Artifact 存储事实，不参与单个 Artifact 的比较。
通用读取器可读的二进制或含禁止控制字符的内容，在文本比较中返回
`DiffersFromCheckpointMetadata`；读取器的损坏错误仍保留为类型化失败。

Task 检查在一次已有事务中读取当前 Task 快照和引用内容，比较精确 Task 引用、完整有序清单、
手工 Draft 修订 1，以及各项返回内容的创建元数据、长度、合法文本和当前 Task 所有者一致性。
所有者来自授权读取，而非检查点；不另行审计报告的已确认前缀。Task 不存在时，
已有提前返回路径检查来源和 Task 边界，但跳过 Artifact 命名空间；已存在的空 Task 仍走
存在分支及其命名空间检查。

这里没有原始草稿正文，摘要/长度及当前元数据相符不是与原计划逐字节比较，也不证明写者身份
或原存储代次。命名空间固定只保护各自读取事务，不防御调用前由特权者替换的同名副本。

## 限制与不变状态

检查不写业务数据，但已有读取仍取锁并提交，不是 SQL `READ ONLY` 模式；原命令期限和锁限制
保持不变。通用 Artifact 读取器可能先按其 1 MiB 上限取得正文，再套用更严格的文本契约：
256 KiB 不是整个检查操作的内存上限，也不是进程级并发配额。

匹配、不同、不可见、错误或取消都不修改检查点、不确认历史、不收编目标、不标记 Task 完成、
不复活尝试、不重试写入或删除内容。这是单目标读取，不是整个工作区审计或持久结果协议。
检查点与结果元数据仍可能敏感，应保持私密，凭据与正文不得进入日志。

没有检查点保存/加载、刷盘、意图日志、迁移、公共路由、浏览器连接、Session 签发或在线来源
安装。调用者可另行保留元数据，但 C2-S 不保证保存，也不验证重启后的原调用者身份。
持久意图、恢复身份和重试策略仍须各自定义契约。

## 验证边界

默认测试与显式隔离 PostgreSQL 用例分别覆盖状态/范围准入、当前权限、精确 Artifact/Task 比较、
类型化失败、缺席与空 Task 的区别，以及读取或取消后业务数据不变。SQL 精确入口为
`scripts/test-session-task-draft-inspection.sh`，选择 `session_task_draft_inspection_tdd`。
实际分日期结果记录于[项目状态](project-status.md)；此前 C2-P/Q/R 证据保留各自范围，不能作为
本次独立入口的执行证据。
