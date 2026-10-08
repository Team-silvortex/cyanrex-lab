# 会话授权日志步骤观察

状态：**C2-W 内部准备层，已收录于 0.5.4**。决策日期：**2026-10-08**。
源码版本：**0.5.4**。

C2-W 以当前 Session 权限观察一条已记录日志步骤的资源，在同一个来源层事务中组合 Schema 2
意图、完整步骤前缀及一次授权 Artifact 或 Task 内容读取。不含正文的结果分别表达记录状态与
当前元数据是否相符；两者都不复活尝试、不证明发布历史，也不授予重试权限。

## 显式选择与准入

`source.observe_session_draft_journal_step(token, &SessionDraftIntentWorkspace, task_id, ordinal)`
返回 `Result<Option<SessionDraftJournalStepObservation>, SessionDraftJournalObservationError>`。
调用者提供可信路由配置与当前 Session；新签发 Session 仍须匹配保留[意图](session-task-draft-intent.md)
的精确所有者/账号代次。存储名称不能重定向读取，也不能让新 Session 接管原存活包装器。

序号超过 32 时以 `InvalidOrdinal` 拒绝，不访问数据库或资源。已归属的意图中，序号超出已记录
前缀时返回 `NoRecordedStep`，不读取资源；即使它是将来可能执行的计划步骤也一样。
意图缺失或被所有者/账号过滤时返回 `None`，不同于已记录步骤的资源 `NotVisible`。
`None` 和 `NoRecordedStep` 仍须完成末尾当前权限检查并确认事务提交。
已归属但未记录的序号也先复验原日志快照，再返回结果。
仅接受 Schema 2，不增加 DDL、不升级 Schema 1，也不附带安装。

## 同一事务保留首次日志身份

来源层操作沿用十秒命令期限，核验当前 Session、精确所有者/账号及可信路由后，捕获规范的
不可变意图、完整合法前缀和首次观察到的日志关系身份。合法前缀是连续 Committed 记录，末尾
至多一条 Unknown；选择较早序号不能隐藏损坏的后缀。

目标资源经借用事务的 Artifact 读取器或原 C2-M Task 内容 helper 在同一事务中读取，不另调
公共读取方法，也不先做脱离事务的授权。读取后，使用**首次捕获的关系身份**复核**原完整意图、
步骤序列及 nonce**，然后再次检查新鲜权限、提交，才返回结果。资源读取期间的等价表替换不能
被当成新基线接受。

本操作不写业务 DML，不修改 Ready/零进度检查点、步骤标记、资源或存活尝试，但仍持锁、读取
文件并 COMMIT；这不是 PostgreSQL `READ ONLY` 模式，取消或期限到达也不是回滚或重试证据。

## 记录状态与观察结果

不透明结果仅提供 `step()`、`recorded_status()` 和 `result()`。
记录状态为 `SessionDraftRecordedStepStatus::{Unknown, Committed}`，不暴露 nonce 或资源正文。
独立的 `SessionDraftJournalObservedOutcome` 包括：

| 结果 | 本次读取的含义 |
|---|---|
| `MatchesIntentMetadata` | 已授权目标符合保留意图的元数据及预期创建形状 |
| `DiffersFromIntentMetadata` | 可读目标不符合，包括后续合法 Task 编辑 |
| `NotVisible` | 授权资源读取器未找到该已记录步骤的可见目标 |

Artifact 读取已分配的**精确修订**，新 head 出现后仍可读取原历史修订，不会用 head 替代。
复用 [C2-S 元数据检查](session-task-draft-inspection.md)，核验引用、声明长度、标题、序号 1、
无 parent、`cyanrex.task-text` kind、`text/plain` media 和有效文本。文件名/语言是清单标签，
不是独立 Artifact 存储事实。

Task 读取当前快照，经 C2-M 验证其全部引用内容，再比较完整有序清单及预期创建形状。
后续合法编辑可能产生差异，即使原日志仍为 Committed。Task 缺失仍跳过 Artifact 命名空间；
存在的空 Task 则遵守已存在路径的检查。损坏资源或 blob/digest 错误保留类型，不变成
`NotVisible` 或伪造的元数据差异。

不返回正文**不等于不访问文件**：Task 可能检查至多 32 项当前文本，合计内容限额为 8 MiB。
通用 Artifact 读取器可先物化最多 1 MiB，再进行更严格的 256 KiB 文本比较；这些不是进程总内存
或并发配额。意图和结果元数据仍应保密，日志中不要记录正文或凭据。

## 观察不是恢复协议

Unknown 加元数据相符不等于资源提交已确认；Committed 加缺失或差异也不授权修复、重建或删除。
`None`、`NoRecordedStep` 和 `NotVisible` 不证明全局缺席、回滚或以后不会执行工作；每次调用
有自己的读取时刻。原始草稿字节不可用，元数据相符也不证明原存储代次、先前写者、管理员完整性、
备份防回放或宿主崩溃恢复。

[C2-V](session-task-draft-journal-inspection.md)仍独立只读日志，不访问资源或文件。两个 API
共享私有捕获/校验机制，C2-W 不调用 C2-V 公共方法。C2-Q/S 合同不变，也不恢复 C2-U 已停止或
丢失的包装器。不提供确认回执、nonce 输出、继续执行、收编、重试、清理、尝试复活、公共
HTTP/SDK、浏览器保存、在线认证切换或部署变更。

## 验证边界

来源适配器位于 [`journal_observation.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/journal_observation.rs)，
私有捕获共用于 [`draft_intent_journal/inspection.rs`](../../engine/src/services/draft_intent_journal/inspection.rs)，
借用事务的 Task 读取位于 [`task_commands/content.rs`](../../engine/src/services/auth_service/durable_source/task_commands/content.rs)。
2026-10-08，仅 Unix 的 `session_task_draft_journal_observation_tdd` 通过两项默认测试、十二条
精确隔离 SQL 用例及三项新增公共守卫，runner 为 `scripts/test-session-task-draft-journal-observation.sh`。
SQL 并发覆盖外部日志写者阻塞，不是本读取事务内篡改；首次快照复用另受源码守卫约束。
此前检查/派发结果不作为继承证据。[项目状态](project-status.md)保留行为 Red 和仅夹具 kind
修正，[测试指南](testing-guide.md)说明选择范围。
