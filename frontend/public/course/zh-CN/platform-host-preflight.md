# 平台宿主目录预检

于 **2026-10-15** 收录至 **0.6.0** 源码，见[发布验证](../../reports/releases/0.6.0/README.md)。
下列日期说明保留原实施范围，不是新的部署验收。

这是 2026-10-15 的 0.5.9 之后未发布跟进，在[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)
下增加显式宿主配置与只读目录预检。入口仍是未挂载的 Rust 库，不启动监听，不构造 Engine、
Session 签发器或存储安装器。现用教学运行时与认证不变。

## 可信配置

`PlatformHostPreflightConfig` 包含规范化的可信 origin、工作区引用、三个不同的私有命名空间、
预期数据库坐标和显式 `SessionStockLimits`。配置没有默认值、请求反序列化、环境查询、连接串
或秘密；构造时不做 I/O，复用原有 origin 与命名空间校验规则。

`PlatformNamespaces` 分开 Source、Task 和 Artifact 命名空间，协作注册表也在 Source 中。
`PlatformDatabaseTarget` 要求预期数据库名、运行角色、PostgreSQL system identifier 与数据库
OID。操作者须通过另行可信的部署流程获得这些坐标，不能把首次发现的目标自动收编为正确目标。

工作区与 Session 上限只是配置绑定，不证明持久数据属于该工作区，也不证明将来的登录入口
实际使用上限。独立的 [Session 存量策略](durable-session-stock.md)与
[Session 生命周期适配](platform-session-lifecycle.md)保持原有边界。

## 只读检查

`inspect_platform_database(config, connection)` 借用调用者持有的 `PgConnection`。它拒绝已在
事务中的连接，不嵌套到调用者的旧快照；自己持有一笔有界的 `REPEATABLE READ, READ ONLY`
事务，只在确认提交之后返回不透明观察值。不构造连接池，不修改角色、search path 或数据库权限。

SQLx 管理的已有事务直接拒绝。驱动深度不能识别手写 SQL BEGIN，因此另用单批创建并释放
savepoint 探测边界；已有调用者事务保持开启并被拒绝，不提交、回滚或接管它的工作。

操作有整体期限及事务局部的语句／锁等待超时。成功取得自有 SQLx 事务对象之后，调用者取消或
期限耗尽会使驱动为之后的连接工作排队回滚。中断的 BEGIN 也可能已到达服务器，却未返回事务
对象；错误不证明服务器事务已结束。在边界探测中取消，也不授权回滚调用者的工作。

观察只读取 PostgreSQL 系统目录，并核对以下条件：

- 实读 `pg_catalog.pg_control_system()` 的 system identifier、数据库名及 OID 必须匹配配置的
  物理目标，不回退到服务地址或只比数据库名。
- 当前用户、会话用户和配置角色完全一致。角色可登录、可连接，但不能是超级用户、数据库所有者，
  不能绕过 RLS、复制、创建角色或数据库；不能有任何直接角色成员关系，也不能有数据库 CREATE
  或 TEMP 权限。
- 三个命名空间都存在，授予 USAGE 但不授予 CREATE，且不由运行角色拥有。保存实际 namespace
  OID 与 owner OID；有效应用 search path 仅包含 Source，也不能存在临时命名空间。

操作者需另行允许运行角色在目标数据库执行只读 `pg_catalog.pg_control_system()` 函数。
缺少权限时拒绝预检；库自身不授权、不提权，也不换成更弱的身份判断。

这些检查只覆盖列出的目录权限，不穷尽可执行的提权函数或所有数据库访问。物理身份坐标不是 TLS
认证，也不能区分特权操作者制作的精确物理克隆；传输身份仍须单独验证。

## 与原观察比较

`PlatformCatalogObservation` 没有 Debug、Clone 或序列化实现。`matches` 比较完整配置和实际
目录身份，不是健康报告、连接工厂、授权凭证或可复用的存储防护。

`verify_platform_database(config, connection, expected)` 重新取得只读观察并与原观察比较。
即使命名空间名称与权限相同，被替换的对象也不能重新收编。配置漂移、不安全权限、目标不符和
目录不可读均拒绝，不返回原始 SQL 错误，也不修复任何对象。

这是某一时点的目录比较，不阻止之后的变更，不 pin 后续连接池连接，也不替代每笔业务事务内
原有的 Source 与资源权限防护。

## 验证与下一道门槛

默认测试覆盖纯配置校验；显式一次性 PostgreSQL 测试使用独立创建的数据库和受限运行角色，
验证目录边界。只接受明确指定的 `CYANREX_TEST_DATABASE_URL`，通过以下入口运行：

```bash
./scripts/test-platform-host-preflight.sh
```

运行器核对精确选择器，CI 复用已有一次性 PostgreSQL 服务。测试不得指向应用数据；各自创建的
数据库与角色会在最终断言之前清理。

空命名空间也能通过：此切片不读取业务表、凭据、Session 记录、权限审计或存储 schema 标记，
不证明 schema 兼容、工作区归属、权限历史有效或内容健康。独立的
[Artifact 根预检](platform-artifact-root.md)只检查目录身份，
[存储标记观察](platform-storage-markers.md)另查五项版本声明及已有作用域字段，不证明完整 schema
兼容；进程权限、安全连接
配置、传输验收和实际低权限宿主组合仍是独立门槛。没有迁移、bootstrap、自动清理、公开登录
或浏览器 Task 保存。

0.6.0 之后的未发布跟进另加 [Linux 进程启动检查](platform-process-startup.md)，在工作线程之前
限制并复核显式身份；它不是数据库连接工厂或运行宿主，原目录观察的连接限制保持不变。

独立的未发布[单次连接器](platform-database-connection.md)现可用显式提供的原观察核验每条新
本地连接；不把上述借用连接的观察 API 变为连接池、持续守卫、对端认证或当前资源权限。
