# 平台存储版本与作用域标记

于 **2026-10-15** 收录至 **0.6.0** 源码，见[发布验证](../../reports/releases/0.6.0/README.md)。
下列日期说明保留原实施范围，不是新的部署验收。

这是 2026-10-15 的 0.5.9 之后未发布跟进，在[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)
下增加独立的标记观察器，将五项存储声明绑定到原[数据库目录观察](platform-host-preflight.md)。
它不证明完整业务 schema 兼容，不是权限审计或可部署宿主；库仍未挂载。

## 支持的声明

`inspect_platform_storage_markers(config, connection, expected_catalog)` 借用调用者持有的
连接，要求与原目录观察的完整配置和身份匹配。只选择以下五张表的必要元数据字段：

| 配置命名空间 | 标记表 | 版本 | 比较的作用域字段 |
|---|---|---|---|
| Source | `collaboration_auth_source_schema` | 2 | authority |
| Source | `collaboration_identity_schema` | 2 | 此标记没有作用域字段 |
| Source | `collaboration_access_schema` | 2 | 此标记没有作用域字段 |
| Task | `collaboration_task_schema` | 3 | authority 与 workspace |
| Artifact | `collaboration_artifact_schema` | 1 | authority 与 workspace |

每个标记必须是普通永久表，没有 RLS、强制 RLS、继承或分区。可见列须恰为预期布局及内置类型，
非空且不是生成列或 identity 列，并核对 singleton 主键。记录必须恰好一条，singleton 为真，
版本受支持；Source、Task 和 Artifact 已有的作用域字段须准确匹配可信配置。

运行角色不能拥有这些表，也不能有其触发器权限。其他授权和全部业务表定义不属于此标记合同。
两个注册表版本号不证明工作区已注册、成员活跃、策略有效或审计历史一致。

## 事务与锁边界

观察器复用目录层对 SQLx 管理事务及手写外层事务的拒绝，自己持有一笔有界
`REPEATABLE READ, READ ONLY` 事务，不修改角色或 search path。

设置事务局部超时后，先按固定顺序尝试锁定五个完整限定的候选表名，再取得目录观察快照。
ACCESS SHARE 锁顺序避免用等待之前的旧快照批准等待期间变化的约束；之后必须通过原目录
比较，才选择任何标记行。因此，候选名锁可能在物理目标复核之前失败，错误类别不承诺目标
诊断优先级。

检查实际持有的关系锁、解析后的命名空间／关系身份和所选表 OID。标记通过完整限定的 `ONLY`
查询及 `LIMIT 2` 读取；经过类型保护的布尔投影避免将漂移字段的任意文本物化给调用者。
事务局部 `row_security = off` 使适用的 RLS 报错，而不是静默过滤，不授予绕过权限。

整体期限覆盖外层事务探测、锁等待、读取及提交／回滚。取得自有 SQLx 事务对象后取消，会为后续驱动
工作排队回滚；中断的 BEGIN 或断开的服务器仍有不确定状态。失败和超时均不允许接纳新对象、
修复、迁移或自动重试。

## 与原观察比较

`PlatformStorageMarkerObservation` 没有 Debug、Clone、序列化或公开构造，绑定已经核验的
目录和标记身份。`matches` 只被动比较观察，不重新读取存储，也不授予运行授权。

首次 `inspect` 没有旧标记身份；原目录仍匹配时，它可以观察当前有效的替代表。
仅凭目录观察无法固定标记历史。

`verify_platform_storage_markers(config, connection, expected)` 再取一份有界观察并与原值比较。
同名且格式正确的替代表也不能成为原标记。结构／版本不支持、作用域不符、身份漂移或无法
读取均返回静态错误，不披露 SQL 消息、凭据或业务正文。

## 验证与剩余门槛

显式一次性 SQL 测试在自有数据库安装真实模板，使用受限只读运行角色，独立检查标记访问、
业务列零读取权限、实际拒绝的投影及持久数据摘要不变。其余用例覆盖缺失／损坏标记、不安全
结构、原依赖和真实锁等待漂移。明确指定一次性 `CYANREX_TEST_DATABASE_URL` 后运行：

```bash
./scripts/test-platform-storage-markers.sh
```

运行器核对所有精确选择器，CI 复用已有一次性 PostgreSQL 夹具；不得指向应用数据。各自创建
的数据库和角色会在最终断言之前清理。

此 API 不读取用户、Session、凭据、Task／Artifact 正文、outbox 或审计历史。标记有效不证明
全部业务列／约束兼容、记录有效、当前权限、内容健康或部署可写。注册映射／历史、完整存储
合同、暂存根、进程权限和实际宿主组合仍是独立门槛。[Artifact 目录观察](platform-artifact-root.md)
也独立；不启动监听、不签发 Session、不 bootstrap、不迁移存储，也不接通浏览器 Task 保存。
