# ADR-003：旧成员关系与部署授权的持久化准备层

日期：**2026-09-27**。状态：**已实现独立的 C1-B 准备层**。
基于 **0.4.3** 实现并收录于源码版本 **0.4.4**，承接 [ADR-001](collaboration-foundation.md) 和
[ADR-002](collaboration-identity-store.md)，未切换在线权威路径。

后续：[ADR-004 / C1-C](collaboration-policy-audit.md)新增显式权限 Schema 2、带操作者命令和事务审计。
下文保留 C1-B / Schema 1 的设计与历史验证；升级后旧无操作者写入会被拒绝，在线认证仍未切换。

## 范围与实现

[权限存储](../../engine/src/services/collaboration_identity_store/access.rs)在显式身份注册表上增加
持久化 Membership 与原实例部署 Grant 状态；
[策略预览](../../engine/src/services/collaboration_identity_store/access_policy.rs)从数据库当前记录
检查四种迁移敏感操作。没有接入 AppState、HTTP 或启动迁移，没有导入角色、重写账号/会话或改变
运行实例的权限。现有 AuthService 与路由守卫仍是唯一在线认证和授权路径。

本层**不是**通用 CapabilityGrant 引擎、AI 委派、权限管理 API 或新的多空间运行时。写入要求可信
维护调用方、明确选定的测试/迁移数据库以及 C1-A 固定的教学 Workspace；环境变量、公开注册、浏览器
字段、资源 UUID 和发现标签均不能提供这种权威。

## 存储与修订保护

[独立 Schema 模板](../../engine/migrations/0007_collaboration_access.sql)新增：

| 表 | 不变量 |
|---|---|
| `collaboration_access_schema` | 显式权限 Schema 版本 1，与产品/身份 Schema 版本分开 |
| `collaboration_memberships` | authority/Workspace/Principal 唯一，记录 active/suspended、角色集合与修订 |
| `collaboration_deployment_grants` | authority/Principal 唯一，记录 active/revoked、原来源空间及相同修订 |

复合外键保留 authority 与成员作用域。Grant 的来源空间记录迁移出处；有效范围是原**实例**，不是
任意新空间或其他实例。撤销后的行保留，不删除。Grant 行缺失、修订不一致、未知 Schema 或非法记录
明确失败，不替换成默认值，也不自动修复。

必须先显式安装身份 Schema，再调用 `install_access_schema`。安装采用 advisory lock 与事务 DDL，
已有版本只核对结构，不重建丢失的表；不导入账号、不读取秘密。失败不会覆盖原先存在的其他表。
不要因仓库中出现模板就向部署库执行。

`replace_legacy_access` 将成员关系与显式部署状态放在一个事务中修改：

1. `expected_revision = None` 仅允许首次创建，从修订 1 开始。
2. 已有策略必须携带经过确认的准确修订，即使目标内容与当前相同也不能跳过检查。
3. 当前修订下，相同的规范策略不变；实际变更使两张表一起递增。修订必须是 JSON 安全正整数，耗尽则报错。
4. 旧创建请求和旧修订不能在撤权、暂停或后续变更后重新授予权限。没有无条件 upsert、自动冲突重试或
   按用户名继承授权。
5. 所有写入检查影响行数，并在提交前读回目标状态。触发器抑制写入、篡改结果或延迟提交失败均不能报成功。

首版角色词汇限制为 `cyanrex.teaching.teacher`、`cyanrex.teaching.learner` 和
`cyanrex.workspace.owner`。顺序按集合规范化；重复、未知、teacher 与 learner 同时存在或超过两个
角色均拒绝。owner 可以与一种教学角色并存。允许空角色集合：活动成员仍可访问自己的私人内容，但
没有教学审阅或部署管理权。

这些行记录**当前状态**，不是完整的权限修改审计历史。尚无操作者认证、追加式审计、授权过期、委派、
动态角色目录或 outbox。

## 有效策略与生命周期边界

四种预览都要求准确 authority、未退役的 Human 账号绑定和 active Principal。原始存储记录只供历史/
维护检查，不直接代表有效授权。

| 动作 | 附加条件 |
|---|---|
| 读私人 Artifact | 原空间与成员均 active，且内容属于本人 |
| 恢复 attempt | 同样仅限本人，教师和空间 owner 也不例外 |
| 审阅学生 attempt | 空间/成员 active，操作者为 teacher；目标是同空间下身份与成员均 active 的 learner |
| 管理部署 | 原 authority 下操作者拥有显式 active 部署 Grant，任何角色都不隐含此权 |

资源适配器必须提供核实过的存在性和所有权。输入刻意不包含目标 `owner_role`，审阅会在同一数据库
事务中读取目标当前成员角色。教师可审阅学生提交，不意味着能读取所有私人 Artifact。

部署权**独立于**空间成员和归档状态。暂停成员或归档教学空间会阻止空间内操作，但不会撤销已有实例
Grant；撤销部署需显式操作，停用/退役 Principal 则同时阻止两类访问。这使教学空间停用后仍能管理
实例。仅有空间 owner 或 teacher 角色不带部署权。未来旧系统迁移须明确携带已核实的原教师部署
Grant；当前单人教师行为不变。

C1-A 退役保留策略记录，但预览拒绝退役/停用主体；同名新账号获得另一 Principal，没有继承的策略。
不提供旧生命周期复活。退役或归档后仍允许将成员暂停并撤销部署；授予活动空间访问要求身份和空间
active，授予部署要求身份 active，但独立于空间状态。

## 并发、失败与接入限制

读取持有 authority 行共享锁，身份/权限修改持有对应写锁。遵守协议的独立连接池/进程会对尚不存在
的成员键竞争、权限变更和身份退役进行协调。相关身份、空间、成员、Grant、Schema 行也采用共享读取锁。
这是首版准备层的简单正确性边界，不宣称高吞吐账号管理；直接数据库修复及未来迁移必须遵守锁协议。

沿用十秒调用等待、两秒 SQL 锁等待和五秒单条 SQL 上限。没有缓存或内存/文件降级；后端修复后可重新
读取，不锁存允许结果。**报错、超时或取消不证明事务没有提交。** 先按原 Principal/修订对账，再进行
单独确认的重试；不能把刚读到的新修订自动视为重新授予已撤销权限的许可。

`preview_legacy_access` 是某一时刻的对比，不是租约或 bearer 凭证。事务锁在调用方执行受保护操作前
已经释放；未来在线适配器必须把已认证生命周期、真实资源归属、当前策略和受保护操作放到经过审查的
同一权威路径。简单“先预览，再独立写入”仍有检查与使用之间的竞争。旧 Session、CSRF、密码/TOTP
和资源生命周期要求继续适用。

## 验证

[存储/并发用例](../../engine/tests/collaboration_access_store_tdd.rs)、
[隐私/生命周期用例](../../engine/tests/collaboration_access_store/policy.rs)与
[故障注入](../../engine/tests/collaboration_access_store/faults.rs)包含 15 项真实 PostgreSQL 测试及
1 项默认执行的关闭连接池测试。复用随机隔离 Schema，不选择应用 `DATABASE_URL`。全部通过新建
PostgreSQL 16 集群验证，仅私有 Unix Socket、不监听 TCP；同时重跑了之前 15 项身份持久化测试，全部通过。

覆盖并发初始化/创建/更新、独立重载、旧修订重授权、教师/学生/owner 隐私、角色变化、暂停/归档、
跨实例、退役同名重建、建表回滚、损坏状态、被抑制/改写的写入、延迟提交失败、修订耗尽、提交前取消，
以及等待撤权事务后的读取。

CI 显式列出每项持久化用例，并先核对准确测试名存在；
[CI 回归](../../scripts/tests/postgresCi.test.mjs)拒绝漏项和空选择。本地验证没有触发远端 CI 或发布。

完整默认 Rust 测试集为 **329 项通过、110 项忽略**。其中 30 项忽略用例（身份 15 + 权限 15）已在
隔离数据库显式执行，其余 80 项本轮未跑。format-only 质量门禁通过，包含 **73 项通用检查**、文件
行数、版本、文档镜像、OpenAPI/SDK 兼容及 Rust 格式检查。不宣称浏览器、真实内核、局域网、部署迁移
或备份恢复验收完成。

```bash
# CYANREX_TEST_DATABASE_URL 必须指向可丢弃测试库，不能使用运行实例。
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_access_store_tdd --test collaboration_identity_store_tdd -- --include-ignored
```

## C1 剩余工作

- 在选定的 AuthService 生命周期中持久保存、核实账号生命周期，协调创建、删除、会话和策略，设计单一
  写入切换。旧内存降级不是持久身份源，不能悄悄向新表做尽力双写。
- [C1-C](collaboration-policy-audit.md)已补齐准备层策略命令与审计；接入在线授权前仍需可信身份
  适配、生命周期审计、通用 Workspace/Principal 状态命令和更完整的 Grant 模型。
- 接触真实账号前完成部署盘点、备份恢复验证、来源对账及显式切换批准；合成注册表不是生产备份或迁移。
- 将策略接到受保护操作的事务/准入边界，再端到端验证旧登录、教师与私人内容权限、会话撤销和跨空间拒绝。

C1-B 推进的是准备层存储与对比，不代表整个 C1 完成，也不改变产品版本。
