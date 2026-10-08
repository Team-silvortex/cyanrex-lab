# 会话授权草稿发布

状态：**C2-P 内部准备层，已收录于 0.5.3**。决策日期：**2026-10-07**。
本流程仅支持 Unix，只创建新任务：通过已有当前 Session Artifact 命令发布 [C2-O 计划](task-draft-publication.md)
中的文本项，再经 [C2-M](session-task-content.md) 创建私人手工 Draft。调用者每次显式推进一步。
它不是整份草稿事务、公共上传路由、浏览器保存适配器或在线认证切换。

## 准备与归属

`DurableAuthSource::prepare_task_draft_publication(token, workspace, plan)` 返回不透明的
`SessionTaskDraftPublication`。准备不访问数据库或文件：固定来源句柄与可信
`SessionTaskContentWorkspace`，校验规范 token 外形，预分配完整的一组随机 UUIDv4 Task/Artifact/
修订 ID 及预期内容摘要。ID 不从本地编辑身份、文件名或正文摘要派生，标题和正文限制沿用 C2-O。

尝试只保存私有、带用途隔离的 Session 指纹，不保存原始 token。调用者在
`advance(&mut self, token)` 时再次提供 token；不同 token 在标记派发前被拒绝，Ready 状态不变。
指纹匹配不是认证：每个实际写入仍独立通过已有当前 Session 命令及其事务检查。步骤之间不能
更换来源、工作区或计划，也不接受调用者指定 owner。

尝试类型没有 `Clone`、`Deserialize` 或 `Debug` 实现。只读访问器提供 `state`、`task_ref`、
`allocated_artifacts`、`confirmed_artifacts` 和 `confirmed_task`，不允许注入确认或从快照恢复尝试。
访问器暴露的是历史元数据，不是当前授权；元数据也应保密，草稿正文与 token 不得进入日志。
认证材料应留在任务记录与导出内容之外。

## 每次推进一步

| 状态或结果 | 含义 |
|---|---|
| `Ready` | 可用原 Session token 显式推进下一步 |
| `Unconfirmed(Artifact { index, reference })` | 已在首次等待前标记这一精确发布，尚无确认 |
| `ArtifactConfirmed { index }` | 原 Artifact 命令返回符合预期的确认元数据，已同步追加到记录，尝试回到 Ready |
| `Unconfirmed(Task { reference })` | 已在首次等待前标记最终创建操作，尚无 Task 确认 |
| `TaskConfirmed` 与 `Complete` | C2-M 返回符合预期的确认 Task/清单元数据并保存在尝试中，拒绝继续推进 |

每次推进至多派发一个已有授权写入。各项 Artifact 发布仍为独立事务；最后一个 Artifact 步骤
不会同时创建 Task。即使正文相同，也保留分别分配的引用。首版只创建新内容和新 Task，不复用
调用者给定的修订、不修改 Artifact，也不替换已有 Task。

已确认修订元数据与尝试保存的原始正文交给 C2-O 纯绑定检查；这是自洽检查，不是额外读取文件。
C2-M 在自己的事务中重新读取真实 blob，独立复核精确引用、owner、命名空间及当前 Session，
再确认 Task。空计划直接构造空清单，由 C2-M 派生真实 owner，不虚构 Principal 或占位 Artifact。

## 取消与部分结果

调用者必须把尝试保存在可能被取消的 future 之外。每一步先记为 `Unconfirmed`，再等待已有命令；
成功返回后不再等待便记录确认。尚未 poll 的 advance 被丢弃不会派发工作；派发后丢弃 future，
或收到派发后的错误，会保留已确认 Artifact 前缀与精确未知步骤，并禁止继续推进。`Stopped`
不表示已回滚。原错误类型保留，但即使冲突也不能证明先前成功或允许重试。

流程不会删除已发布内容、撤销先前事务、重试未知步骤或修复尝试。失败发布可能留下文件；最终
Task 创建失败不撤销已确认 Artifact 发布。原 Artifact/C2-M 命令期限与锁限制仍逐命令生效，
不是整条流程期限，也不限制进程总内存、并发尝试或持久存量。

返回快照不符预期时为 `InvalidConfirmation`，不计入确认进度；尝试保留未知步骤并禁止推进。
这一检查不会撤销可能已经提交的命令。

进度只在内存中。销毁整个尝试、终止持有尝试的任务或退出进程都会丢失这份记录。只读快照不是
持久回执、幂等键、恢复协议或恰好一次创建证明；另建尝试可能生成逻辑重复工作。本片不提供
自动结果对账。

0.5.3 收录的 [C2-Q](session-task-draft-observation.md)使用同一尝试和原 Session，显式只读观察未知目标。
当前匹配、不同或不可见都不追加确认、不解锁状态，也不允许重试。下方原 C2-P 证据不覆盖这一
后续读取操作。

0.5.3 收录的 [C2-R](session-task-draft-checkpoint.md)导出不含正文或认证材料的有界检查点元数据，
不保存数据，也不反序列化原尝试。解析后的进度只是调用者声明，不是确认回执；旧快照不允许
继续推进或重试。

0.5.4 收录的 [C2-T](session-task-draft-intent.md)可在原 Session 下，从真实 Ready/零确认尝试另行
登记不可变意图元数据。`advance` 保持不变，不要求或写入该日志；仅登记不强制派发前持久化。

## 剩余边界与验证

[C2-N 路由](task-content-http.md)仍未挂载，不调用本流程，也不会获得 8 MiB 上传额度。
没有新增 Session 签发、CLI、AppState 接线、Schema 迁移、部署授权、公共 OpenAPI 或 SDK。
浏览器保存、认证安装与结果恢复仍须独立实现；教学运行时和本地草稿行为不变。

2026-10-07 的 14 项默认准备/确认/取消单测、12 条准确枚举的隔离 PostgreSQL 用例及三项公共守卫
通过。SQL 覆盖空/多项成功、token 不匹配、未 poll 推进、部分发布、取消、最终提交失败、
目标冲突、撤权/过期与正文变化。这是限定范围的运行，不是浏览器或部署验收。分日期结果记录在
[项目状态](project-status.md)，精确选择入口见[测试指南](testing-guide.md)。
[功能张量](capability-maturity.md)为这一内部组合单列坐标，不提升缺失的浏览器保存连接评分。
