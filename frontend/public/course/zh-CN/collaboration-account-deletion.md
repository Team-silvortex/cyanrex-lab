# ADR-008：受限的会话授权账号删除

状态：**C1-G 受限内部准备层，收录于 0.4.7**，最初基于 0.4.6 提交
`0dc195086965f6ca6972c742a36f3c825bc6be41`，承接 [C1-F](collaboration-session-commands.md)。
不切换 live AuthService，不改公共 API/SDK、旧迁移、已有账号接入或部署。

## 选择纵切前映射的不变量

| 组件 | 已有不变量 | 删除联动要求 |
|---|---|---|
| 认证源 C1-E | 注册提交稳定且不可改写的 username/代次，Session 同时引用二者；源元数据锁保护空键 | 核实精确代次，同事务清除全部目标 Session 和账号 |
| 身份注册表 C1-A/D | 精确代次/Principal，永久墓碑，当前状态须符合审计头 | 退役并停用，保留归属和历史，同名重建不继承旧 Principal |
| 策略/审计 C1-B/C | 当前显式部署 Grant 独立于教师角色；authority 锁串行修改 | 核对当前管理权，保留策略历史，由退役阻止有效授权 |
| 退役 C1-D | 命令 ID 绑定 actor/载荷，保护最后管理者，追加审计，提交后返回 | 复用事务内待提交退役写入；其回执本身不证明认证源已删除 |
| 会话命令 C1-F | 源→注册表元数据→authority→子行；锁等待后及提交前实时核对 Session | 一开始就取得源 `FOR UPDATE`，禁止由共享锁升级 |

## 最小可用操作

[`delete_session_account`](../../engine/src/services/auth_service/durable_source/deletion.rs)
单独接收 token；`SessionDeleteAccountCommand` 只含命令 ID、Workspace、精确源账号引用及
预期 Principal。actor 来自当前持久 Session 和带审计的管理 Grant，调用方不能指定。
同一 Schema 须已显式启用源版本 1、身份版本 2、访问版本 2；不调用安装器或引导流程。

目标必须是**另一个账号**，精确代次仍存在，且绑定到预期的 active、未退役 Human。
即使存在其他管理员，也拒绝自删；未绑定、已停用/退役、来源不存在或跨范围目标同样拒绝。
这让本切片无需处理自撤销授权或另建删除回执账本。

一条事务持有源写锁和注册表 authority 锁，依次：

1. 核实当前 Session/管理者、精确源账号和身份绑定。
2. 执行既有 C1-D 退役、停用 Principal 并追加身份审计。
3. 再查目标来源，显式删除其**全部** Session（包含过期会话），核对影响行数，再删除精确账号。
4. 最终确认账号/会话不存在、带审计墓碑与回执一致；实时复核存活 actor 的管理权和准确 Session。
5. 确认 COMMIT 成功后才返回回执。

actor 不能是目标，最终仍有效的带审计 Grant 和精确源 Session 证明至少有一名源支持的管理者。
已删除/同名重建账号不能充数，C1-D 原有最后管理者保护也保留。策略修订、Membership、Grant、
资源归属和历史审计均不改写，由退役阻止有效访问。Workspace 归档本身不撤销独立部署管理权。

## 严格一次性结果，不新增重放协议

返回的 `LegacyIdentityReceipt` 记录身份退役；只有本适配器本次成功返回，才确认认证源删除与其
一起提交。旧回执编码、命令摘要、0001–0010 SQL 模板及独立 Retire 语义均不变。

本适配器**不返回成功重放**。成功后来源不存在会返回 `AccountMismatch`；同名重建后旧代次仍
错配。旧命令 ID 换新代次/Principal 会冲突。独立 Retire 即使使用同 ID/载荷，也不能作为删除
仍存在源账号的许可：已退役目标被拒绝，待提交退役回执的 replay 也在源写入前再次拒绝。

取消、超时、提交响应丢失**不证明回滚**。保留原命令 ID、actor 和目标坐标，通过可信维护入口
同时核对来源状态与身份审计。退役回执或 `AccountMismatch` 都不证明删除由此适配器执行；不能
换 ID、自动补偿或宣称已有可重试删除回执 API。持久删除回执/恢复、自删、未绑定及退役后清理仍待后续。

## 并发与失败边界

源 `FOR UPDATE` 在注册表锁之前阻挡注册/登录/退出和 C1-F 命令。登录先提交则其 Session 随后
被删除；登录排在删除后则无法认证旧账号。退出先提交会使删除准入失败；删除先提交则可在退出前
完成。策略撤权和删除共用 authority 锁，两名管理者中最多移除一名。会话读取和同名重建等待删除
提交后再观察不存在/新代次；独立注册表写入不得反向取得源锁。

抑制写入、缺审计、触发器改状态、等待期间过期、延迟提交失败、提交前取消均不返回成功。
沿用操作十秒/锁两秒/SQL 五秒限制；迟到的 COMMIT 仍可能跨越到期时刻或丢响应，测试不证明
持续授权或解决不确定提交。特权 SQL/DDL 与旧在线写入者并未被准备层封锁，禁止与 live AuthService
共用已启用认证源的表。

## 验证及后续门槛

[`durable_account_deletion_tdd`](../../engine/tests/durable_account_deletion_tdd.rs)、
[故障测试](../../engine/tests/durable_account_deletion/faults.rs)和
[并发测试](../../engine/tests/durable_account_deletion/concurrency.rs)新增 1 条默认关闭连接池测试、
16 条显式 PostgreSQL 用例。实现前已观察到缺失 API 编译失败。CI 先核对每条 ignored 测试存在，
再在一次性 PostgreSQL 16 服务逐条准确执行；[选择回归](../../scripts/tests/postgresCi.test.mjs)拒绝漏项。

```bash
CYANREX_TEST_DATABASE_URL='<一次性 PostgreSQL URL>' CARGO_BUILD_JOBS=2 \
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_account_deletion_tdd \
  -- --include-ignored --test-threads=4
```

开发容器已编译测试并执行默认用例，但仅 root 的用户映射阻止本地 PostgreSQL 初始化；数据库结果
须以 PR 的真实 PostgreSQL CI 为准，不能把本地 ignored 算作通过。本轮不代表发布标签、Release
Candidate Validation、在线迁移/恢复、部署或内核验收。统一 bootstrap、公开路由/CSRF/准入、
凭据修改、完整账号生命周期/对账及 live cutover 仍是独立后续门槛。
