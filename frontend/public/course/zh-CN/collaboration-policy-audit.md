# ADR-004：带操作者的策略命令与事务审计

日期：**2026-09-27**。状态：**独立 C1-C 准备层，收录于源码版本 0.4.5**。
基于 **0.4.4** 实现，下文原切片的验证记录保留历史事实。

后续：[ADR-005 / C1-D](collaboration-identity-lifecycle.md)通过另行显式升级身份 Schema 2，
为绑定/退役加入带操作者的生命周期审计。下文验证记录仍对应 C1-C 原切片。
承接 [ADR-003](collaboration-access-store.md)，不切换在线认证，不迁移部署数据库。

## 范围与信任边界

[策略命令](../../engine/src/services/collaboration_identity_store/policy_command.rs)为固定 legacy
Workspace 增加带操作者、请求 ID、预期修订的权限变更；
[审计存储](../../engine/src/services/collaboration_identity_store/policy_audit.rs)将回执与策略一起提交。
无新增 HTTP API、AppState 接入、启动迁移、环境配置开关、缓存或文件/内存降级。线上教师权威、
单人模式、私人内容所有权与现有 AuthService 行为保持不变。

`LegacyPolicyCommand.actor` 必须由可信适配器从已认证、未撤销的账号生命周期中解析。
**PrincipalRef 不是登录凭证；知道管理者 UUID 不代表具有其权限。** 本层核对指定主体的当前
持久权限，但不认证 Session，不允许直接把浏览器提交的 actor 交给它。在线接入仍须保留 Session、
CSRF、TOTP、账号删除/撤销与受保护操作的统一准入边界。

## 显式升级与基线

[升级器](../../engine/src/services/collaboration_identity_store/policy_audit_schema.rs)通过
`upgrade_policy_audit_schema` 将**权限 Schema 1 升为 2**；身份 Schema 与产品版本不变。
历史 [0007 模板](../../engine/migrations/0007_collaboration_access.sql)不改写，新增
[0008 模板](../../engine/migrations/0008_collaboration_policy_audit.sql)只由此显式升级器执行。

- 先核实 C1-A 账号生命周期与 C1-B 原教师部署 Grant。每个已建 authority 必须至少有一名
  未退役、active Human 且持有显式部署 Grant 的管理者。空库或未完成引导的 authority 拒绝升级，
  不自动创建超级用户；该入口不是新的账号初始化器。
- 在同一事务中锁住权限版本及 authority，阻止策略、身份和 authority 创建穿过基线快照。
  每条现有策略追加一条 `baseline`，保留其当前修订与规范状态；操作者、请求 ID、摘要、变更前状态
  均为空。只声明“迁移时观察到”，不补造缺失的历史修改。
- 建表、全部基线、版本更新共同提交。冲突表、非法策略、缺失结构或未知版本明确失败，不自动修复。
  重复/并发升级不会补写第二批基线。已升级时核对审计结构和追加保护触发器，不降级。
- 小规模准备入口最多处理 10,000 个 authority / 10,000 条策略，沿用十秒操作、两秒锁等待、
  五秒单 SQL 上限；超限/超时不是部分成功。大规模分批迁移须另行设计。
- Schema 2 下，旧 `replace_legacy_access` 返回 `AuditContextRequired`。只认识版本 1 的旧二进制
  也会拒绝新版本，不能继续无审计写入。Schema 1 的原行为及测试保留。

此准备层尚不提供升级后的新 authority 引导、紧急管理员恢复或通用 Workspace 管理。不可在没有
部署盘点、恢复验证和显式批准时升级现用库，也不能通过手改版本号或旧写入口绕过命令。

## 命令、幂等性与管理权

`PolicyCommandId` 是独立的非 nil 规范 UUID 类型，调用方在首次请求前稳定保存；重试不能重新生成。
命令摘要使用带版本的规范 JSON 元组和 SHA-256，覆盖请求 ID、完整 actor、预期修订、完整目标策略。
角色顺序按集合规范化。摘要版本与编码属于持久兼容约定，后续修改必须保留旧回执的解码能力。

一次调用在同一 authority 写锁/事务中：

1. 核对固定 Workspace 与 Schema；操作者须属于该 authority、绑定未退役 active Human，且当前
   显式部署 Grant 有效。teacher / owner 角色本身不授予管理权。
2. 查找该 authority 下相同请求 ID：摘要相同返回原历史回执，标记 `replayed=true`，不再次写权限；
   不同主体、预期修订、目标或内容返回 `CommandConflict`。不同 authority 的请求 ID 命名空间独立。
3. 新请求须满足 `None = 仅首次创建` 或准确修订匹配；两张策略表一起更新，变更才递增修订。
   同值命令不递增修订，但仍保存 `unchanged` 回执，不能丢失已接受命令的幂等记录。
4. 撤销最后一个有效实例管理者的部署 Grant 返回 `LastAuthorityManager`。有其他有效管理者才可
   自行撤权。成员暂停、空间归档本身不撤销独立部署权；显式撤销和身份停用的语义不变。
5. 追加包含前后状态、actor、请求 ID、摘要与数据库记录时间的回执，读回核对策略与回执，再提交。
   抑制/改写写入或提交失败不能留下只变权限、没留审计的成功结果。

重试前**先重新检查操作者当前管理权**。自行撤权后，即使原命令已提交，也不能凭原请求 ID 读取特权
回执；需由仍有管理权的主体核实。旧授权命令在后来撤权/账号退役之后重放，只返回原历史结果，
不会恢复权限，也不会把策略授予同名新账号。回执不是当前有效权限证明。

**错误、超时或取消仍可能与提交竞争，不等于已回滚。** 结果不确定时保存原命令并对账，不能换新 ID、
自动采用新修订再授权，或把权限预览当作授权租约。测试中的取消发生在明确的提交前阻塞点，不证明
任意时刻取消都能回滚。

## 审计读取与完整性边界

`legacy_policy_audit` 只向当前实例管理者开放内部读取，按 authority / Workspace / Principal
隔离。使用递增 sequence 的 keyset 游标，每页 1–100 条，游标为 JSON 安全正整数。
sequence 可以有空洞；它不是 C3 的跨实例提交偏移，不保证外部动作 exactly-once。

Schema 2 的策略历史读取、权限预览与新命令核对当前策略和最新审计快照。缺失审计表/记录、修订或
内容不符、未知字段、错误范围和非法快照明确失败，不返回缓存的允许结果。修复后的读取可重新尝试。
快照包含权限元数据，不保存密码、Session、TOTP 或私人内容正文；仍应限制数据库与日志访问。

数据库约束与触发器拒绝审计 UPDATE、DELETE、TRUNCATE；业务代码只追加。
这**不是防数据库所有者篡改的 WORM/签名账本**：特权 DDL、禁用触发器、改函数或直接写表仍属于
受控数据库维护边界。没有对外导出接口、通用业务 outbox、审计清理策略或全历史密码学验证。

原 C1-C 切片的最后管理者保护仅涵盖此策略命令；后续 [C1-D](collaboration-identity-lifecycle.md)
在显式升级身份 Schema 2 后也保护退役，并封锁无操作者的旧身份写入口。在线认证和特权 SQL 维护
仍不属于该协议，不能宣称“全系统任何路径都不会失去最后管理者”。

## 验证与下一步

[命令/并发测试](../../engine/tests/collaboration_policy_audit_tdd.rs)、
[权限与范围测试](../../engine/tests/collaboration_policy_audit/permissions.rs)、
[故障注入](../../engine/tests/collaboration_policy_audit/faults.rs)提供 16 条真实 PostgreSQL 用例，
另有默认关闭连接池测试及扩展的 ID 契约测试。先观察缺接口失败，再实现；CI 漏跑检查同样先失败后补齐。

本轮使用全新 PostgreSQL 16、私有 Unix Socket、随机 Schema 和合成账号，未读取现用数据库。
16 条审计用例与原有 15 条身份、15 条权限用例全部通过，覆盖并发去重/修订冲突、跨实例、首次授权、
退役同名重建、无操作者写入封锁、最后管理者、审计分页/追加保护、基线回滚、缺表、状态不一致、
抑制/篡改回执、触发器改写策略、延迟提交失败和审计插入前取消。

[CI](../../.github/workflows/ci.yml)逐条核对测试名存在并执行忽略的数据库用例；
[CI 回归](../../scripts/tests/postgresCi.test.mjs)防止遗漏或空选择。完整默认 Rust 回归和 format-only
质量门禁也已通过。未运行本轮浏览器、真实内核/局域网或迁移恢复验收，不把合成测试当作部署批准。

完整默认 Rust 回归为 **330 项通过、126 项忽略**，其中 46 项数据库用例已另行显式执行；其余
80 项忽略用例本轮未运行。通用检查 **74 项通过**，版本仍为 0.4.4，OpenAPI/SDK 冻结基线未改写。

```bash
# 必须是可丢弃测试库，禁止使用运行实例的 DATABASE_URL。
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_policy_audit_tdd --test collaboration_access_store_tdd \
  --test collaboration_identity_store_tdd -- --include-ignored --test-threads=4
```

下一步是选定持久账号生命周期权威，协调创建、删除、会话撤销和带审计的管理命令，再设计单一写入切换。
通用 Grant、主体/空间状态命令、在线检查/执行衔接和真实来源对账仍未完成；C1-C 不代表整个 C1 完成。
实现切片当时未升版、提交、推送或部署；现收录于源码版本 0.4.5，见
[发布验证](../../reports/releases/0.4.5/README.md)。0.4.4 的历史证据保持原样。
