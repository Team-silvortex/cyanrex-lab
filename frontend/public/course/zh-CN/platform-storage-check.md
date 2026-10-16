# 单次平台存储检查

**2026-10-16 未发布**的[格式 3 诊断](platform-marker-expectation.md)在读取标记行之前核对声明
目录及全部五标记身份，一条新连接完成检查及关闭；格式 1／2 语义、进程／根门禁及仅检查输出
保留，不启动宿主。

已于 **2026-10-15** 收录至源码 **0.6.1**，见[发布验证](../../reports/releases/0.6.1/README.md)。
下文保留原实施阶段及验证范围；源码收录不代表部署验收。

这是 **0.6.0** 之后、**2026-10-15** 的未发布跟进，在
[平台宿主提案 3](https://github.com/Team-silvortex/cyanrex-lab/issues/3)下增加显式 Linux
`check-storage`。它把私有输入、进程、Artifact 根、数据库目录和存储标记检查接成一次诊断。
不启动宿主，不返回连接、保留基线、授权或可复用的就绪凭证；教学 Engine 及启动路径不变。

## 对显式选定的目标执行

沿用 [check-config](platform-startup-input.md) 的严格版本 1、2 或 3 JSON 与私有文件规则。
仅在独立审阅服务身份、补充组和本地数据库路由后执行：

```bash
cargo run --manifest-path engine/Cargo.toml --locked --bin cyanrex-platform -- \
  check-storage --uid 1000 --gid 1000 --groups 1000 --config /private/platform/input.json
```

这些数字和路径是示例，不是发现所得的默认值。参数顺序固定，空补充组须显式写 `--groups none`。
语法错误以 64 退出；进程、输入、根、数据库及关闭失败均以 1 退出，使用同一固定错误，不打印
路径、密码或驱动／SQL 原始消息。成功以 0 退出，仅打印
`platform storage observations completed; no host started`。

与 `check-config` 不同，此命令会打开配置的现有根和一条本地数据库连接。配置语法有效或根
不存在可以通过 `check-config`，但不能仅凭这些通过 `check-storage`。

## 顺序与限制

命令先按显式策略准备低权限进程，读取私有输入并复验单主线程状态。随后观察现有、有效 UID
所有且权限为 `0700` 的 Artifact 根，不创建或枚举内容，然后才构造 current-thread Tokio runtime。

格式 1 运行时调用 `check_platform_database_storage(config, endpoint, password)`。该独立库诊断只复用
连接器内部的显式选项／拨号构造，不从新发现的观察构造 `PlatformDatabaseConnector`。
同一个十秒预算覆盖一次连接尝试、原只读目录事务、原只读五标记事务及显式关闭连接。
检查失败也走有界关闭；检查或关闭任一失败均不能输出成功。

带有[版本 2 目录声明](platform-catalog-expectation.md)时改为调用
`check_platform_database_storage_expected(config, endpoint, password, expected)`，复用同一单次核心。
完整配置不符在拨号前拒绝；实际目录须与全部声明身份匹配，才进入标记检查。版本 1 保留原诊断。

运行时销毁后，即使数据库工作失败，也复验原 Artifact 根及进程。根元数据 I/O 在数据库期限
之外，同步文件系统（包括 NFS／FUSE）没有硬耗时上限。格式 1／2 的两笔事务（格式 3 为一笔
声明事务）及根快照不是跨资源
原子快照，也不是阻止同 UID／root 修改的锁。超时或终止会丢弃尚未发布的客户端状态，不证明
失联服务器已经结束回滚。

[目录契约](platform-host-preflight.md)检查配置的 system identifier／数据库 OID、准确角色、
列出的角色／命名空间限制和 Source-only search path。空命名空间不足以通过，还必须符合
[五标记契约](platform-storage-markers.md)的固定版本及配置作用域。只读必要标记元数据，不读
用户、凭据、Session、Task／Artifact 正文或审计历史；Artifact 检查只读目录元数据。

版本 1 检查本次观察到的对象；首次观察前已有的有效同名替换仍可能通过，版本 1 不提供
过去的 namespace／marker／root 身份。版本 2 对照显式声明拒绝目录身份变化，包括首次连接
之前的替换，但仍不提供历史 marker／root 身份。临时目录观察只用于将本次标记检查绑定到同一目标，
观察值最终全部丢弃；未来宿主启动仍需另行审阅的原基线。成功不表示完整 schema 兼容、业务
健康、可写性、权限历史有效、当前 Session 授权或部署验收。

## 连接与秘密边界

传输仍遵循[显式本地端点规则](platform-database-connection.md)：规范 Unix socket 或字面量
环回 IP，关闭 SSL。操作者须在密码交付前独立信任本地对端；物理身份在认证后核验，不是
TLS／对端认证。不加载 URL、pgpass、Engine 默认值或 `.env`。捕获到 PGOPTIONS 时会在网络
之前拒绝，包括空值；根元数据 I/O 可以更早发生。

不重连、不建池、不回退、不安装／迁移／修复权限，也不签发 Session。CLI 不安装驱动日志
订阅器，只输出固定消息；库调用方仍须负责全局日志策略（包括服务端通知）及进程准备。
私有输入不克隆或输出，丢弃秘密缓冲区不等于安全清零。

## 验证

```bash
cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_check_defaults_tdd
./scripts/test-platform-storage-check.sh
```

第二条命令要求显式一次性 `CYANREX_TEST_DATABASE_URL`、角色／数据库管理能力，以及运行角色
的密码认证。可仅对管理员使用本地 trust；所有角色都 trust 无法验证错误密码拒绝。CI 复用
已有 PostgreSQL 服务，先核对五个准确 SQL 选择器存在，再逐项执行。

默认测试使用真实环回对端验证启动错误脱敏、实际握手期限、socket 关闭及没有第二次尝试。
PGOPTIONS 与不安全根测试观察未拨号；inotify 阳性对照验证错误 UID 在配置 FIFO 打开前拒绝。
这些对端测试不覆盖已认证目录工作期间的取消。

SQL CLI 测试只在自有夹具安装真实模板，运行角色可读标记元数据但不能读业务列。覆盖成功、
错误密码／坐标、危险权限及异常标记，且没有自动修复。独立锁观察确认真实 marker 等待后才
替换根或终止直接子进程。数据／输入快照不变，诊断后台连接归零；最终断言前清理自有资源。
这些是本地诊断证据，不是多机局域网／TLS 或浏览器保存验收。

另行的[保留存储绑定连接器](platform-storage-connection.md)接收独立选定的原观察，可以交出
核验后的新连接。此 CLI 仍是单次诊断，不构造该连接器，不为它导出或采纳观察值。
