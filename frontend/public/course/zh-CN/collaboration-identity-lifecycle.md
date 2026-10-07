# ADR-005：带审计的身份绑定与退役

日期：**2026-09-27**。状态：**C1-D 独立准备层，收录于源码版本 0.4.5**。
基于 **0.4.4** 实现，下文原切片的验证记录保留历史事实。
承接 [身份注册表](collaboration-identity-store.md)和[策略命令/审计](collaboration-policy-audit.md)。
不切换现有 AuthService，不迁移现用数据库，也不声称在线会话已被撤销。

## 本轮边界

[生命周期命令](../../engine/src/services/collaboration_identity_store/identity_command.rs)为旧身份注册表
提供带操作者的 `Bind` 与 `Retire`：命令 ID、调用者、固定 Workspace 和完整账号坐标进入摘要；
Principal / 绑定状态与追加式审计一起提交，重复请求返回原回执。
现有身份 Schema 1 的可信维护入口保持原行为；**显式升级至身份 Schema 2 后**必须走命令入口。

这仍是内部准备层，不是公开注册或账号删除 API。调用者必须先通过可信认证，并核实来源账号的
`LegacyAccountId`；客户端自报的 actor、用户名、角色和 UUID 都不是身份证明。
本层不存储或复制密码、TOTP、Session token / 摘要，不新增登录协议、环境开关或易失降级。

## 为什么还不直接接入旧 Session

核对现有[认证服务](../../engine/src/services/auth_service/service.inc.rs)、
[会话路径](../../engine/src/services/auth_service/sessions.inc.rs)和
[账号修改](../../engine/src/services/auth_service/account_mutations.inc.rs)后，仍有以下边界：

- 旧持久账号和会话以 username 关联，没有持久的账号创建代次；相同用户名不证明是同一个账号。
- 旧账号/会话读取和注册仍可降级到进程内存。准备层不能用这些缓存证明一个持久身份仍然有效。
- 旧删除已有账号/会话事务，不能在其提交后再“尽力”调用新注册表退役，否则会出现两条写入权威。
- `PrincipalRef`、历史回执及权限预览均不能替代 Session、CSRF、TOTP 或当前资源权限检查。

因此本轮只补齐身份状态事务与管理准入。合成测试明确断言旧 `users` / `sessions` 不被改动：
退役使新准备层拒绝该主体后续授权，不等于关闭其旧 Session、终止连接或清理正在运行的内核资源。
实际在线删除/撤销仍由旧认证路径负责，切换前必须另做统一事务和失败语义设计。

## 显式升级

[升级器](../../engine/src/services/collaboration_identity_store/identity_audit_schema.rs)要求先启用
C1-C 权限审计（权限 Schema 2），再显式调用 `upgrade_identity_audit_schema`。
此操作将**身份 Schema 1 升至 2**；权限 Schema 和产品版本不变。历史模板不改写，新增
[0009 模板](../../engine/migrations/0009_collaboration_identity_audit.sql)不由启动、读取或构造函数调用。

升级在一个事务中锁住身份版本与 authority，逐个核对仍有有效管理者，然后为全部旧身份（包括墓碑）
记录 `baseline`。基线保留观察到的状态，不补造历史操作者、命令 ID、变更前状态或退役原因。
DDL、基线、版本更新共同提交；重复/并发升级只验证已有结构，不补写基线、不修复冲突表。
缺失审计结构、追加保护触发器、非法快照及未知版本明确失败。

沿用十秒调用、两秒锁等待、五秒 SQL 上限；本小规模入口最多处理 10,000 个 authority 和
10,000 条旧身份。更多数据需要另行设计迁移，不以部分成功或内存结果代替确认。

升级后，旧的无操作者绑定、退役及空间初始化写入口返回 `IdentityAuditContextRequired`；
只认识身份版本 1 的旧二进制也会拒绝新版本。读取已有空间改用原只读查询，不能把初始化当作读取。
本轮不提供新 authority 引导、紧急管理者恢复或通用主体/空间状态命令。

## 命令与状态规则

`IdentityCommandId` 为独立的非 nil 规范 UUID，不能与策略命令 ID 隐式混用。
SHA-256 摘要覆盖版本化 JSON 元组中的命令 ID、完整 actor、Workspace、动作及账号坐标；编码属于
持久兼容约定。命令 ID 在 authority 内唯一；同 ID 不同 actor、动作、代次或目标会冲突。

每次调用（包括重复回执）先核对当前管理权：操作者必须是同实例、未退役的 active Human，且有
当前有效的显式部署 Grant。teacher / owner 角色本身不提供这项权威，退役或撤权后不能借旧命令读取审计。

| 动作 | 状态与审计规则 |
|---|---|
| Bind | 仅活动 Workspace；核实账号代次后创建 Principal 与绑定，记录 `bound`，不创建 Membership 或 Grant |
| 重复绑定同一活动代次 | 复用原 Principal；新命令记录 `unchanged`，原命令只返回历史回执 |
| Retire | 精确匹配 username、代次及预期 Principal；墓碑时间和 Principal 停用与 `retired` 审计同事务提交 |
| 已退役账号的新退役命令 | 记录 `unchanged`，不改墓碑时间，不影响同名新账号 |
| 同名重建 | 先退役旧绑定，使用另一代次和新的绑定命令；生成新 Principal，没有继承角色或部署 Grant |

旧绑定命令在后来退役后重放，返回的仍是原历史回执；这不是重新激活或当前身份状态证明。
重新 Bind 已退役/停用的原代次明确失败。没有按用户名继承、隐式恢复、无条件 upsert 或换 ID 自动重试。
若要给新主体授权，须另外提交 C1-C 策略命令，绑定本身不是授权。

## 最后管理者与并发读取

生命周期命令和 C1-C 策略命令共用 authority 写锁。退役会移除有效管理权，因此也检查
`LastAuthorityManager`，不只保护撤销 Grant 的路径。并发“管理者 A 退役、管理者 B 撤权”最多有
一个成功；另一个不能使实例失去全部有效管理者。成员暂停/空间归档不等于撤销独立部署权，归档后
仍可退役非最后管理者，但不能创建新绑定。

身份 Schema 2 的正常读取取得 authority 共享锁，再读取身份及最新审计快照。
仅锁子表不足以保护尚不存在的账号键：回归测试先复现绑定已开始而查询跳过写事务、提前报告不存在，
再通过统一锁顺序修复。相关[维护规则](../../.github/instructions/collaboration-store.instructions.md)
限定在此存储模块，不改变项目其他存储协议。

读取、有效权限判断和新命令会核对身份状态与审计头；缺失表/记录或未经审计的状态变化不能成为
缓存授权。事务内部在写状态之后、追加回执之前只做原始读回，避免把自己的未完成审计误判为损坏。
所有修改核对影响行数及最终状态，审计失败不能只留下身份变更；提交确认前不发布新 Principal。

这些保证限于遵循协议的准备层入口。特权 SQL/DDL 能绕过保护；追加式触发器不是防数据库所有者
篡改的签名账本。旧认证仍是独立运行路径，不能由这里推导全系统管理者或跨进程会话恢复保证。

## 失败、历史与接入门槛

历史读取仅供当前实例管理者，按 authority / Workspace / Principal 隔离，使用递增 sequence
游标和每页 1–100 条上限；游标是 JSON 安全正整数，允许空洞，不是 C3 事件提交偏移。
审计只追加，数据库拒绝 UPDATE、DELETE、TRUNCATE。角色、凭据及私人内容正文不进入身份审计。

错误、超时、取消或丢失响应仍可能与提交竞争，**不证明回滚**。保留准确命令 ID 和参数进行对账；
不要换一个账号代次或新命令去“补偿”。自行退役后原操作者已失权，需要另一名有效管理者核实结果。

原切片要求把账号创建/删除、Session 撤销和内部命令放进经过审查的权威事务路径；
来源与组合的受限后续进度见下文。不能只是按 username 拼表，也不能先修改旧表再尽力双写新表。
接触真实数据前仍需来源盘点、恢复验证、切换批准与回退边界；当前实现未完成这些生产门槛。

后续：0.4.6 收录的 [ADR-006 / C1-E](collaboration-auth-source.md)实现显式空源账号/会话适配器与
持久代次；同属 0.4.6 的 [C1-F](collaboration-session-commands.md)已组合来源核实与绑定/策略事务。
0.4.7 收录的 [C1-G](collaboration-account-deletion.md)新增受限管理删除/退役联动，同版还收录
[C1-H 改密](collaboration-password-change.md)、[C1-I 空实例引导](collaboration-bootstrap.md)、
[C1-J 本地初始化](collaboration-provisioning.md)和 [C1-K 只读对账](collaboration-reconciliation.md)内部准备层。
完整生命周期及真实数据切换仍待完成。下文验证保留 C1-D 原切片证据，不涵盖这些后续。

## 0.5.2 收录的时间读取防护

2026-10-07 后续在二进制解码前，通过 SQL 检查存储 `retired_at` 和身份审计 `recorded_at`。
NULL 退役时间仍合法，独立有效性标志阻止坏非空退役时间被当作活动身份；必填审计时间无法表示时
返回 `InvalidRecord`。身份读取、审计头/历史、重放、追加读回和对账保留原状态、锁序及事务规则，
不新增年代/排序策略、不改 Schema 或修复数据。详见[注册表时间合同](collaboration-reconciliation.md)，
下方 C1-D 结果仍是历史证据。

## 验证

[命令测试](../../engine/tests/collaboration_identity_audit_tdd.rs)、
[权限/生命周期测试](../../engine/tests/collaboration_identity_audit/permissions.rs)与
[故障注入](../../engine/tests/collaboration_identity_audit/faults.rs)新增 16 条真实 PostgreSQL 用例，
另有默认关闭连接池测试和扩展的 ID 契约测试。缺失接口、缺 CI 选择以及空键读取竞争均先观察失败后修复。

使用独立 PostgreSQL 16、私有 Unix Socket 和合成账号；16 条新用例及原有 46 条数据库回归全部通过。
覆盖升级/绑定并发、摘要冲突、同名重建、历史重放、失权后拒绝、最后管理者竞争、首次显式授权、
归档、分页/追加保护、旧 Session 不被误改、缺表/状态损坏、基线回滚、抑制或改写审计、延迟提交失败、
提交前取消及等待未完成绑定的读取。

[CI](../../.github/workflows/ci.yml)逐项校验准确测试名并执行默认忽略的数据库用例；
[CI 回归](../../scripts/tests/postgresCi.test.mjs)拒绝漏项或空选择。
在本地直接执行新增 CI 步骤的 16 条准确选择命令全部通过。完整默认 Rust 测试 **331 通过、142 忽略**；
其中本轮另行显式执行上述 62 条数据库用例，其余 80 条忽略测试未在本轮运行。
`quality-gate.sh --format-only` 通过：75 条公共回归、格式、文件长度、版本及文档同步、API/SDK
契约和工具检查均通过；版本仍为 0.4.4。
未运行本轮浏览器、真实内核/局域网或迁移恢复验收；旧发布报告和 API/SDK 冻结证据保持原样。

```bash
# 只能使用可丢弃测试库的 CYANREX_TEST_DATABASE_URL，不能使用部署 DATABASE_URL。
CARGO_BUILD_JOBS=2 cargo test --manifest-path engine/Cargo.toml --locked \
  --test collaboration_identity_audit_tdd --test collaboration_policy_audit_tdd \
  --test collaboration_identity_store_tdd --test collaboration_access_store_tdd \
  -- --include-ignored --test-threads=4
```

实现切片当时未升版、提交、推送或部署；现收录于源码版本 0.4.5，见
[发布验证](../../reports/releases/0.4.5/README.md)。C1-D 补齐的是注册表生命周期审计，不代表 C1 或在线认证切换完成。
