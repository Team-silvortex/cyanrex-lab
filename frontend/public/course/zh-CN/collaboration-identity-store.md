# ADR-002：协作身份的持久化准备层

日期：**2026-09-27**。状态：**已实现显式、独立的 C1-A 注册表**。
基于 **0.4.3** 实现，收录于源码版本 **0.4.4**，未进行在线迁移。
[ADR-001](collaboration-foundation.md)继续作为契约基线。

后续：[ADR-003 / C1-B](collaboration-access-store.md)增加独立安装的成员关系与部署策略准备层。
下文 C1-A 范围及验证记录保留原身份切片事实；两层均未接入在线认证或受保护操作。
[ADR-005 / C1-D](collaboration-identity-lifecycle.md)后续加入显式身份 Schema 2、带操作者的绑定/
退役命令和审计头校验。以下无操作者维护行为仅适用于身份 Schema 1，历史验证数字不改写。

## 本轮实现

[CollaborationIdentityStore](../../engine/src/services/collaboration_identity_store/mod.rs)在 PostgreSQL
保存操作者固定的 authority/教学 Workspace ID、Human Principal 和旧账号生命周期绑定，提供初始化、
查询、绑定、退役操作。重复操作复用已提交 ID，不同存储实例通过数据库事务和锁协调。

这是**迁移准备层，不是在线认证或授权**：未接入 AppState、HTTP 或 Engine 启动；不读取旧用户、会话、
密码、TOTP 或部署配置；没有迁移现用数据库或部署。没有新增角色授权、Membership、登录 Cookie、
部署权限或后台写入者。

## 账号身份不等于用户名

可信调用方必须提供三个独立坐标：

| 坐标 | 作用 | 不能从哪些信息推断 |
|---|---|---|
| Authority + 固定 legacy Workspace | 选择核实后的来源实例和教学边界 | 主机名、文件路径、发现标签、浏览器字段 |
| 规范 username | 保留旧登录/查询名 | 对冲突来源做未经核实的名字归一化 |
| `LegacyAccountId` | 区分同名账号的每一次创建生命周期 | 密码/TOTP 摘要、用户名哈希、导入时间或推测的创建时间 |

`LegacyAccountId` 是独立的非零 UUID 类型。重复导入/重试必须保持不变，删除重建账号必须更换。
注册表**不会**在读取旧账号时临时编造它。后续认证适配器必须把它作为账号生命周期事务的一部分保存、
核实；历史身份不明确时先对账，不猜测。

`LegacyIdentityBinding` 保持 C-M1 形状，`StoredLegacyIdentity` 另带账号生命周期、Principal 状态
和可选退役时间。查询会保留停用/退役记录供操作者检查；返回记录不代表授权成功或已认证用户。

## 存储布局与安装

[增量 Schema 模板](../../engine/migrations/0006_collaboration_identity.sql)定义：

| 表 | 不变量 |
|---|---|
| `collaboration_identity_schema` | 唯一版本记录，目前只理解身份 Schema 版本 1 |
| `collaboration_authorities` | 持久 authority UUID，不等于旧实例标签 |
| `collaboration_workspaces` | authority 内的空间身份和状态 |
| `collaboration_legacy_workspaces` | 每个 authority 至多一个固定教学空间 |
| `collaboration_principals` | 带 authority 的 Principal 身份/类型/状态，与登录名分离 |
| `collaboration_legacy_identities` | authority/名字/生命周期唯一；同 authority/名字至多一个活动绑定 |

复合外键始终将 authority 与局部 ID 绑定。每个账号生命周期和 Principal 在所属 authority 的旧身份
绑定中只能出现一次，ID 不允许全零。活动用户名的部分唯一索引要求先显式退役，才能绑定同名新生命周期。
没有外键接入旧 `users` 表；打通在线账号生命周期仍是下一步的显式工作。

构造函数只接收连接池。`install_schema` 是对明确选定的可信数据库/Schema 执行的独立操作，事务内
使用 Schema 级 advisory lock 串行化初始化，版本记录和表一起提交，失败则回滚。已安装版本只核对
预期列，不重建消失的表。普通读取遇到缺失 Schema 或未知版本明确报错，不自动修复、降级或执行 DDL。

不要因为仓库里出现这个模板就向部署库执行。生产接入仍要求 C0 部署盘点、备份恢复验证、明确来源身份、
维护/切换计划及授权。原有启动建表路径不变。

## 操作边界

1. **初始化映射：** 显式给定 Authority/Workspace 对；重复调用返回相同映射。已有 authority 若传入
   不同 Workspace 会冲突，不留下新空间孤儿行。归档空间不会被初始化或绑定隐式恢复。
2. **绑定：** 重试携带相同且经过核实的生命周期，复用同一活动 Principal。另一个活动生命周期，或将
   生命周期用于另一用户名，都会失败。Principal 和绑定同事务提交，不发布未经确认的新 ID。
3. **退役：** 必须匹配已确认的 authority、空间、username、生命周期及预期 Principal。绑定墓碑和
   Principal 停用同事务提交。重复退役旧账号不影响同名新账号，不删除历史、不重新指定原归属。
   本层尚不删除旧会话或撤销在线访问。
4. **同名重建：** 新生命周期获得新 Principal；旧生命周期即使名字已复用也不能重新绑定。本注册表
   不继承或发放任何角色/Grant。
5. **读取：** 不分配、不迁移、不归一化、不改记录。预期列中的非法类型/状态明确失败；退役绑定却指向
   活动 Principal 被视为不一致。尚未退役但已停用的 Principal，也不会因再次绑定而恢复。

首版身份写操作按 authority 行串行化，包括尚不存在用户名的竞争。这是迁移准备阶段对简单正确性的
选择，不宣称高吞吐账号创建；锁来自数据库，不是进程内 Mutex，不同连接池/进程使用相同协议。
普通查询使用共享锁，不取得 authority 写锁。事务期间 Schema 版本保持稳定，未来迁移必须遵守该边界。

所有写操作核对影响行数，并在提交前读回预期状态。查询/解码错误、被触发器抑制的写入、延迟提交错误
或连接池故障都不能返回成功。没有内存/文件缓存，也不读取 `CYANREX_DB_FALLBACK`，不会锁存降级；
后端修复后可以重新查询。

每个调用的等待上限为 10 秒，事务锁等待为 2 秒，单条 SQL 为 5 秒。**错误、取消或丢失回执不证明
提交没有发生。** 显式重试前按同一映射/预期 Principal 核对，不能另造生命周期绕过不确定结果。
测试验证提交前取消，不能据此宣称取消总能撤销已完成的提交。

## 验证

[集成测试](../../engine/tests/collaboration_identity_store_tdd.rs)与
[故障注入](../../engine/tests/collaboration_identity_store/faults.rs)使用随机命名的隔离 PostgreSQL
Schema，不读取应用的 `DATABASE_URL`。CI Engine 任务显式列出 15 项持久化用例，逐项确认测试存在后
运行，使用自己的临时 PostgreSQL 服务。[工具回归](../../scripts/tests/postgresCi.test.mjs)会拒绝漏项
或意外空选择。

```bash
# CYANREX_TEST_DATABASE_URL 必须指向可丢弃的测试库，不是在线实例。
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_identity_store_tdd -- --include-ignored
```

本地验证使用新建 PostgreSQL 16 集群，仅开放私有 Unix Socket、不监听 TCP。15 项持久化用例及
1 项默认执行的关闭连接池用例全部通过，覆盖：

- 并发建表/空间映射/账号绑定、新连接池重载、账号生命周期竞争；
- 跨 authority、预期 Principal 不匹配、归档/停用状态及不一致记录；
- 退役后同名重建、重复退役、旧生命周期重放、初始化失败不留下孤儿；
- 插入/更新被抑制、SQL 故障、绑定/退役的延迟提交故障、提交前取消，以及移除故障后的重试；
- 缺失/不支持/损坏 Schema、只读查询不存在记录、不以易失内存冒充成功。

完整默认 Rust 测试集为 **328 项通过、95 项忽略**。上述 15 项持久化用例属于默认忽略项，已另行
显式执行；其余 80 项忽略用例本轮未执行。format-only 质量门禁也通过，包含 **72 项通用检查**、
文档镜像、版本一致性、OpenAPI/SDK 兼容和 Rust 格式检查。CI 覆盖已配置并在本地校验，本轮没有
触发远端 CI 或发布。

这些测试不证明在线账号生命周期、Membership/Grant 授权、会话撤销、停机对账、备份恢复或生产迁移
验收完成。合成测试数据库不是现有实例的备份。

## C1 剩余门槛

- 导入真实账号前，核实实例、持久/降级来源和恢复过的备份；保护、对账来源归属，不把同名登录当成旧身份。
- 持久保存账号生命周期，并与 AuthService 的创建/删除协调到一条经过审查的权威事务路径；先定单一
  写入切换与回退边界。
- 补齐 Membership/Grant 生命周期及策略检查，包括主体停用、空间归档、成员暂停、授权撤销、私人
  内容和跨空间访问。
- 完成后再将 legacy 适配器接入认证请求，保留旧 Session/TOTP/CSRF 和单人教师权威。空间 owner
  仍不能自动取得实例部署权。

本轮推进的是 C1-A，不是整个 C1；不升产品版本，也不自动发布。
