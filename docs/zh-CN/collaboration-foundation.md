# ADR-001：协作内核基础与旧系统基线

日期：**2026-09-27**。状态：**仅对 C-M1 类型契约与离线对比生效**。
源码基线：`feec8494189b6ee22f20bbd8737fe190df3b22f3`，产品版本 **0.4.3**。
收录于源码版本 **0.4.4**；下文冻结基线与历史验证记录保持不变。

这是[下一代架构书](next-architecture.md)的首个实现切片。产品继承现有版本序列；契约版本 `1`
不意味着产品重置到 `0.0.1`。部署中的实际行为仍以[当前架构](architecture.md)为准。

后续：[ADR-002 / C1-A](collaboration-identity-store.md)新增独立、显式的 PostgreSQL 身份注册表。
下文 C-M1 范围与验证保留首个切片的记录，不代表发生了在线迁移。

## 1. 本轮范围与证据

C0 源码盘点和 C-M1 现在有 Rust 类型、旧权限的离线预览及合成回归用例。本轮**不新增路由、表、
启动迁移、授权存储、登录逻辑、事件分发器或第二套写入者**。AppState 和现有路由守卫不调用新预览。
测试中的 ID 均为合成数据，不来自真实账号导出。

本次是**源码基线，不是在线部署盘点**。没有读取账号/配置秘密、学生提交或数据库内容。每个实例的
实际活动存储、降级历史、备份恢复、账号删除重建历史和内核资源，必须在 C-M2 或切换前另行核实。
因此不能将整个 C0 标为完成。

实现入口：

- [核心契约](../../engine/src/models/collaboration/mod.rs)：身份、引用、标量及事件信封。
- [旧权限预览](../../engine/src/services/legacy_workspace.rs)：纯函数式、固定范围的对比。
- [契约测试](../../engine/tests/collaboration_contract_tdd.rs)与
  [权限映射测试](../../engine/tests/legacy_workspace_projection_tdd.rs)。

## 2. 冻结的源码盘点

以下事实核对自基线提交。配置了数据库地址，不代表每个服务始终使用 PostgreSQL：降级按服务独立发生，
部分降级会持续到重启。

| 领域 | 现有身份/存储权威 | 新内核必须保留的边界 |
|---|---|---|
| 用户/会话 | SQL `users.username` 主键；会话引用 username，持久化 token 摘要；显式纯内存模式及部分认证降级仍存在 | 保留密码/TOTP/会话行为；不在每次登录时重新分配 Principal；持久退出/改密/删除确认不能被易失授权存储替代 |
| 教师/学生 | AuthService 根据部署账号、服务端教师/旧管理员名单决定权限；`admin` 兼容教师，不是另一层级 | 快照必须取服务端解析的角色，不接受浏览器字段或 Agent 自报身份 |
| 脚本 | username 所有权；SQL `user_scripts` 或实例数据目录中的用户 JSON；`default` 使用历史路径 `scripts/<username>.json`，其他实例使用 `scripts/<instance>/<username>.json` | 保持私有归属，记录所选来源；不按修改时间自动选择 SQL 或文件 |
| 尝试/评语 | SQL `learning_attempts` 或 `learning/<instance>/attempts.json`；当前评语记录 reviewer/comment/revision/time | 保留源码字节/摘要、原 ID、所有权、阶段和评语修订；不存在的历史评语版本保持未知 |
| 进度 | 按 username/lab 聚合；曾通过即完成，保留最早通过时间，后续失败不清除完成状态 | 教学验收策略保持，不只看最后一条 Run |
| 事件 | 按用户的有界 EventBus，可选异步 SQL 持久化和易失降级；旧公开事件没有持久 event ID | 作为遥测保留，不提升为可信审计事实，也不充当 outbox |
| 头文件/编译器 | 头文件、选择元数据、本地工具链和运行配置 | 单独盘点实际文件及工具链；宿主路径不是远端输入包 |
| Runner/Agent/模块 | 租约、节点注册表、任务队列和模块状态在内存中；内核挂载可能在请求结束后继续存在 | 重启不证明清理完成；运行资源归属与未来持久 Run 分开；Runner Agent 是 machine，不是 AI 参与者 |
| 发现/部署 | 绑定用户名的临时课堂邀请；原生 SSH 操作；实例内教师管理 | 发现不是认证；新空间 owner 不自动取得 SSH 或实例部署权限 |

固定版本证据：[认证][auth]、[路由层级][routes]、[会话守卫][guards]、[学习路由][learning]、
[进度聚合][progress]、[脚本][scripts]、[学习存储][learning-store]、[SQL 模板][migrations]、
[Runner 边界][runner]。完整旧路由/链路仍见[功能网络](functional-network.md)，不重写其历史指纹。

## 3. 决策：身份与作用域

1. `AuthorityId` 表示持久的 Cyanrex 实例权威，不等于已有的 `CYANREX_INSTANCE_ID` 清洗标签、主机名、
   文件路径或 HTTPS 对端身份凭证。C1 需要持久分配，并绑定经过核实的实例配置。
2. Principal、Workspace、Artifact、ArtifactRevision、Task、Run、Review、Event、Correlation
   使用不同 Rust ID 类型。草案 JSON 格式为非零、小写、带连字符 UUID；读取/反序列化不分配 ID，
   非规范拼写直接拒绝，不静默合并。
3. Principal 类型为 `human/agent/service/machine`，教师/学生不是身份类型。Principal 有
   active/disabled 状态，Workspace 有 active/archived 状态，Membership 有 active/suspended
   状态；角色引用表示空间内的预设。
4. `PrincipalRef` 必含 authority + principal ID，`WorkspaceRef` 必含 authority + workspace ID。
   Membership 的 principal ID 属于其 authority。跨实例联合需显式绑定，不能只比较局部 UUID/用户名。
5. `LegacyIdentityBinding` 把规范 username 与指定 Principal ID 分开。现有 username 是 3–64 字节
   的小写 ASCII 字母、数字或 `_-.`；导入不静默归一化名字，也不从名字重新计算身份 ID。
6. C1 必须持久化唯一的活动身份绑定，并在账号删除后退役它。同名重建不能继承旧 Principal 的授权。
   无法区分账号重建或无法确认原归属的数据需要对账；C-M1 尚未实现这一注册表，也不宣称能恢复所有
   历史归属。

## 4. 决策：旧角色迁移不得扩大隐私可见性

预览只接收**可信、已解析的**角色、身份及资源归属，不自行读取环境变量或会话。固定 legacy Workspace
来自操作者保存的映射，不接受客户端 workspace ID 改变作用域。

| 原主体/动作 | 自己的旧内容 | 其他学生的旧内容 | 其他空间/实例 |
|---|---|---|---|
| 任意已认证角色：读私人脚本/Artifact | 允许 | 拒绝 | 拒绝 |
| 任意已认证角色：经所有者端点恢复 attempt | 允许 | 拒绝，教师也不例外 | 拒绝 |
| 学生：教师审阅操作 | 拒绝 | 拒绝 | 拒绝 |
| 教师/旧管理员：教师审阅操作 | 仅目标是学生 attempt 时适用 | 允许学生 attempt，不包括任意私人 Artifact 或教师/管理员 attempt | 拒绝 |
| 教师/旧管理员：部署管理 | 原实例显式 Grant | 不向学生授予 | 其他实例拒绝 |

`student` 映射为 `cyanrex.teaching.learner`；`teacher` 和旧 `admin` 映射为
`cyanrex.teaching.teacher`，**另附原实例部署 Grant**。单人部署教师仍可教学和管理，不必切换账号。
只在新空间授予 `cyanrex.workspace.owner` 不会产生部署授权。

预览只对比四种迁移敏感动作。角色名、Membership/Grant JSON 或 `permits` 返回 true 均不是凭证。
通用资源授权、撤销、停用主体、归档空间、动态角色变化和策略存储属于 C1；真实请求仍需认证、实时
角色/资源查询、CSRF 和适用的密码/TOTP 检查。不得将旧预览缓存为在线授权。

测试覆盖教师/旧管理员/学生映射、恢复与私人内容的所有者限制、仅学生提交可供教师审阅、他人复用预览、
其他 authority 下相同 ID、其他 Workspace 及错误动作/资源组合。它们验证离线映射，**不证明在线多空间
隔离已实现**。

## 5. 决策：固定版本引用与事件记录

草案沿用 Rust API 的 `snake_case` JSON 风格，尚未接入 OpenAPI/JavaScript SDK；未来公开 API
版本需另行定案。

- `ArtifactRef` 必须指定空间作用域、Artifact ID、**revision ID** 和 64 位小写 SHA-256 十六进制摘要，
  不提供浮动 `latest`。同摘要不共享 ACL；解析仅校验结构，不证明内容摘要、存在性、归属或权限。
  发布服务还必须核对内容及授权。
- `EventEnvelope` 包含独立 `schema_version=1`、event ID、空间、有类型的聚合引用、聚合修订、
  可未知的操作者/发生时间、必填记录时间、带命名空间的事件类型、correlation ID、可选 causation ID
  与固定版本 payload 引用。历史未知事实保持未知，记录时间由记录者提供，不在解析时编造。
- 聚合的作用域由外层空间指定。应用服务必须核对引用一致性、操作者权限和 payload 引用访问权后再接收
  记录；结构解析不做这些检查，也不允许跨空间读取。
- 修订号为不超过 `9007199254740991` 的正整数，确保 JavaScript 精确表达。角色/事件名是至多
  128 字节的带命名空间小写标识，不是命令。
- 草案 DTO 拒绝未知字段、未知枚举和不支持的信封版本；扩展需要显式版本/Schema 决策，不能在
  解析后重新保存时静默丢弃。现有公开 API 对未知字段的处理不变。
- 旧遥测 `Event` 不变，不能直接解析为新信封。同内容历史遥测也必须保留独立来源/ID，摘要相同
  不证明是同一事件；本轮未提供遥测到业务事件的转换器。

本轮没有实现 outbox、事件排序、流偏移、重放、Run 恢复或可信审计。未来业务状态和 outbox 必须同事务
提交；已提交流的 offset 与 event ID/聚合修订分开，任何字段都不单独保证 exactly-once 执行或取消回滚。

## 6. API 基线与单一写入权威

以下保护基线保持逐字节不变：

| 基线 | C0 时 SHA-256 |
|---|---|
| [OpenAPI 兼容基线](../../sdk-js/compatibility/openapi-baseline.json) | `59626ac51895120c9111683f2d4a9d95d03334edd250c2e8f72413fc448cfca6` |
| [SDK 公开成员基线](../../sdk-js/compatibility/public-surface.json) | `5ab9e5308b06c25b8c67b141301a62281f48a2fe845249d7fe34ca1bf41bcab9` |

保留旧 namespace、operationId、返回形式及 `staff/admin` API 分类名。[SDK 稳定性政策](../../sdk-js/STABILITY.md)
明确所有 minor 线的 patch 都遵循 additive-only；同时统一“保留完整后续 minor”的文字和例子：
`0.4.x` 弃用后，**整个 `0.5.x` 仍保留**，最早在有迁移说明、经过明确审查的 `0.6.0` 删除。
不重生成兼容基线来掩盖失败。

现有服务继续是唯一写入者，不引入双写或影子调度器。未来新核心不能确认持久写入时必须拒绝可靠承诺。
首次切换优先显式维护窗口：暂停旧写、补齐最终差异、drain/核对运行资源，再让新旧 API 都经过同一
权威核心。新写入前可切回旧服务，新写入后必须对账并核对运行所有权。删旧表、提权和部署另行决策。

## 7. 验证与下一关

先加入测试，观察缺少新模块而失败，再实现通过 9 项契约测试和 7 项映射测试。ID 类型区分另有
compile-fail 文档测试。标准后端门禁会与旧回归一起执行：

```bash
CARGO_BUILD_JOBS=2 ./scripts/quality-gate.sh --backend-only
```

测试使用可丢弃的 `CYANREX_DATA_DIR`，不继承生产 `DATABASE_URL` 或部署凭据。不得在生产库运行
默认忽略的 PostgreSQL 集成测试。

本轮通过完整默认 Rust 测试（含 compile-fail 文档测试）、公共预检/格式检查及其中 71 项工具测试、
26 项前端权限/流程测试，以及 SDK 的 13 项运行时、类型检查和 3 项包测试。OpenAPI/SDK 生成与
兼容检查保持同步。本轮未运行可选 PostgreSQL 集成、真实内核/局域网验收、浏览器端到端测试或联网
依赖审计，不宣称部署验收通过。

| 关卡 | 本轮交付 / 剩余事项 |
|---|---|
| C0 源码基线 | 已记录源码层存储/权限/API 盘点及决策 |
| C0 部署基线 | **待完成**：核实真实实例、持久/降级来源、恢复演练后的备份和运行资源归属 |
| C-M1 | 已有类型契约及离线角色映射；不包含在线身份注册表或授权切换 |
| C1 | [C1-A](collaboration-identity-store.md)身份映射、[C1-B](collaboration-access-store.md)成员/部署策略及 0.4.5 的 [C1-C](collaboration-policy-audit.md)策略命令/审计、[C1-D](collaboration-identity-lifecycle.md)带操作者的绑定/退役准备层已实现；0.4.6 的 [C1-E](collaboration-auth-source.md)新增持久账号/会话来源，[C1-F](collaboration-session-commands.md)组合会话授权的绑定/策略事务；完整账号生命周期、通用 Grant 和在线受保护操作接入仍待完成 |
| C-M2 | 待实现夹具驱动的离线 attempt 转换及 ID/摘要/所有者/进度/评语对账；先合成数据，再经批准的隔离副本 |

本轮基础切片不自动创建新版本、提交、标签或部署。

[auth]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/auth_service.rs
[routes]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/application.rs
[guards]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/routes/auth_session.rs
[learning]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/routes/learning.rs
[progress]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/learning_store/queries.rs
[scripts]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/script_store.rs
[learning-store]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/learning_store.rs
[migrations]: https://github.com/Team-silvortex/cyanrex-lab/tree/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/migrations
[runner]: https://github.com/Team-silvortex/cyanrex-lab/blob/feec8494189b6ee22f20bbd8737fe190df3b22f3/engine/src/services/runner_driver.rs
