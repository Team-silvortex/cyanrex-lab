# 从可信声明打开保留绑定的平台存储

**源码 0.6.3 已收录**的 `open_platform_storage_expected`，按
[宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)，将完整可信的
[目录／五标记声明](platform-marker-expectation.md)接到真实的
[保留绑定连接器](platform-storage-connection.md)。与诊断不同，确认成功后返回持有经核验绑定的
能力；不启动宿主、不挂路由、不安装 schema，也不授予当前 Session 权限。

## 显式输入

此 Unix 库函数消费一个 `PlatformHostPreflightConfig`、本地 `PlatformDatabaseEndpoint`、完整
`PlatformStorageMarkerExpectation` 及真实 `PlatformArtifactRootObservation`，借用准确根路径和
密码。密码会复制到结果保留的私有连接选项；丢弃这些值不等于安全清零。返回
`Result<PlatformStorageConnector, PlatformStorageConnectionError>`。

可信调用方须独立选定目录／标记身份及原根观察。声明不是伪造观察，当前打开的自有根也不能
证明历史身份。不能仅凭名字、版本、作用域一致而补齐缺失声明；没有仅当前状态／仅目录的
重载、基线导出或更新方法。

须在秘密、运行时线程和拨号之前准备 [Linux 进程](platform-process-startup.md)。本库不降低
凭据、不准备进程，也不在交付口令前认证被替换的本地数据库对端。调用方仍负责本地传输信任
及 SQLx／服务端 notice 日志策略。

## 打开顺序

1. IO 前比较完整声明配置及原根的配置／原始路径拼写。不一致返回 `InvalidConfig`；尾部 `/.`
   等拼写不是原路径。
2. 复核调用方选定的原私有根，不采纳新目录。原根替换或不安全时，在拨号前返回 `Unavailable`。
3. 只构造一次原显式 SQLx 选项。坏密码或捕获到 `PGOPTIONS`（包括空值）均为 `InvalidConfig`，
   不改进程环境。
4. 在一份十秒数据库预算内，只拨号一次，以既有只读事务核完整预期目录和全部五标记；检查
   成败都关闭初始连接。检查成功但客户端关闭调用失败，仍不能发布结果。
5. 数据库阶段之后，包括正常返回的失败或超时，再核原根。只有检查、关闭及原根复核全部成功，
   才用真实标记观察、已消费的目录声明、原根及已构造选项组装保留连接器；不为组装结果二次读
   环境、连接或重新观察。

成功确认的是客户端关闭调用完成，不是服务端 Terminate 确认或已观察到后端消失。
返回连接器不持有初始活连接，没有连接池、秘密 getter、Debug、Clone 或序列化。其后显式
`connect()` 仍比较原目录、五标记和根，不刷新基线。业务命令须保留事务内新鲜 Session、命名
空间及资源校验。

## 失败与取消

观察、拨号、数据库超时和关闭失败均返回通用 `Unavailable`，不带驱动原始错误或秘密。
不安装、迁移、修复、收编替换身份、重试或清理应用数据。以后每次尝试都须另行提供完整可信
输入；失败不返回绑定，也不授权重试不确定业务操作。

同步文件系统元数据检查在数据库预算之外，没有硬耗时上限。根与数据库观察不是跨资源原子锁。
取消 future 会丢弃未发布状态，但不保证执行最终根复核、确认服务端回滚或立即消失远端后端。

原 `check-storage` 及 `check_platform_database_storage_*` 仍只返回 `Result<()>` 并丢弃观察；
格式 1–3 及其诊断含义不变。此工厂是独立可信库 API，不能把诊断成功当作
运行基线。

## 验证与后续组合

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_open_defaults_tdd
cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_open_tdd
bash scripts/test-platform-storage-open.sh
```

默认测试覆盖显式配置／根拒绝及真实本地连接探针。准确 SQL runner 要求一次性
`CYANREX_TEST_DATABASE_URL`，具备数据库／角色管理权且运行角色使用口令认证，验证真实声明、
只读权限、保留绑定 Session 生命周期、原身份替换、实际标记等待及取消；不证明已部署宿主、
浏览器交付或局域网／TLS 验收。

独立[共享 Session 路由](platform-session-router.md)现将登录和生命周期接到一份绑定；显式
[回环宿主](platform-loopback-host.md)组合格式 3 输入、声明式打开及有界本机监听。TLS、平台
浏览器认证及 Task／Artifact／索引接线仍属后续工作。教学 Engine、原在线路由、
OpenAPI 及 SDK 契约不变。
