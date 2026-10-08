# 会话授权草稿日志检查

状态：**C2-V 内部准备层，已收录于 0.5.4**。决策日期：**2026-10-08**。
源码版本：**0.5.4**。

C2-V 另以当前 Session 权限读取保留的意图及已记录步骤前缀。它要求已有的
[C2-U Schema 2 日志](session-task-draft-dispatch.md)，不要求发布包装器仍存。
结果描述日志元数据，不是资源当前内容、复活的尝试或未知写入的重试许可。

## 显式读取与身份边界

`source.inspect_session_draft_journal(token, &SessionDraftIntentWorkspace, task_id)` 返回
`Result<Option<SessionDraftJournalInspection>, SessionDraftIntentError>`。受信任来源/工作区由
调用者显式提供，记录中的命名空间名称不能自行选择其他路由。新签发的 Session 只有在精确所有者
和账号代次均匹配 [C2-T 意图](session-task-draft-intent.md)时才可读取；相同用户名或教师角色不能替代这些条件。

本接口**仅接受 Schema 2**，不安装命名空间、不升级 Schema 1，也不增加 DDL。原有来源层持有的
事务核验当前权限、绑定配置、不可变意图和完整有序步骤前缀；复验前缀与已固定关系身份后，
再次检查新鲜权限，再提交并返回。错误保留类型；已检出的记录损坏、过期权限或不兼容存储不会变成缺失结果。

该读取不写业务数据、不访问文件，但仍持有原有事务锁并提交；“只读”不表示使用 PostgreSQL
`READ ONLY` 事务模式。它不查询 Task 或 Artifact 存储，因此目标命名空间名称仅作为受信任
路由标签核验，不证明原资源或原物理存储仍存在。

## 已记录前缀不等于执行权限

不透明结果只提供以下访问器：

| 访问器 | 含义 |
|---|---|
| `intent()` | 已授权的不可变意图；其检查点仍为 Ready、报告确认数为零 |
| `recorded_committed_step_count()` | 连续 Committed 步骤记录数，若末尾 Task 步骤存在则计入 |
| `unknown_step()` | 可选的下一条已记录 Unknown 目标，不暴露 nonce |
| `all_steps_recorded_committed()` | 所有计划 Artifact 步骤及末尾 Task 步骤都已记录为 Committed |

合法前缀由零条或多条 Committed 记录组成，末尾可有一条 Unknown。缺口、目标乱序、Unknown
之后仍有记录或行数溢出均拒绝，不修复或截断。最多 32 个 Artifact 步骤加一个末尾 Task 步骤。
空 payload 仍须有 Task 记录；零条记录不代表发布完成。

结果不修改 Ready/零进度检查点，不追加确认，也不更新任何存活包装器。元数据仍应保密，
不要随意记录标题、标签和引用。数量及标志只描述本次读到的日志，不是资源复读、派发回执，
也不提供针对管理员、克隆存储或备份回放的原始发布历史证明。

## 缺失与不确定结果

`None` 可能表示记录不存在，也可能表示当前所有者/账号代次无权看到它；不证明未派发工作、
原事务已经回滚，也不授权重新登记、发布或删除。它不证明全局不存在或以后不会派发；后续
检查有自己的读取时刻。读取会等待原协议的锁，超时返回类型化错误，而不是越过写者的锁，
把它尚未提交的记录直接视为缺失。资源结果未知仍不授权重试。

Unknown 记录不是回滚回执。全前缀已记录也不会重建正文、原尝试或封闭派发许可。
日志检查不能继续、收编、重试、清理、修复或复活停止的包装器。事务内身份固定不证明跨重启
原存储代次；宿主崩溃持久性与备份回放仍是独立边界。

[C2-Q](session-task-draft-observation.md)仍要求原尝试/令牌存活；
[C2-S](session-task-draft-inspection.md)另行读取显式检查点目标，本接口均不调用。
C2-R 解析仍是纯数据，原 C2-P 派发仍独立。不增加公共 HTTP/SDK、浏览器保存、在线认证切换或部署变更。

独立的 [C2-W 步骤观察](session-task-draft-journal-observation.md)在同一事务捕获日志后读取
实际资源；它共享私有校验，不调用本公共方法，也不把 C2-V 元数据结果变成执行或恢复许可。

## 验证边界

当前 Session 适配器位于 [`journal_inspection.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_inspection.rs)，
私有日志读取位于 [`draft_intent_journal/inspection.rs`](../../engine/src/services/draft_intent_journal/inspection.rs)。
仅 Unix 的 `session_task_draft_journal_inspection_tdd` 目标与
`scripts/test-session-task-draft-journal-inspection.sh` 将此读取和派发执行分开。

2026-10-08 的两项默认 Rust、十条精确隔离 PostgreSQL 用例及三项公共守卫通过，覆盖已记录
前缀、当前权限、Schema/身份故障、等待与取消，不验证资源、不恢复发布或验收部署恢复。
此前 C2-T/U 运行不作为继承证据。[项目状态](project-status.md)保留初始行为 Red 和夹具修正，
[测试指南](testing-guide.md)列出精确 runner 及选择范围。
