# ADR-010：空实例的原子初始化引导

状态：**C1-I 内部准备层，收录于 0.4.7**，承接
[C1-E](collaboration-auth-source.md)、[C1-F](collaboration-session-commands.md)、
[C1-G](collaboration-account-deletion.md)和 [C1-H](collaboration-password-change.md)。
一次显式、可信运维调用可在同一事务中创建首个来源账号、旧教学 Workspace、Human 身份、
所有者/教师成员关系、部署 Grant 和审计基线。这不是公开注册、在线部署变更或已有账号迁移。

## 范围与信任

[`bootstrap_empty_authority`](../../engine/src/services/auth_service/durable_source/bootstrap.rs)
接收操作者固定的 Workspace/authority、类型化用户名和新密码，authority 必须匹配注入的认证源。
调用方必须是可信本地初始化代码，连接池须固定到预期数据库/schema；不能是浏览器用户、被发现
的对端或第一个注册的人。本切片没有 HTTP/CLI 入口、启动钩子、环境变量默认值或自动调用。

所选连接只能解析到一个非系统命名空间，且没有临时对象。存在任何关系、函数或类型都会拒绝，
包括无关数据、旧认证表、已安装但为空的认证源、部分注册表，以及成功引导过的状态。不修复、
接管、清空或覆盖重试。这比独立准备层安装器更严格：应显式选择全新空命名空间，不能将其部分
安装结果交给本入口。非协作的特权 SQL/DDL 仍在适配器信任边界之外。

固定初始策略是活跃的 `cyanrex.workspace.owner` 与 `cyanrex.teaching.teacher` 成员角色，加上
本 authority 独立的显式部署 Grant。部署权来自操作者的引导动作，不从教师角色推导。这保留
单人使用时教师同时负责部署的模型，以旧教学 Workspace 作为第一个协作映射；不是其他实例的
全局权限，也不赋予读取其他人私人内容的权力。后续注册仍无绑定、无 Grant，须由当前带审计的
管理命令显式绑定/授权。

## 事务与结果发布

密码仍限制为 8–4096 字节，Argon2 和 TOTP 密钥准备在数据库锁之外完成。初始化器在已有的
有界事务内依次执行：

1. 在 DDL 和判空之前，按认证源、身份、访问策略顺序取得三个安装器已有的 schema 级 advisory
   锁。并发引导不能都把同一命名空间看作空库并分别创建首个管理者。
2. 确认命名空间为空，用未改动的 0001/0010 模板创建认证表，固定来源 authority/版本 1，取得
   来源写锁并验证来源约束。
3. 用未改动的 0006/0007 模板创建注册表和访问表；先取得身份/访问元数据写锁，再写 authority
   和子行，创建固定 authority 与活跃的旧教学 Workspace。
4. 复用待提交的注册写入器，生成随机新账号代次；创建 Human 绑定及规范化的所有者/教师成员关系，
   显式部署 Grant 从修订 1 开始。
5. 用未改动的 0008/0009 模板建立审计表，分别追加一条身份基线和策略基线，再激活身份/访问
   Schema 2。外部操作者不是已认证 Principal，不伪造历史 actor、命令 ID 或可重放回执。
6. 根据审计头、当前管理权复核完整的 Workspace/身份/策略链，确认仅有一套初始状态；再复核来源
   authority、凭据和账号代次。必须恰好一个账号、零个 Session；确认 COMMIT 才返回结果。

注册表辅助函数借用外层 SQL 事务，只返回待提交状态，不自行提交，也不反向取得来源锁。
抽取后的共享注册写入器保留普通注册不授权的语义。全新 Schema 引导与现有通用审计升级器不同，
不放宽后者的前置条件，也不开放在已激活的注册表上创建新 authority。

返回值包含已提交的账号、Workspace、身份、策略及一次性 TOTP 配对材料，含秘密的结果不实现
`Debug` 或序列化。不签发 Session；用户仍须通过正常密码/TOTP 登录后才能执行管理命令，初始化
数据库不等于浏览器已认证。审计激活会同时封锁旧的无操作者注册表写入口，唯一管理者保护也保留。

## 失败、并发与恢复限制

抑制/篡改写入、审计失败、延迟提交失败和测试覆盖的提交前取消，不留下半套账号、Grant、审计
激活或 DDL。沿用操作十秒、锁两秒、SQL 五秒限制。来源/身份安装器排在引导后时等待已有 advisory
锁；访问安装器可能因身份元数据尚不可见而安全失败。独立安装器先提交时，引导拒绝其非空结果。

重复调用、换用户名/作用域、删除首个来源账号后重试，均不会重新创建管理权或再次返回密钥。
这不是应急管理者入口。错误、取消或提交响应丢失**不证明回滚**；目前没有持久引导命令回执或
成功重放。基线记录状态，不证明谁做了初始化或 TOTP 密钥已送达。应通过可信维护入口核对固定
数据库/schema，不能通过删表、删除审计历史、换 ID 或静默补建管理者恢复。测试仅在确认自身
合成事务已回滚后，才主动重试。

## 验证与下一关

[主测试](../../engine/tests/durable_bootstrap_tdd.rs)、
[故障测试](../../engine/tests/durable_bootstrap/faults.rs)和
[并发测试](../../engine/tests/durable_bootstrap/concurrency.rs)新增一条默认关闭连接池用例与
14 条显式 PostgreSQL 用例。实现前观察到缺失 API 编译失败及 CI 缺少选取项失败。2026-10-02
在独立 PostgreSQL 16 运行，15 条全部通过。CI 先检查 ignored 用例存在，再逐条准确执行；
[选择回归](../../scripts/tests/postgresCi.test.mjs)防止漏项。

由于业务入口拒绝预装表，故障注入使用按随机 fixture schema 过滤的 DDL event trigger，要求
**带 event-trigger 权限的一次性测试库**。序列计数器可在回滚后证明目标行触发器确实执行，避免
无关的早期 DDL 错误被误算为故障测试通过。测试钩子不进入应用代码或生产迁移。
即使回调按 schema 过滤，PostgreSQL 的 event-hook 目录仍是数据库全局状态；整条用例在专用数据库
advisory 锁内安装、使用和回收钩子，修复已观察到的并行清理/缓存竞争。单条用例内部的引导与安装
竞争仍然并发执行，不靠串行化业务操作获得通过结果。

```bash
CYANREX_TEST_DATABASE_URL='<一次性 PostgreSQL URL>' CARGO_BUILD_JOBS=2 \
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_bootstrap_tdd \
  -- --include-ignored --test-threads=2
```

同轮通过 337 条默认 Rust 测试（219 条 opt-in 用例忽略）、161 条显式选取的 PostgreSQL 认证/C1
用例，以及格式/公共质量门禁，包含 80 条脚本回归、冻结契约/版本/课程同步与工具测试。十个 CI
步骤的脚本体在一次性数据库本地执行，没有启动远程 CI。构建、运行目录和数据库相互隔离，关闭
调试信息及增量编译以控制磁盘占用；未运行前端生产构建、在线迁移、部署或特权内核验收。

后续 [C1-J](collaboration-provisioning.md)补齐受控本地入口与私有密钥交付；密钥恢复、持久生命周期
对账、经审阅的迁移/恢复、公开路由/CSRF 接入和在线切换仍是独立门槛。
不修改冻结的 API/SDK 契约、当前运行配置、旧迁移或源码版本，也不代表
候选版、远程 CI、部署或内核验收。
