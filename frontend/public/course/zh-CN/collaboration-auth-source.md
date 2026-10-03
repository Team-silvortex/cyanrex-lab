# ADR-006：持久账号代次与会话来源

日期：**2026-09-27**。状态：**C1-E 内部准备层，收录于源码版本 0.4.6**。
承接[带审计的身份生命周期命令](collaboration-identity-lifecycle.md)。本轮不是 AuthService 切换、
已有账号迁移、新公开接口或部署变更。

后续：[ADR-007 / C1-F](collaboration-session-commands.md)组合当前会话核实与绑定/策略命令，
未开放删除/退役联动或切换在线认证。下文来源范围及验证保留 C1-E 原切片记录。

## 决定与范围

[持久认证源](../../engine/src/services/auth_service/durable_source/mod.rs)接收注入的 PostgreSQL
连接池和明确固定的 `AuthorityId`。不读取环境配置，不种子化教师，不设内存凭据/会话存储，也不在
数据库失败时降级。一个 Schema 只属于一个认证源；换 authority 打开会失败，不新建另一套身份。

注册把随机非 nil 的 `LegacyAccountId` 与用户名、凭据一起提交；会话同时绑定**用户名和准确的
账号创建代次**。删除后同名重建使用另一代次，不能继承旧会话或注册表身份。安装后的 Schema
禁止原地改写账号/会话坐标及会话摘要。

| 内部操作 | 确认结果与边界 |
|---|---|
| `install_empty_schema` | 仅显式安装到不存在或结构兼容的空认证表；不导入、回填账号 |
| `register` | 提交确认后返回新代次及 TOTP 设置；不创建 Principal、角色、Membership 或 Grant |
| `login` | 核对密码/TOTP、当前代次并提交会话；数据库仅保存 token 的 SHA-256 摘要 |
| `validate_session` | 只读、实时核对准确账号/会话及过期时间；不清理、不查角色、不分配身份 |
| `logout` | 确认删除这一条会话，不存在时幂等；不是账号退役、设备退出或内核资源清理 |

这些只是可信适配器的内部原语，注册不是公开加入或教师引导。`DurableAccountRef` 和
`DurableSession` 都可由调用方构造，且只是读取时的数据/快照，**不能作为后续身份或策略命令的
授权凭证**。不接入路由、AppState，也不切换现有 AuthService；HTTP/SDK 及教师/学生流程不变。

## 显式安装，不推断迁移

[安装器](../../engine/src/services/auth_service/durable_source/schema.rs)以 Schema 级 advisory lock
串行化安装，再锁住两张认证表检查为空；校验旧字段类型、非空约束、主键及级联外键的实际目标，
随后执行新增 [0010 模板](../../engine/migrations/0010_durable_auth_source.sql)，登记认证源 Schema 1
和固定 authority。DDL、约束与启用状态同事务提交。已发布的 0001–0009 模板不改写，启动和读取不调用安装器。

非空旧表直接拒绝，不分配 ID、不改凭据、不撤销会话。即使是空表，缺少会话主键也不兼容：字段
投影成功不证明唯一性。回归先复现该缺口，再补充目录校验，同时拒绝缺外键、凭据可空及时间类型错误。
这一要求记录在限定模块范围的[维护规则](../../.github/instructions/durable-auth-source.instructions.md)。

重复安装核对现有结构、坐标不可改写触发器、版本及 authority，不修补缺失状态。新代次字段无默认值，
旧 SQL 插入省略它会失败；**但这不能封锁旧 AuthService 的内存降级及全部旧写入口**：旧运行时仍可
降级、改凭据或删除记录。禁止把两套实现同时指向启用后的 Schema；真实切换必须另做旧写入封锁和
经过审查的迁移。结构检查/触发器只保护遵守协议的代码，不抵抗数据库所有者篡改。

## 事务与认证

认证源修改先对元数据行取 `FOR UPDATE`，一致性读取取 `FOR SHARE`，之后才锁账号/会话行，
包括尚不存在的键。当前有意让同源写入跨连接池串行；会话读取会等待未提交的退出，再观察删除结果，
而非返回缓存。整个调用十秒、锁等待两秒、单条 SQL 五秒上限。

复用现有 Argon2 异步密码助手，昂贵验证期间不占用数据库事务。登录先读凭据并验证密码/TOTP，
再取得源/行锁，完整比对凭据快照及账号代次后插入会话；插入后再查一遍，防止触发器在同一事务中
更改身份或凭据。增删确认影响行数及最终读回，提交确认前不返回账号 ID、设置密钥或原始 token。
含密钥的注册/登录返回包装不实现 Debug 或 Serialize。

会话从锁等待后的数据库实时时间起有效十二小时；读取也在行锁之后检查数据库实时时间，不使用
事务开始时间。过期会话只拒绝，不在读取时悄悄删除。取消、超时、提交响应丢失仍**不证明回滚**；
注册结果不确定时不能换代次重试，本切片不提供持久命令回执、重取设置密钥或自动补偿/对账接口。

登录准入有界但**仅进程内有效**：克隆实例共享最多 1,024 个用户名条目，每名在固定五分钟窗口最多
准入五次；包括存储错误在内的准入后失败计入次数，确认登录成功清除条目。独立构造的新实例不共享
此限制。密码最多 4,096 字节、OTP 最多 64 字节，注册密码至少八字节。向不可信客户端公开前，仍须
补齐分布式限流、公开注册策略、全局密码任务背压以及 HTTP/CSRF 接入。

## 下一接入门槛

C1-E 建立 C1-D 所需的持久代次来源。0.4.6 已包含的 [C1-F](collaboration-session-commands.md)
已用统一锁顺序把当前会话、绑定、策略/审计和提交组合起来；0.4.7 收录的
[C1-G](collaboration-account-deletion.md)追加另一个活跃绑定账号的受限删除/退役联动。同版还收录
[C1-H 改密](collaboration-password-change.md)、[C1-I 空实例引导](collaboration-bootstrap.md)、
[C1-J 本地初始化](collaboration-provisioning.md)和 [C1-K 只读对账](collaboration-reconciliation.md)内部准备层。
完整生命周期/恢复及在线接入仍待完成；不能先提交认证变更，再“尽力”写注册表。
独立 C1-D 退役仍不撤销 Session，本源退出也不退役 Principal。

真实数据接入另须来源盘点、恢复验证后的备份、明确迁移映射、旧写入封锁、切换批准及回退/对账边界。
本轮没有修改现用数据库、运行服务、账号或角色。

## 验证

[认证源回归](../../engine/tests/durable_auth_source_tdd.rs)及
[故障注入](../../engine/tests/durable_auth_source/faults.rs)包含 14 条显式 PostgreSQL 用例，
另有默认关闭连接池测试和有界/共享准入单测。缺接口、漏 CI 选择、接受不兼容空表均先观察失败后修复。
覆盖并发安装/注册、authority 错配、重载/同名重建、密码/TOTP、会话摘要、过期/退出、坐标不可改写、
代次错配、缺失/未知存储、抑制/改写落盘、延迟提交失败、提交前取消、退出/读取顺序，以及插入期间
更改凭据/账号代次。

本地使用 PostgreSQL 16、私有 Unix Socket 和合成账号：新增 14 条、原有协作 62 条、旧认证 22 条
全部通过，共 **98 条数据库测试**。完整默认 Rust 测试 **333 通过、156 忽略**；上述 98 条另行执行，
其余 58 条忽略测试未在本切片运行。新增 CI 步骤的 14 条准确选择命令也在本地执行通过。
`quality-gate.sh --format-only` 通过 **76 条公共测试**以及格式、文件长度、版本/文档同步、API/SDK
契约和工具检查。本切片未运行浏览器、前端构建、真实内核/局域网、部署/恢复验收或联网依赖审计；
工具自身的冒烟检查不是内核或制品验收。

[CI](../../.github/workflows/ci.yml)逐一列出并执行全部 14 条默认忽略用例；
[CI 回归](../../scripts/tests/postgresCi.test.mjs)拒绝漏项或空选择。仅可通过
`CYANREX_TEST_DATABASE_URL` 使用可丢弃测试库，不能使用部署的 `DATABASE_URL`。

```bash
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test durable_auth_source_tdd -- --include-ignored --test-threads=4
```

实现切片当时基于 0.4.5 验证，未发布或部署；现收录于源码版本 0.4.6，见
[发布验证](../../reports/releases/0.4.6/README.md)。历史报告及冻结的 API/SDK 证据保持不变，
本次源码发布不切换在线认证。
