# 显式平台数据库连接

已于 **2026-10-15** 收录至源码 **0.6.1**，见[发布验证](../../reports/releases/0.6.1/README.md)。
下文保留原实施阶段及验证范围；源码收录不代表部署验收。

这是 **0.6.0** 之后、2026-10-15 的未发布跟进，在
[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)下增加单次连接器。
每条新连接都在交给调用方前与原[目录观察](platform-host-preflight.md)或显式可信
[目录声明](platform-catalog-expectation.md)比较。它不是连接池、
宿主监听、Session 签发器或存储安装器，不改变教学启动路径。

## 可信构造

`PlatformDatabaseConnector::new(config, original, endpoint, password)` 消耗原观察值，并要求完整
宿主配置一致；不建立替换基线，也不收编同名对象。配置仍包含准确物理数据库及角色、三个
命名空间、Origin、工作区及 Session 上限，但这些不证明业务归属或 schema 健康。

另一构造器 `::with_catalog_expectation(config, expected, endpoint, password)` 消耗首次连接之前
独立提供的[目录声明](platform-catalog-expectation.md)，同样要求完整配置一致；不会从首次读取
补齐身份。

`PlatformDatabaseEndpoint::unix(directory, port)` 接受规范绝对 UTF-8 目录，不允许空段、点、
父目录段或控制字符，完整 socket 文件名保守限制为 100 字节。
`::loopback(address, port)` 只接受字面量环回 IP，不接受 DNS、通配或局域网地址。
端口须显式且非零。

显式密码可以为空，最多 4096 个 UTF-8 字节且不能含 NUL。空值不是读取密码文件的请求。
连接器没有 Debug、Clone、序列化或选项／密码访问器，调用方仍须保护自己持有的秘密。

## 环境不提供连接权威

SQLx 0.8.6 的 `new_without_pgpass` 内部仍读取环境，因此这里不声称完全不读环境。
由于其 options setter 只追加不替换，捕获到 PGOPTIONS 时会在网络连接前拒绝，包括空字符串。
实际 host、socket、端口、数据库、角色、密码及应用名全部显式选择；不解析连接 URL，不读
pgpass，不清除进程全局 PG 变量，也不加载 Engine 环境默认值。

启动选项仅设置 Source search path、5000 毫秒语句超时、2000 毫秒锁超时及 10000 毫秒事务空闲
超时，应用名为 `cyanrex-platform-connection`。关闭普通／慢 SQL 语句日志，但其他驱动日志仍由
调用方负责；服务端通知仍可能经全局订阅器进入 `sqlx::postgres::notice`。

支持的本地传输显式使用 SSL Disable；继承的 PGSSL 证书值保持不生效，不作为回退。
**物理身份在数据库认证之后核对，而不是在密码交付之前。** 提供秘密前须独立信任 socket
路径或环回路由；这不是 TLS／对端认证、远程数据库支持或已部署局域网验收。

## 单次尝试与原始目标

`connect().await` 建立一条新 `PgConnection`，再调用原目录验证器与原观察比较，或观察后与
显式声明比较，整体期限十秒；
拨号阶段也有十秒上限。没有连接池钩子、自动重连、重试、回退地址或替换观察值。
只有核验成功才交出连接；失败或取消会丢弃尚未发布的连接状态。驱动、SQL 及网络原始错误
统一映射为固定 `Unavailable` 错误。

每次显式调用重新核对角色安全、物理目标及原命名空间／所有者。即使同名替换有效也会拒绝；
恢复准确原身份后，后续显式调用可以成功，但不会自动重连，也不授权重放业务命令。

返回的是可信调用方自有连接，**不强制只读**，不提供持续固定或当前 Session 权限。
只有连接器自己的验证事务只读；调用方之后仍可改变连接状态。业务命令须保留原事务内的
Source／资源防护。空命名空间可以通过，不读取存储标记、正文或权限审计历史。

## 验证与剩余组合

```bash
cargo test --manifest-path engine/Cargo.toml --locked --lib platform_host::database_connection
./scripts/test-platform-database-connection.sh
```

第二条命令要求显式一次性 `CYANREX_TEST_DATABASE_URL` 及角色／数据库管理能力，不回退到
应用 URL。四个准确 SQL 入口覆盖不同物理连接、设置、有效同名 namespace 替换、所有者漂移及
权限／控制函数失权。夹具只读状态来自其自身角色策略，不是 SCRAM 认证验收。
最终行为断言前清理自有数据库、角色及连接；CI 复用已有一次性 PostgreSQL 服务。

默认测试在子进程隔离 PG 环境污染，以私有 FIFO 检测禁止的密码文件读取。另用真实环回对端
验证内部拨号失败、取消及实际握手期限，不代表公共 connect 在目录验证阶段的取消验收。
库尚未与 [Linux 启动门禁](platform-process-startup.md)、连接池、平台路由或浏览器保存组合；
不安装、迁移、修复权限或签发 Session。

另行的[保留存储绑定连接器](platform-storage-connection.md)把本连接器与调用方选定的原标记／
根观察组合，核验后才交出新连接；不改变此底层 API，也不把一次诊断变成保留基线。
