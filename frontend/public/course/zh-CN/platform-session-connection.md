# 保留平台存储绑定的已有会话操作

源码 **0.6.3** 已收录显式回环／TLS 宿主、受限 Session 签发及私人 Task 创建／列举／重开／清单换版。
不接通浏览器／SDK 保存、局域网或存储迁移；早期实施阶段标签保留原范围与证据。

Unreleased [原绑定文本发布](platform-text-artifact-connection.md)由两个 CLI 宿主显式选择（八路由／七通道）；
原工作区构造器仍保持七路由／六通道。

0.6.2 之后未发布的独立[登录连接器](platform-login-connection.md)增加显式受限内部签发。
本查询／退出能力及其 HTTP 状态不新增登录方法。
其独立[登录 HTTP 适配](platform-login-http.md)同样未挂载，不改变本能力。
[共享 Session 路由](platform-session-router.md)在两种独立命令能力间私有共享一份原存储绑定，
本连接器仍不增加签发方法。

**2026-10-16 收录于 0.6.2**，此 0.6.1 之后的跟进将
[保留存储绑定的连接器](platform-storage-connection.md)接到已有 Session 查询／退出及
显式 [Session HTTP 适配](platform-session-lifecycle.md)。此切片遵循
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，不启动监听或签发方。

## 显式组合

`PlatformSessionConnector::new(storage)` 消耗真实 `PlatformStorageConnector`，不做 IO。
权限域和 Origin 只来自连接器的完整原配置，不另接第二份参数。包装器没有 Clone、Debug、
序列化、连接池、秘密访问器或更新基线入口；命令只有 `validate_session(token)` 和
`logout(token)`，`origin()` 只公开非秘密元数据。

`SessionLifecycleHttpState::with_platform_storage(storage, max_in_flight)` 显式选择该后端并继承
原 Origin；旧 `new(source, origin)` 和 `with_capacity` 兼容保留。一个状态只选择一种后端，
平台失败不回退连接池。克隆共享原响应正文生命周期名额，独立构造的状态仍有独立预算。

构造不取得或收编观察。可信调用方必须独立选定原目录、五标记和私有根，并在秘密、工作线程
及连接之前另行应用 [Linux 进程门禁](platform-process-startup.md)。仅检查 CLI 不构造此包装器，
也不提供可填入它的观察值。

## 事务顺序

每个规范 token 命令只取得**一条新物理连接**，经过原根前检、有界目录／标记核验及根后检，
再在同一连接上持有 READ COMMITTED 业务事务，保留显式 READ ONLY 策略：

1. 在业务事务内核对保留的原目录和全部五标记的身份、形状、版本及作用域，持有关系锁直至结束。
2. 准备原 Source-only Session 操作：固定命名空间、原始 search_path 和关系，锁定来源行，
   再锁定／读取准确 Session 和账号，或删除该 token 摘要。
3. 再次核对同一份原目录／标记，不更新基线或重设 search_path。
4. 完成原 Source 复验，最后读取新鲜 Session 到期时间或核实准确 token 缺席。
   该最终读取与 COMMIT 尝试之间不追加存储守卫。
5. 只有确认 COMMIT 才返回成功，随后关闭连接，不重试。

私有准备／完成助手也供旧连接池 Session API 复用，但不公开半完成命令或可复用权限。
原登录、凭据、清理及资源命令保持各自事务边界。平台退出另在最终存储／Source／缺席检查之前
刷新延迟约束与触发器。

查询的非法 token 不连接并返回无 Session；退出的非法 token 是输入错误。规范未知或已过期
token 可直接确认退出，不单独预验证。只有确认 HTTP 退出才清除 `cyanrex_platform_session`；
存储、Source 或提交失败返回私有 `503 service_unavailable`、`unconfirmed`，不动平台或教学 Cookie。

## 权限与失败边界

运行角色合并五标记 SELECT 与原 [Session 列权限](platform-session-permissions.md)，并具有来源行
加锁所需权限。行锁／退出要求显式非只读策略，运行代码不通过 SET READ WRITE 绕开只读角色。
不需要读取密码盐／摘要、TOTP secret 或 OTP 水位；命令不安装表、用户、授权、命名空间、文件或 Session。

连接／数据库准备、业务事务和优雅关闭各有独立十秒预算，同步根检查仍没有硬期限。根身份只用于
连接准入，不在 SQL 期间持续加锁。标记观察不固定每张业务表的历史身份，Session 关系固定在每条
命令内取得并持有。这不是跨资源原子性、全局连接上限或完整宿主健康核验。超时／取消或未确认响应
不证明回滚，也不允许自动重放；已确认提交不会因后续关闭失败变成未确认。

## 验证与后续工作

默认传输／后端回归入口：

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_backend_tdd
```

真实数据库、角色、触发器和锁用例只使用一次性 `CYANREX_TEST_DATABASE_URL`，需要数据库／角色管理
能力及运行角色密码认证，再执行：

```bash
bash scripts/test-platform-session-connection.sh
```

脚本先核对每个准确用例名存在。测试使用已提交的合成已有 Session，不冒称公开登录／OTP 签发，
结束时回收自有根、数据库及角色。七个用例覆盖凭据受限成功、只读／缺权、原标记／根替换、真实
Source 行锁等待后过期、等待期间命名空间所有者漂移及取消、DELETE 重定向和延迟 Session 重插。
2026-10-16 在隔离 PostgreSQL 中七条新增及 64 条相邻 Session／OTP／存储 SQL 回归全部通过，
随后停止该实例并删除一次性数据。

正常应用／CLI 启动、局域网 TLS、公开 Session 交付、私人 Task
资源组合、浏览器保存、SDK／OpenAPI、迁移及恢复仍需独立推进。
