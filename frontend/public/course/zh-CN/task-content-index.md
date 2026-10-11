# 所有者私有任务内容元数据索引

已收录于源码版本 **0.5.6**。下方分日期实施和验证保留原 0.5.5／未发布阶段及范围；
源码收录不启用网络传输或公共 API。

状态：**未发布的内部准备层**。决策日期：**2026-10-11**。
源码版本：**0.5.5**。独立的 `TaskContentIndexStore` 和 `SessionTaskContentIndexWorkspace`
从已有 Schema 3 任务内容读取有界元数据页，不安装存储，不开放 HTTP/SDK/UI，不读取 Artifact
文件，也不证明内容能重新打开。

索引与[内容持久化](task-content-store.md)、[会话内容读取](session-task-content.md)分开。
索引条目不是内容快照，也不是可复用的读写授权。本跟进不改变存储版本、迁移或恢复流程。

另有源码 0.5.5 上同日独立的 **2026-10-11 未发布[索引 HTTP 适配](task-content-index-http.md)**，
在精确 `GET /platform/v1/task-index` 投影已有会话页面。它独立构造且未挂载，本服务层契约和
原内容路由保持不变。HTTP 投影不让这些服务值变为可序列化 DTO，也不授予内容访问、浏览器
保存、SDK/OpenAPI 开放或恢复能力。

## 显式索引配置

`TaskContentIndexStore::new(pool, scope)` 只接纳 `definition: None` 的手动记录。
`TaskContentIndexStore::from_definitions(pool, scope, &catalog)` 显式配置中立索引，同时接纳
手动记录，以及完整已存定义仍与冻结 `TaskDefinitionCatalog` 相等的目录记录。配置目录可组合
多个包与准确版本，也可混合有自动评估绑定和明确无自动评估绑定的定义。无绑定定义仍为 `Some`，不是手动任务。

会话版本使用 `SessionTaskContentIndexWorkspace::new(namespace, scope)` 或
`SessionTaskContentIndexWorkspace::from_definitions(namespace, scope, &catalog)`。
命名空间来自可信配置，不是客户端路由，必须合法且不是系统命名空间。构造器不接受 Artifact
工作区或私有文件根；配置索引不安装或接管存储。

选中的目录记录缺少配置定义、完整定义漂移或模式错误时，返回 `InvalidRecord`，不静默跳过或
重新解释。中立索引不放宽原 `TaskContentStore` 和会话内容写入/get 策略：原有手动与目录句柄
仍互相拒绝对方模式。索引准入不执行提供器或评估。

## 分页与游标契约

可信方法为 `TaskContentIndexStore::list(owner, after, limit)`；会话方法为
`DurableAuthSource::list_session_content_tasks(token, &workspace, after, limit)`，从当前 Session
派生所有者，不接受调用方自报所有者。

`after` 是可选的 `TaskRef`；`limit` 默认 **20**，范围 **1–32**。按该所有者的 Task UUID
升序，选择严格位于游标之后的任务。每次至多读取 `limit + 1` 个候选任务头，即最多 **33** 个。
所有选中记录，包括额外一条前瞻记录，都复用已有严格 Schema 3 Task、清单及最新匹配 outbox
校验。不用 JSON 字段过滤隐藏所有者的损坏记录；前瞻记录损坏也让整页失败。
即使所有者的页为空，也用固定零行投影检查 Task/outbox 必需列是否可用，避免内容列缺失被
当成正常空列表。这些检查不解码他人的记录，也不证明完整 Schema 或全库记录健康。

`TaskContentIndexPage` 只含 `entries` 与 `next_after`。仅当有经过校验的前瞻记录时，
`next_after` 才是最后一条**返回条目**的引用，不是前瞻引用。末页或空页没有下一游标。
每个 `TaskContentIndexEntry` 准确包含以下七个字段：

| 字段 | 含义 |
|---|---|
| `reference` | 带作用域的任务引用 |
| `revision` | 当前任务修订 |
| `title` | 当前任务内容标题 |
| `status` | 当前生命周期状态 |
| `definition_ref` | 准确配置定义引用，手动任务为 none |
| `updated_at` | 当前任务更新时间 |
| `payload_items` | 清单项数量，不是正文或字节可用性 |

页面不返回正文、清单、所有者、完整定义或 Artifact 修订/摘要引用。元数据仍属于私人所有者，
不能当作公开目录数据。

游标只是位置，不是授权，也不证明目标存在或属于该所有者。同作用域中他人的任务引用、
不存在的 ID 都可作为位置；查询仍只选择当前所有者。不同工作区/权限域的游标拒绝。
不提供总数、搜索、跨页快照或完整清单保证。有限选择以外的记录没有检查，因此分页成功
不是整个存储的完整性或健康证明。
33 个任务头只限制返回/解码候选量，不限制 PostgreSQL 实际扫描或排序量。既有 Schema 没有
所有者/任务复合索引；命令和 SQL 时限约束等待，但本轮不宣称大规模列表性能已验收，也不增加索引或迁移。

## 事务与授权边界

直接可信索引在有界的**只读可重复读**事务中执行，确认提交后才返回。可信调用方须先为所给
所有者完成授权，所有者过滤不等于认证。存储检查 Schema 3、作用域及完整任务头/最新事件一致，
不检查 Artifact 存在、字节或历史来源。

会话索引只使用 `DurableAuthSource` 拥有的一个事务。原来源及登记表守卫先确认当前账户世代和
经审计的活跃成员资格，再读取 Task 元数据，并对有限候选任务头使用 `FOR UPDATE`。
提交前，同一守卫使用新鲜数据库时间复核原命名空间身份、准确 Session、账户及成员。
只有全部检查完成且提交确认后才给出待返回页面，沿用原十秒命令期限及语句/锁等待限制。

该路径不固定 Artifact 命名空间身份，不访问 blob 或文件。引用内容缺失或不可读时，索引仍可能
成功；重新打开内容仍需要原会话内容读取器及准确字节检查。页面不产生业务写入、保存回执、
HTTP 路由、Session 签发、浏览器保存、自动重试、清理或恢复尝试。取消或确认丢失不证明回滚，
也不允许复用授权。

## 源码库存与验证范围

源码选择为 `task_content_index_tdd` 与 `session_task_content_index_tdd`。
独立的 `scripts/test-task-content-index.sh` 选择**四条可信存储**与**八条会话 SQL 用例**，
共 **12** 条。这些数量是源码库存，不是通过结果。选择范围包含分页/游标边界、完整记录及
目录校验、私人所有权、Artifact 独立性、当前生命周期及锁等待取消/到期边界。

实际执行结果与未覆盖范围记录于分日期的[项目状态](project-status.md)。旧内容存储、会话
内容或 0.5.5 发布结果不证明本新增选择已执行、已接公共入口或已完成安装实例验收。
选择方式见[测试指南](testing-guide.md)。

实现依据：

- [可信索引与分页类型](../../engine/src/services/task_store/content_index.rs)。
- [当前会话索引适配](../../engine/src/services/auth_service/durable_source/task_commands/content_index.rs)。
- [可信存储测试入口](../../engine/tests/task_content_index_tdd.rs)。
- [会话测试入口](../../engine/tests/session_task_content_index_tdd.rs)。
