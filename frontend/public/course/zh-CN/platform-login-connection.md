# 通过保留的平台存储登录

**源码 0.6.3 已收录**的 `PlatformLoginConnector`，通过显式保留的存储绑定为已有账号签发 Session。
它遵循[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，复用准备层 Source 登录算法；
不启动宿主、公开 HTTP 登录、设置 Cookie、注册账号或接通浏览器保存。

独立[登录 HTTP 适配](platform-login-http.md)现将此能力投影为有界请求和确认后的专用 Cookie；
仍须独立构造，未挂载到实际应用。

## 显式登录能力

`PlatformLoginConnector::new(storage)` 无 IO 地消费真实的 `PlatformStorageConnector`，从原配置
固定权威、Origin 和会话存量上限；`origin()` 只返回公开元数据。`login(username, password, otp)`
是一条完整命令，仅在 COMMIT 确认后返回原有秘密类型 `DurableLogin`。连接器和登录秘密均无
Debug／Serialize；连接器不提供 Clone、连接池、秘密 getter 或基线更新入口。

[查询／退出连接器](platform-session-connection.md)保持独立，不增加登录方法。构造只查会话的
HTTP 状态，不获得读取凭据或签发权限。调用方须先完成 [Linux 进程门禁](platform-process-startup.md)，
选择原目录／标记、私有 Artifact 根和另行配置的运行数据库角色，再构造相应能力。
不修改 CLI 配置格式、安装器、迁移或在线 AuthService。

## 密码工作间隙与提交顺序

旧连接池 Source 和保留绑定登录共用私有登录阶段，不复制密码、OTP 和存量校验。
这些阶段不公开半完成命令或可复用授权。

1. 建立新的原存储连接。在业务事务内核对保留的目录／标记，固定 Source 命名空间与关系身份，
   读取原账号凭据；再次核原存储后确认读取事务并关闭连接。
2. 在既有有界工作器上验证密码，不持数据库连接或 Source 锁。真正缺失的账号也做一次合成密码
   验证，但不能收编稍后出现的同名账号；存储或坏记录错误不回退成合成验证。
3. 经同一原绑定另开连接。使用 READ COMMITTED，核对最初 Source 身份、当前账号代次和凭据，
   并取得 Source 写入栅栏。
4. 消费 OTP 前执行原配置的会话存量限制。消费 OTP、插入准确一条只存 token 哈希的 Session，
   触发延迟约束；先复核原存储，再完成末尾 Source、凭据、水位、准确 Session 和存量证明。
5. 紧邻 COMMIT 尝试重新核验新鲜 OTP，此后不再添加 SQL 守卫。只有确认提交才交出
   `DurableLogin`，随后优雅关闭失败不撤销已确认结果。

token 保留原十二小时有效期，本片不记录日志、不明文持久化，也不经公共路由返回。
可信调用方负责安全交付；响应丢失或等待取消可能留下已签发但未送达的 Session。
未知结果不允许收编匹配行、重置 OTP 消费、自动重试、删除 Session 或重新初始化。

## 限制与数据库权限

保留绑定登录始终采用 `config.limits()`，没有无限额重载。过期会话行也计入存量，插入前后均在
Source 写栅栏下检查，因此独立连接器也在数据库内串行决定 OTP／存量。旧无限额登录入口仍独立，
不可把未经审阅的其他写入方混入同一激活认证源。

每个连接器保留既有有界尝试策略：每个用户名五分钟内至多接纳五次，至多跟踪 1,024 个名字，
确认成功后清除此用户名的记录。共享一个连接器即共享该状态，另行构造的连接器各有独立映射。
窗口从该用户名第一次获准尝试时开始。
全进程密码门禁仍至多派发四个作业、接纳二十个作业；等待取消后，实际作业继续持有名额。
这不是宿主入口限流、分布式滥用防护或恒定时间认证。

登录角色除原目录／标记观察外，还须显式获得凭据 SELECT、OTP 水位 UPDATE、Session
INSERT／读回及原 Source 行锁权限。代码不自行授权，不要求拥有 schema／数据库、创建 schema、
注册账号或删除 Session。较窄的查询／退出角色继续不能读取凭据，不应为使登录成功而静默扩权。

命令等待共用十秒预算，最终优雅关闭另有独立等待。同步文件系统观察不能被此计时器强制中断。
每次新连接准入时核根身份，不在整个 SQL 期间持续锁住目录；目录／标记和 Source 检查保留各自
事务顺序，不证明永久存储健康或内核隔离。

## 验证与后续边界

默认 API 检查与准确选择的一次性 PostgreSQL runner 分开运行：

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_api_tdd
bash scripts/test-platform-login-connection.sh
```

SQL runner 要求显式设置一次性 `CYANREX_TEST_DATABASE_URL`，具备数据库／角色管理权，运行角色
须使用口令认证；先核对每个准确测试名，再执行。夹具使用合成凭据、真实受限角色和私有根，
不测试或改写应用账号。

公共签发仍须接通实际宿主和安全同源 HTTPS；未挂载适配器已有界预认证准入和 Cookie 投影，
但不证明浏览器收到 Cookie。宿主启动和
Task／Artifact／索引命令须消费同一经审阅绑定，再接 UI／SDK 的保存、列表、重开。
现教学认证、未挂载的生命周期路由及只在本地保存的任务编辑器保持不变。
