# 草稿发布目标的只读观察

状态：**C2-Q 内部准备层，已收录于 0.5.3**。决策日期：**2026-10-07**。
本 Unix 操作只观察仍保存在内存、处于 `Unconfirmed` 状态的 [C2-P 尝试](session-task-draft-publication.md)
所固定的精确目标，将当前可读数据与计划创建值比较。不确认先前写入、不改变尝试状态、
不恢复发布，也不证明回滚。

## 操作与当前权限

`attempt.observe_unconfirmed(&self, token)` 返回
`Result<SessionDraftPublicationObservation, SessionDraftPublicationError>`。Ready 与 Complete 尝试
返回 `NoUnconfirmedStep`。必须匹配原规范 Session 指纹；不同 token 返回 `SessionChanged`，
不访问数据库，固定来源与工作区也不能替换。指纹匹配不是授权：原 Session 仍须通过已有读取
命令的当前账户、审计成员、命名空间及最终会话检查。过期或撤销会话返回原类型化错误，
重新登录不能接管这个尝试。

每次只按精确未知步骤调用一个已有 `read_session_artifact` 或 `get_session_content_task`。
不新增直接 SQL、授权守卫或业务写入。原事务仍会取锁并提交，不是新增 SQL `READ ONLY` 设置；
逐命令期限仍生效，不构成整条流程期限或观察次数配额。

## 观察结果

报告只含 `step` 与 `result`，不含 payload 正文。`SessionDraftObservedOutcome` 有三种值，
底层读取失败仍返回错误，不能转换为其中一种结果。

| 结果 | 本次观察 | 不能证明 |
|---|---|---|
| `MatchesPlannedCreate` | 当前可读内容符合计划创建目标和预期元数据 | 由本次尝试创建、先前提交已确认或之后仍未改变 |
| `DiffersFromPlannedCreate` | 当前可读有效数据与计划创建不同，后续合法 Task 编辑也可能得到此结果 | 允许覆盖、修复、删除或收编 |
| `NotVisible` | 授权读取在其观察时点没有返回可见目标 | 全局不存在、已回滚、没有进行中的写入或可以安全重试 |

其他写者也可能在分配的身份下创建相符数据；匹配只是当前观察，不证明发布来源，也不升级 C2-P
历史确认记录。元数据报告本身也可能敏感，应保持私密，正文和 token 不得进入日志。

Artifact 观察按已分配的精确修订读取；即使 Artifact head 已前进，历史修订仍可能匹配，不要求
它仍为当前 head。Task 观察则读取当前 Task 快照及正文。

Task 存在时，单次既有 C2-M 读取事务检查当前 Task 及其当前引用对应的真实字节，不另外重新
核验尝试的整个历史 Artifact 前缀。Task 不可见时，已有提前返回路径检查来源和 Task 边界，
不会检查 Artifact 命名空间；`NotVisible` 不是整个工作区健康结论。命名空间固定只保护各自事务，
不防御调用前由特权者替换的同名副本，也不核验全部数据库历史。

## 状态与恢复限制

观察只不可变借用尝试。匹配、不匹配、不可见、读取错误或取消，都不会追加确认、标记 Task
完成、解除 `Unconfirmed` 或允许 `advance`。各次观察有各自读取时点，写者可能在“不可见”之后
完成；没有跨调用一致快照，也不保证观察结论一直有效。

尝试仍是依赖原 Session 的调用者内存状态，销毁它或退出进程就会丢失上下文。不提供持久日志、
回执导入、重新登录恢复、重试、删除、清理、来源安装、公共 HTTP、浏览器接线或部署权限。

0.5.3 收录的 [C2-R 元数据检查点](session-task-draft-checkpoint.md)不改变这个准入边界：解析检查点
不能重建原尝试，也不能发起观察。

独立的 [C2-S 检查](session-task-draft-inspection.md)使用调用者提供的当前权限，而非 C2-Q 的
原指纹，不能替代此操作，也不验证历史调用者。

## 验证边界

**2026-10-07**，**七项默认 Rust 单测、十二组准确枚举的隔离 PostgreSQL 用例及三项公共守卫
全部通过**。分别覆盖比较、当前权限、精确读取范围、类型化失败、进行中的写者和取消读取。
这 22 项新增检查不证明先前写入来源，也不构成已接通的浏览器或部署流程。

纯测试与隔离 PostgreSQL 用例须分别覆盖状态/token 准入、各类观察结果、类型化读取错误、
尝试状态不变、当前 Session 拒绝和精确读取范围。SQL 精确入口为
`scripts/test-session-task-draft-observation.sh`，选择 `session_task_draft_observation_tdd` 目标。
分日期执行证据记录在[项目状态](project-status.md)，[测试指南](testing-guide.md)将观察检查与
写入、浏览器及部署验收区分。C2-P 历史记录保留原范围。
