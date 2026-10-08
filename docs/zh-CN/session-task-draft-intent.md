# 会话授权草稿意图记录

状态：**C2-T 内部准备层，已收录于 0.5.4**。决策日期：**2026-10-07**。
源码版本：**0.5.4**。

C2-T 从真实、尚未开始的 [C2-P 尝试](session-task-draft-publication.md)显式登记不可变发布意图
元数据，再允许经过独立授权的记录读取。它不发布内容，也没有把日志接入原 C2-P `advance`。登记不是
Artifact 或 Task 已派发、已提交的证据，也不是恢复协议。

## 显式安装与登记

受信任调用者构造 `DraftIntentJournal::new(pool, scope, installation_id)?`，再对独立的空日志
命名空间调用 `install_empty_namespace()`。日志 Schema **1** 是新的存储格式，不是产品版本，
也不迁移现有存储。安装标识必须是配置的 UUIDv4；没有新增启动钩子、安装 CLI 或环境命名空间收编。

后续 [C2-U 包装器](session-task-draft-dispatch.md)要求独立的 Schema 2 全新安装器。意图登记/
读取兼容两种格式，原安装器仍创建 Schema 1；不升级已有 Schema 1 命名空间，也不因登记而启用派发。

`attempt.register_intent(original_token, &SessionDraftIntentWorkspace)` 只接受真实尝试的
**Ready、零条已确认 Artifact** 状态，并使用原 Session 指纹。解析得到的检查点不能代替真实
尝试。它在当前 Session 事务中推导精确 `PrincipalRef` 与 `LegacyAccountId`，不信任调用者指定的
owner；事务固定当前认证源、身份登记与日志的命名空间身份。

记录包含严格 Ready/零进度的 [C2-R 检查点](session-task-draft-checkpoint.md)，以规范形式保存为
上限 **64 KiB** 的 `bytea`，另绑定所有者、账号代次及配置的认证源、Task、Artifact 命名空间名称。
不保存草稿正文、原始 token 或 Session 指纹；标题、标签、引用与摘要仍是私密元数据。
登记不查询目标 Task 或 Artifact 存储。

读取在驱动解码前把每个命名空间名称限制为 63 字节，同时限制检查点长度。两条命令均检查永久普通表、
准确主键及必需列形状，并保留事务中首次观察的表身份，随后复验；仅重命名有效约束不会被误拒绝。
这些检查拒绝结构或身份漂移，不是抵御特权数据库管理员的保证。

目标引用重复即为 **Conflict**，即使元数据完全相同也不收编已有记录、不返回幂等成功。日志写入
显式使用 `SET LOCAL synchronous_commit = 'on'`；成功确认仅对应数据库报告的 COMMIT 边界，
不证明宿主机存储配置、崩溃恢复或备份耐久性。超时、取消或未收到确认不证明回滚，也不允许盲目重试登记。

## 读取保留记录

`source.get_session_draft_intent(token, workspace, task_id)` 使用当前 Session 授权。重新签发的
Session 仅在**所有者及精确账号代次同时匹配**记录绑定时可读；用户名相同或取得教师角色都不能
替代这些检查。受信任工作区提供配置，记录不能自行选择任意来源或存储。

返回值是不可直接构造的 `SessionDraftPublicationIntent`，不是活跃发布尝试。读取不观察 Task
或 Artifact，不恢复进度、不追加确认、不推进写入，也不重试任何操作。C2-Q 仍要求存活的原尝试
及原 token；C2-S 仍是独立的显式目标检查，C2-R 解析仍只得到调用者数据而非权限。

## 记录的证明范围与剩余边界

日志安装 ID 区分配置的日志。保存的命名空间名称不能证明认证源、Task 或 Artifact 存储原来的
物理代次。记录和安装 ID 不提供对特权数据库管理员的防篡改保证、不认证备份回放后的历史，
也不证明过去发生过派发或资源提交。

现有 C2-P `advance` 仍可在没有日志时执行，因此本片**没有强制派发前持久化**。后续 C2-U 包装器
另在资源事务前提交 Unknown 步骤，并在精确绑定身份检查下共同提交标记与资源 SQL；并非只提交
日志再调用旧发布方法。该显式包装器要求存活原尝试；登记/读取仍不授权自动重试、清理、迁移、
重启恢复或收编尝试。2026-10-07 的 C2-T 证据不覆盖 C2-U。

没有新增路由、`AppState` 字段、公共 SDK、provider 传输、浏览器保存或自动安装。认证源/登记
格式及既有 Task/Artifact 授权边界不变。

## 验证边界

实现位于 [`draft_intent_journal`](../../engine/src/services/draft_intent_journal/) 与
[`draft_publication/intent.rs`](../../engine/src/services/auth_service/durable_source/draft_publication/intent.rs)。
`session_task_draft_intent_tdd` 目标和 `scripts/test-session-task-draft-intent.sh` 将登记/读取 SQL
检查与 C2-P 执行、C2-R 纯 codec 测试分开。准入、重复冲突、当前授权、精确账号代次、命名空间身份
和未知结果须有自己的证据，不能继承此前发布测试。执行范围见[测试指南](testing-guide.md)及
分日期[项目状态](project-status.md)。
