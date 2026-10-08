# 日志化会话草稿发布

状态：**C2-U 内部准备层，已收录于 0.5.4**。决策日期：**2026-10-08**。
源码版本：**0.5.4**。

C2-U 为仍存的 [C2-P 发布尝试](session-task-draft-publication.md)新增显式日志化包装器。每个资源
步骤先提交一条 Unknown 日志，再在独立授权的事务中共同提交资源 SQL 与完成标记。连续调用可以
创建包含多个内容项的 Task，但不是整份草稿事务，也不是重启恢复或重试协议。

## 独立安装与所有权

`DraftIntentJournal::install_empty_dispatch_namespace()` 只在全新空命名空间显式安装**日志
Schema 2**。原 Schema 1 安装器、登记与记录读取继续支持；Schema 1 不能派发，也不自动升级。
没有迁移或启动安装器。

先登记真实初始尝试的 [C2-T 意图](session-task-draft-intent.md)，再调用
`attempt.into_journaled(workspace)` 消耗原尝试，得到 `SessionJournaledTaskDraftPublication`。
转换只是 Ready/零确认状态的纯所有权检查，不读取数据库，也不证明登记已提交；保留原认证源、
受信任工作区、Session 指纹、已分配引用及正文。包装器没有 `Clone`、导入器、unwrap，不能取回
未日志化的原尝试。
元数据也应保持私密，不将正文或 token 写入日志。

只有 `wrapper.advance(original_token)` 才派发步骤。它检查原 token，每次至多处理一个已分配
Artifact，或最终 Task；每个数据库事务均独立检查当前 Session 权限及意图绑定的精确所有者与
账号代次。已加载意图或解析检查点不能构造包装器，也不能授予派发许可。
错误 token 在派发前拒绝，不消耗包装器的 Ready 状态。

不可变意图保持原规范 Ready/零进度检查点，进度另存为有序步骤记录。最多 32 个 Artifact 步骤和
一个 Task 步骤；已占用的序号不会被收编，相同 nonce 也不构成重试权限。

## 每步具有两个事务边界

| 阶段 | 检查与变更 | 确认边界 |
|---|---|---|
| A 预留 | 当前 Session、精确绑定意图与原检查点、完整 Committed 前缀；为下一序号写入随机 nonce 及 Unknown 状态 | 单独同步提交日志；未确认 A 的 COMMIT 就不派发 B |
| B 开始 | 重新授权，复核精确意图、所有者/账号、序号和 nonce；在尚未提交的资源事务内条件更新为 Committed | 标记尚未对外提交，更新发生在业务校验之前 |
| B 资源 | 使用已有借用事务的 Artifact 发布，或同事务内的 C2-M 内容校验与 Task 创建 | 资源元数据/outbox 与步骤标记属于同一个数据库事务 |
| B 结束 | 只读复核精确意图、完整步骤/nonce 前缀及关系身份，再要求同步提交并复查新鲜权限 | 只有确认 COMMIT 后才推进包装器内存中的已确认前缀 |

先更新标记，确保其数据库触发器在业务校验前执行。最终日志检查只读取，不修复数据，也不在业务
校验后再写一次完成标记。完整前缀比较保留 B 观察到的先前已提交 nonce，不只核对当前序号。Task 创建仍经
C2-M 重读准确引用的内容字节。

日志没有把各 Artifact 发布与后续 Task 创建合并。每次成功只提交一个资源，后续失败或 future
被丢弃不会撤销此前提交。空草稿直接进入 Task 步骤，仍遵守相同的日志与授权边界。

## 结果不确定时停止存活包装器

一旦派发步骤，失败、取消或 future 被丢弃都会使活跃尝试停在未确认步骤。A 的 COMMIT 未确认就
不调用 B；B 未提交时，A 的持久 Unknown 记录保留，也可能留下私有 Artifact 文件，均不自动删除。
B 的 COMMIT 结果不确定时，资源和标记可能已一起提交，因此错误或缺少回执不能证明存储标记仍为
Unknown，也不能证明写入已回滚。

没有自动重试、重复序号收编、文件清理、状态修复或依据后续观察补确认。只有仍被调用者持有的
包装器能在一步确认成功后继续；丢弃包装器或进程退出会失去继续能力。读取意图或检查点不会恢复
正文、原尝试或执行权限。

## 限制与既有路径

原 C2-P `advance` 仍是独立、未日志化的内部 API。新包装器没有隐式要求所有调用者使用日志，也不
接通公共/浏览器保存。命名空间名称与事务内身份固定不能证明 A/B 之间物理认证源的连续性或跨重启
的原存储代次，不能抵御特权管理员或防止备份回放。同步 COMMIT 仅对应数据库确认边界，不是宿主
fsync、故障切换或崩溃恢复验收。

没有公共 HTTP/SDK 路由、浏览器连接、在线认证切换、尝试复活或新部署权限。C2-T 原 Schema 1
证据仍限于登记/读取；C2-R 解析和 C2-Q/S 观察仍是数据/读取操作，不是恢复许可。
新 Session 能读取 C2-T 记录，不代表它可以替代此包装器要求的原 token。

[C2-V 日志检查](session-task-draft-journal-inspection.md)另允许当前获权 Session 在包装器已
丢失后读取已记录前缀；不读取资源、不暴露 nonce，也不提供派发回执或继续、收编、重试包装器的入口。

## 验证边界

包装器位于 [`draft_publication/dispatch.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/dispatch.rs)，
步骤日志位于 [`draft_intent_journal/dispatch.rs`](../../engine/src/services/draft_intent_journal/dispatch.rs)。
认证源拥有的 [A/B 事务适配器](../../engine/src/services/auth_service/durable_source/draft_publication/dispatch_transaction.rs)
使用私有封闭许可，不能从存储数据直接构造。
`session_task_draft_dispatch_tdd` 将 Schema 准入、双事务顺序、完整前缀、资源故障、取消和授权
变化与此前意图记录测试分开。执行记录见[项目状态](project-status.md)，[测试指南](testing-guide.md)
区分这些内部检查与公共保存、已部署恢复。

2026-10-08 的十四条准确枚举的隔离 PostgreSQL 用例与三项公共守卫通过。其范围是日志化派发
边界，不代表默认/完整门禁或相邻套件结果，也不证明导入恢复、A/B 之间物理源连续性、浏览器
保存或部署验收。
