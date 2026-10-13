# scripts

Cyanrex 的开发、验证、运维和分发工具，覆盖现有教学运行时及显式协作准备层。
测试运行器或初始化工具不会在既有部署中激活通用平台。

| 需要 | 从这里开始 |
|---|---|
| 按模块和边界选择测试 | [当前测试指南](../docs/zh-CN/testing-guide.md) |
| 区分现用服务和准备层 | [平台关系图](../docs/zh-CN/platform-network.md) |
| 初始化或检查新的协作权威来源 | [原生初始化指南](../docs/zh-CN/collaboration-provisioning.md) |
| 通过 SSH 部署已有软件包 | [课堂连接](../docs/zh-CN/classroom-connection.md) |
| 区分当前结果与旧快照 | [项目进度](../docs/zh-CN/project-status.md)、[开发历史](../docs/zh-CN/development-history.md) |

数据库故障运行器要求可丢弃的合成数据；特权冒烟测试要求明确可丢弃的宿主机。
不得把运行数据、`.run/` 配置、备份或回滚镜像改作测试夹具或构建缓存清理目标。
下面的脚本目录保留各工具自己的前置条件。

- `check-instance-conflicts.sh`：多实例启动预检器：
  - 验证请求的 Engine／frontend／Postgres 端口空闲
  - 检测具有相同 instance ID 的 Compose project 是否已运行
  - 通过 `--allow-existing-running` 支持重新连接流程
  - 预检重复端口和 project ID，支持并发启动安全
- `start-lock.sh`：由 `start.sh` 加载的共享运行时 helper，为同一实例的 Compose 操作
  （start/stop/status/logs）提供互斥。
- `quality-gate.sh`：用于提交／CI 检查的单命令质量门禁。
  - 每种模式还检查文件行数、版本／OpenAPI 漂移及仓库工具
  - 默认：运行后端、前端和 JavaScript SDK 检查
  - `--backend-only`：公共预检及后端检查
  - `--frontend-only`：公共预检及前端检查
  - `--sdk-only`：公共预检及 JavaScript SDK 构建、测试和审计
  - `--format-only`：公共预检及 Rust 格式检查
  - `--permissions-only`：公共预检及权限回归检查
  - `--security`：在默认后端／前端／SDK 检查中加入安全审计
  - `--security-only`：公共预检及安全审计检查
  - `--no-npm-install`：前端和 SDK 检查时跳过 `npm ci`
- `test-prepared-resource-sql.sh`：对显式可丢弃 `CYANREX_TEST_DATABASE_URL` 运行两条精确共享资源
  SQL helper 用例。检查普通表 predicate／解码顺序，以及事务本地隔离／期限和单连接
  rollback/drop 恢复。不安装 schema、不连接准备层存储到现用应用，也不代替各 store／Session 套件。
- `test-task-storage.sh`：显式运行全部十五条 Task instance PostgreSQL 用例，要求可丢弃数据库的
  `CYANREX_TEST_DATABASE_URL`。每个精确测试名必须先确认存在。CI 使用相同脚本；测试自行
  创建 schema 并注入故障，绝不能指向已部署数据库。`tests/postgresCi.test.mjs` 检查显式清单覆盖全部用例。
- `test-session-task-content.sh`：运行三十三条精确 schema-3 current-Session 内容 PostgreSQL 用例，
  覆盖显式 manual/catalogue 模式、组合 bound/unbound 元数据、完整定义准入、有序文本、
  私人所有权、元数据编辑／移除、旧／新写后字节检查、namespace pin、生命周期过期／撤销、
  commit／取消及竞争编辑。只要求显式可丢弃数据库，不暴露公共命令或连接浏览器编辑器。
- `test-session-task-draft-publication.sh`：对可丢弃 `CYANREX_TEST_DATABASE_URL` 显式选择
  create-only 草稿 stepper 的 PostgreSQL 用例。检查已确认部分发布、停止的不确定步骤、取消、
  当前权威和最终存储字节验证。进度只是调用者持有的内存，不是持久回执或浏览器保存 API。
  `tests/sessionTaskDraftPublication.test.mjs` 守卫完整选择及仅内部接线。
- `test-session-catalog-draft-publication.sh`：对显式可丢弃 `CYANREX_TEST_DATABASE_URL` 和私人文件
  选择八条精确独立 catalogue 发布 SQL 用例（行为两条、故障三条、生命周期一条、等待两条）。
  覆盖组合 package/version/binding 元数据、单次写入步骤、当前权威、有效故障计数、完整
  definition/manifest 确认、不采用已有结果的冲突、实际观察的取消／过期等待和保留的不确定目标。
  七张 Task/Artifact 表及文件快照区分不变拒绝与失败第二次 Artifact 发布留下的精确孤立 blob；
  typed error 不证明回滚，也不保证所有 auth 表均不变。四条默认准备用例和八条内部
  receipt/cancellation 用例是独立清单，不是执行证据。不启用 manual checkpoint/journal 转换、
  重试、恢复、provider assessment、公共 HTTP/SDK/browser 或现用部署。
  此发布运行器不把 catalogue 尝试转换为 manual observation/recovery 类型；独立 C2-AA catalogue
  观察另行选择。
- `test-session-catalog-draft-observation.sh`：对显式可丢弃 `CYANREX_TEST_DATABASE_URL` 选择八条
  精确独立 catalogue 观察 SQL 用例（行为三条、故障两条、生命周期一条、并发两条）。
  覆盖 original-Session/state 准入、完整冻结定义和有序字节、合法变化与 typed same-pin/manual/binding
  损坏的区别、scope 不可见性、当前权威、实际观察的取消／过期等待及另行显式等待的创建者。
  保留快照使用选定 Source/Artifact/Task 表 MD5、文件 SHA-256／长度／mode 和 root identity/mode，
  不是 raw-row 相等，也不覆盖全部部署 schema。有效故障计数和独立测量的 rollback/orphan 数量
  不使 `NotVisible` 成为回滚证明。四条 public default 和六条内部 comparison/admission 用例是
  独立清单，不是 SQL 执行证据。报告绝不采用进度、启用重试、转换 manual/recovery 类型，
  或连接 HTTP/browser 保存、provider、issuer、迁移或部署。
- `test-session-task-draft-observation.sh`：对显式可丢弃数据库运行十二条精确当前 unconfirmed-target
  观察用例。检查匹配或变化的记录、按所有者过滤的可见性、原 Session 授权、等待中的写入者、
  过期、取消和损坏字节。观察对业务只读，绝不确认、重试或续接已停止发布。
  `tests/sessionTaskDraftObservation.test.mjs` 守卫读取器、仅内部接线及 CI 清单。
- `test-session-task-draft-inspection.sh`：对显式可丢弃数据库选择十二条精确 parsed-checkpoint
  检查用例。当前 Session 和可信 workspace 授权一次读取；报告元数据不恢复尝试，也不证明
  原写入者或 commit。检查 metadata/text profile、所有者隔离、typed fault、过期、等待中的
  写入者和取消。`tests/sessionTaskDraftInspection.test.mjs` 守卫独立内部读取器和完整 CI 清单。
- `test-session-task-draft-intent.sh`：对显式可丢弃数据库选择十二条精确不可变 intent 登记和
  current-Session 读取用例。检查 original-attempt 准入、私人所有权、journal installation identity、
  冲突、损坏、过期及取消。登记不发布资源，也不对既有 draft stepper 准入、续接或重试；
  `tests/sessionTaskDraftIntent.test.mjs` 守卫该边界及精确 CI 选择。
- `test-session-task-draft-dispatch.sh`：对显式可丢弃数据库选择十四条精确两阶段 journaled dispatch
  用例。覆盖新 schema-2 准入、先写后分派、两事务中的当前所有权／Session 检查、原子 marker
  和资源 SQL、完整 prefix／table pin、故障及取消。不启用 recovery/retry 或 public/browser API。
  `tests/sessionTaskDraftDispatch.test.mjs` 守卫封闭边界和完整 CI 选择。
- `test-session-task-draft-journal-inspection.sh`：对显式可丢弃数据库选择十条精确 schema-2 journal
  检查用例。检查当前 owner/account 可见性、包含最终 Task 的完整记录前缀、schema/pin 损坏、
  过期及取消。不读资源／文件、不恢复 wrapper，也不授予重试权。
  `tests/sessionTaskDraftJournalInspection.test.mjs` 守卫仅内部读取器及精确 CI 选择；清单不表示执行通过。
- `test-session-task-draft-journal-observation.sh`：对显式可丢弃数据库选择十二条精确 recorded-step
  resource 观察用例。检查 ordinal／visibility 区分、借用的同事务 Artifact/Task 读取、完整原
  journal／nonce／first pin、当前授权、故障及取消。无正文输出仍读文件，但绝不返回回执或
  retry/recovery permit。`tests/sessionTaskDraftJournalObservation.test.mjs` 守卫私人组合和精确
  CI 清单；选择用例本身不是通过证据。
- `test-durable-session-boundary.sh`：运行十七条精确 Source-only Session PostgreSQL 用例。
  覆盖 namespace/table identity、过滤或不兼容存储、写后 metadata/redirect 故障、两事务登录
  pin 及 logout/expiry 顺序。要求显式可丢弃数据库，而非 registry 或现用部署。
  `tests/durableSessionBoundaryCi.test.mjs` 守卫精确选择和 CI 接线。2026-10-07 的三条用例
  增加非法／边界 Session 过期、login-writeback 和密码修改 guard 覆盖；选择数量不是新通过结果或公共 issuer。
- `test-durable-reconciliation.sh`：选择十七条精确只读生命周期 PostgreSQL 用例，执行前检查
  每个名称。原十二条加入 2026-10-07 的两条 Session-expiry 和三条 registry-timestamp 用例，
  覆盖非法／有限时间、精确错误阶段和被拒 Session 命令。Source expiry 仍是 SQL-only boolean，
  不披露原始 timestamp。要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`；不是修复、部署数据
  验收或所有 timestamp 均安全的检查。结果仍按原日期记在项目进度中。
- `test-registry-timestamps.sh`：跨身份存储、身份审计、策略审计及对账选择十条精确 SQL 用例。
  覆盖 nullable-retirement 有效性、强制审计时间、有限边界、旧 history/replay 及 append readback。
  不接受参数，要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`。用例也属于各完整套件，不得
  重复计数；运行器不修复数据，也不宣称全部 timestamp 安全。
- `test-durable-session-cleanup.sh`：运行显式 expired Session 维护、有界 batch 和 selected-row/source
  确认的精确库级 PostgreSQL 回归。要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`；故障夹具
  绝不能指向已部署 Source。不是运维清理命令、自动计划或 Session 存储全局有界证明。
  执行结果按原日期记在项目进度中。
- `test-durable-password-profile.sh`：运行六条精确 prepared-password PostgreSQL 用例：非法存储
  profile、既有 Session 不变、轮换失败、显式写入者兼容及写后 trigger 回滚。要求显式可丢弃
  数据库；不切换现用认证或自动迁移凭据。`tests/durablePasswordProfileCi.test.mjs` 守卫 CI 选择。
- `test-durable-login-password.sh`：运行缺失账户 prepared-login 密码工作及 database/worker
  边界的精确库级 PostgreSQL 回归。要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`；合成夹具
  不得使用真实凭据或已部署 Source。检查共享验证路径，不证明恒定时间登录、完整防账户枚举
  或公共 Session 签发。执行结果按原日期记在项目进度中。
- `test-durable-otp-freshness.sh`：在库测试二进制中，通过每个 Source 的私人测试时钟运行七条
  精确 PostgreSQL 用例：login/rotation 写入者及 trigger 等待、回滚、密码 executor 被保持时
  释放快照、不变时钟成功，以及 freshness 与组合 consumption 的区别。要求显式可丢弃数据库，
  不改变机器时钟或现用认证。Source 和 CI 守卫位于 `tests/durableOtpFreshness*.test.mjs`。
- `test-durable-otp-consumption.sh`：十一条精确库级 PostgreSQL 用例验证 schema-2 原子 OTP
  consumption：独立 Source、reopen/logout、轮换竞争、精确账户 incarnation、version/shape
  拒绝、写入故障、deferred failure、取消和真实 counter 碰撞。使用私人测试时钟及合成账户；
  不公开时钟覆盖，也不切换 legacy auth。
- `test-durable-otp-schema.sh`：四条精确 PostgreSQL 用例验证真实 schema-1 拒绝、不兼容的
  schema-2 counter shape、新 register/bootstrap watermark 及注入初始化故障。要求具有夹具
  DDL 权限的可丢弃数据库，不迁移既有数据。两运行器的精确选择守卫位于 `tests/durableOtp*Ci.test.mjs`。
- `test-platform-task-content-http.sh`：运行十三条精确独立 HTTP-to-C2-M PostgreSQL 用例。
  覆盖私人快照、无写入传输准入、锁等待后的当前授权、精确文本、事务故障、并发编辑，以及
  延续至 response-body 完成的共享准入。要求显式可丢弃数据库及私人临时文件。
  路由未挂载到现用应用；测试不打开 HTTP listener，不证明浏览器、TLS/proxy、公共内容发布或部署验收。
- `test-platform-text-artifact-http.sh`：十条精确独立 text-Artifact HTTP PostgreSQL 用例。
  覆盖 current-Session 发布、精确历史读取、文本投影、私人错误、body/slot 限额和调用者
  独立 publication-reference-to-Task 夹具序列。要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`
  和私人临时文件。不表示 listener、自动 router merge、whole-draft transaction、浏览器、journal
  工作流或部署验收。
- `test-platform-catalog-task-content-http.sh`：对显式可丢弃 `CYANREX_TEST_DATABASE_URL` 和私人文件
  运行八条精确独立 catalogue Task-content HTTP PostgreSQL 用例。覆盖可信 exact-reference
  组合、四命令中的完整元数据和 bound/unbound identity、manual/owner 隐私、旧／新字节、
  当前 Session/lifecycle、实际观察的等待、write/commit 故障及共享 response-body/cancellation
  容量。拒绝快照覆盖七张 Task/Artifact 资源表及全部私人目录文件名／字节，不覆盖每张
  auth/identity/audit 表。28 条默认用例和三条 `tests/catalogTaskContentHttp.test.mjs` 守卫
  是独立清单，不是执行证据。不启用 live mount、listener、catalogue discovery、client-selected
  mode、provider assessment、自动 router merge、journal P–W 集成或浏览器保存。
- `test-task-content-storage.sh`：运行二十一条精确 schema-3 content-storage PostgreSQL 用例，
  包括仅元数据编辑、schema-2 拒绝、完整 outbox 快照和注入故障。要求可丢弃数据库；可信
  store 套件不证明 Session 或浏览器保存。CI 和 `tests/postgresCi.test.mjs` 在执行前核验完整精确选择。
- `test-artifact-storage.sh`：运行全部十七条 immutable-content PostgreSQL 用例，包括私人文件
  故障和 Task revision pin。要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`；私人文件夹具使用
  临时目录。CI 运行存储脚本并验证精确用例选择。已部署数据库或运行内容目录均不适用于这些测试。
- `test-review-storage.sh`：运行全部十七条 Review PostgreSQL 用例，包括不可变评语历史、
  human/rule 分离、rollback/corruption 及非教学 Artifact/Task/Review 夹具。要求显式可丢弃
  `CYANREX_TEST_DATABASE_URL`；CI 验证并执行每条精确用例。夹具中的审阅者归属是可信输入，
  不是 Session 授权测试。
- `test-session-task-storage.sh`：运行全部十七条 Session-authorized private manual Task 用例，
  包括所有权、退役／重建、logout/expiry 顺序、事务故障及 namespace 恢复。要求显式可丢弃
  `CYANREX_TEST_DATABASE_URL`；CI 验证并执行每条精确用例。它们是合成内部授权夹具，
  不是已部署工作流测试。
- `test-session-artifact-storage.sh`：运行全部十八条 Session-authorized private Artifact 用例，
  包括精确内容访问、revision lock、账户生命周期和 file/database 失败边界。要求显式可丢弃
  `CYANREX_TEST_DATABASE_URL` 及私人临时文件；CI 核验完整清单并执行每条精确用例。
  这些夹具保留的文件不是现用数据。
- `test-session-task-inputs.sh`：运行全部二十条 Session-authorized Task/Artifact input 用例，
  包括固定内容、私人所有权、多 namespace 检查、写后验证及并发 logout/expiry/revocation。
  要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`；CI 检查并执行每条精确用例。
  文件损坏和数据库故障夹具均为合成数据。
- `test-session-review-storage.sh`：运行全部十八条 Session-authorized private human Review 用例，
  包括精确 target/evidence、policy/source 限制、namespace pin、写后检查、生命周期撤销、
  expiry/logout 顺序及取消。要求显式可丢弃 `CYANREX_TEST_DATABASE_URL`；运行器执行前检查
  每个精确名称。这些夹具不证明跨用户审阅权、策略执行、历史 Session 作者归属或 Task 接受。
  执行结果另记于项目进度。
- `test-session-catalog-tasks.sh`：运行全部二十条 Session-authorized catalogue Task 用例，包括
  精确 bound/unbound definition 快照、单一／组合元数据无 provider 构造、provider 独立性、
  零输入元数据检查、私人输入、namespace/write 故障及 Session 顺序。要求显式可丢弃
  `CYANREX_TEST_DATABASE_URL`；运行器执行前检查每个精确名称。合成夹具验证元数据准入，
  不验证 provider assessment、typed evidence 或 Task 接受。执行结果仍在项目进度中。
- `tests/taskCatalogComposition.test.mjs`：守卫显式 metadata-only 组合、复制前有界输入／
  展平数量、exact-reference 重叠拒绝，以及不变的 typed、manual 和 public 构造边界。
  源码检查不运行纯 Rust 或 Session 组合 SQL 用例，不证明 package authentication、
  provider assessment 或 public/browser 验收。
- `tests/taskAssessmentBinding.test.mjs`：守卫 nullable definition 字段与必需 assessment-result
  pin／manual identity 的区分、精确 Session SQL 清单，以及 metadata-only 路径中没有
  assessment dispatch 或 public/manual-publication 提升。源码守卫不运行这些 Rust/SQL 用例，
  也不证明旧二进制读取新的 double-null 记录、迁移、公共命令、自动 Review 签发或 Task 接受。
- `tests/catalogTaskContentHttp.test.mjs`：三条结构守卫检查可信 typed catalogue state、严格流式
  reference object、共享 security/admission/Session 命令接线、不变的 manual/publication/API/SDK/browser
  边界及精确 28-default／八条 SQL 清单。不执行这些 Rust/SQL 用例，不评估 definition，
  也不证明已挂载／已部署工作流。
- `tests/sessionCatalogDraftPublication.test.mjs`：六条结构守卫检查独立 opaque typed catalogue
  attempt、分配前冻结 definition、token domain、每次 advance 一条既有 Session 写命令、
  完整 `Some` 确认、两个共享纯 helper 和不变的 manual/recovery 接线。
  检查四条 default／八条 internal／八条 exact SQL 清单及 runner/CI 选择，不验证 Rust/SQL
  执行、provider assessment 或已部署发布工作流。
- `tests/sessionCatalogDraftObservation.test.mjs`：六条结构守卫检查不可变 catalogue observer、
  original-token/state 准入、委托给既有 current-Session 读取器、精确完整 definition／有序文本
  比较、无正文 DTO 及不变的 manual/journal/public 接线。检查四条 default／六条 internal／
  八条 exact SQL 清单及 runner/CI 选择，不验证数据库／文件执行、历史 commit 或 rollback、
  已采用的确认或恢复权。
- `test-session-task-revisions.sh`：运行全部十七条 Session-authorized Draft input replacement
  用例。覆盖精确旧／新内容、有序 32+32 references、schema 2/1 兼容、catalogue policy、
  私人所有权、过期保存、trigger/commit 故障、过期及取消。要求显式可丢弃
  `CYANREX_TEST_DATABASE_URL`；执行前检查每条精确测试存在。Artifact 发布独立进行；
  这些合成测试绝不针对已部署数据。
- `check-npm-audit.mjs <package-directory>`：共享本地／CI 生产依赖审计，沿用 moderate severity
  阈值。只重试临时连接错误、rate limit/server failure 及 npm 已退役 Quick fallback，
  最多三次尝试，1s/2s backoff，每次限 60s。漏洞报告绝不重试；认证、证书、锁文件、
  非法报告和持续网络错误仍让门禁失败。不安装或修复依赖。
- `tests/npmAudit.test.mjs`：审计范围、期限、重试和 fail-closed 回归，纳入每种质量门禁模式。
  loopback mock 还运行真实 npm CLI，不联系外部 registry。
- `check-security-audit.sh`：按受跟踪的例外清单审计 Rust 依赖。
  - 读取 `scripts/security-audit-exceptions.json`
  - 报告显式例外，遇到未批准 advisory 则失败
- `check-version-sync.sh`：保持 Engine、frontend、SDK、锁文件和面向发布的文档语义版本一致，
  包括生成的 OpenAPI 文档及当前 `CHANGELOG.md` release heading。
  `engine/Cargo.toml` 是规范来源。
- `release-preflight.mjs`：打 tag 前验证干净、已提交的候选；或核验既有带注释版本 tag 与全部
  同步版本字段、注明日期的 changelog 条目及 release 文档一致。
- `tests/releasePreflight.test.mjs`：参数、漂移报告、clean tree、轻量／带注释 Tag 和不可变
  历史树的发布回归。
- `release-metadata.mjs`：写入机器可读的离线包 provenance，包含 Git source state、image
  reference/content ID/build mode 及流式 image-archive SHA-256；`--verify` 检查解包后的软件包，
  `--expect-*` 标志将 Tag 候选绑定到版本、revision、source state 和 image mode。
- `tests/releaseMetadata.test.mjs`：元数据验证、Git-state 检测、路径隐私、篡改和 checksum
  binding 回归。
- `generate-openapi.mjs`：从已注册 Axum 路由、访问规则及维护的 component schema 生成
  `engine/openapi/openapi.json`；`--check` 拒绝过期输出。
- `openapi-contract.mjs`：检查精确 Engine/OpenAPI 路由及访问层级一致性、预期 Engine/SDK
  覆盖、operation metadata、版本同步和 schema reference。
- `check-functional-network.mjs`：只读验证注明日期的[功能网络盘点](../docs/zh-CN/functional-network.md)。
  检查唯一 API 所有权、module/workflow 连接、全部当前页面、catalog ID 和引用的源码指纹。
  审阅源码快照后显式运行；漂移表示盘点需要复查，不代表运行时验收通过或失败。
  它绝不刷新 hash 或改变服务。
- `tests/functionalNetwork.test.mjs`：盘点及漂移检查器的正／负向回归，纳入公共工具测试。
  不动态执行产品工作流。
- `check-capability-tensor.mjs`：只读验证当前已审阅的[架构／能力／实现／成熟度盘点](../docs/zh-CN/capability-maturity.md)。
  检查稀疏坐标、四个独立评分、evidence/path reference 和有序边；不运行被引用测试，也不
  认证部署。导出的 `renderTensorRows` 生成双语评分表。
  `tests/capabilityTensor*.test.mjs` 作为公共 test glob 的一部分验证约束、仓库盘点和两个
  阅读视图。历史 functional-network 指纹和证据日期保持不变。
- `tests/openapiContract.test.mjs`：契约检查中的 parser 和漂移报告回归。
- `generate-sdk-types.mjs`：将 OpenAPI component schema 转换为已提交
  `sdk-js/src/generated/openapi.ts` type map；`--check` 拒绝过期 SDK model。
- `tests/sdkTypeGenerator.test.mjs`：JSON Schema-to-TypeScript 渲染回归。
- `generate-sdk-operations.mjs`：生成 65 个 browser-facing SDK operation 的 input、response、
  access/transport metadata 和 operationId registry，排除签名 Runner Agent 协议。
- `tests/sdkOperationGenerator.test.mjs`：operation transport、parameter、response 和 Agent
  boundary 生成回归。
- `generate-agent-tools.mjs`：从 OpenAPI 生成 28 个明确审阅的 JSON tool definition，展开本地
  schema 并拒绝 path/method/access 漂移；`--check` 拒绝过期输出。
  不授予模型权限，也不自动导出每个 SDK operation。
- `tests/agentToolGenerator.test.mjs` 和 `tests/aiAgentSettingsContract.test.mjs`：catalogue 隔离、
  schema/reference guard，以及仅教师可用的非秘密配置契约回归。
- `sdk-surface-compatibility.mjs`：检查固定的 additive-only `CyanrexClient` namespace/method
  基线，拒绝删除或重命名公开 member path；`--write-baseline` 只用于经审阅的操作。
- `tests/sdkSurfaceCompatibility.test.mjs`：嵌套 surface 提取、删除、追加增长及确定性基线回归。
- `api-compatibility.mjs`：比较当前 Engine 契约和固定 SDK 基线，拒绝移除 operation、访问变化、
  input 收窄或成功 response 削弱。
  - `--write-baseline` 在批准的兼容性重置后有意替换基线
  - 常规质量检查绝不改写基线
- `tests/apiCompatibility.test.mjs`：request、response、access、enum 和 nullability 的破坏性／
  追加式兼容回归。
- `clean-macos-metadata.mjs`：清理 `.DS_Store` 和 AppleDouble `._*` sidecar，避免通过 macOS
  文件系统复制仓库后被 Next.js 误当成源码。仓库级 CLI 包装
  `frontend/scripts/clean-macos-metadata.mjs`；frontend build/dev hook 使用上下文本地实现，
  因此 Docker 构建不要求父目录脚本。
- `tests/dockerBuildInputs.test.mjs`：检查 Engine compile-time asset allowance/copy，在独立的
  frontend-only 上下文运行 frontend prebuild hook；纳入每种质量门禁模式。
- `frontend/scripts/sync-course-docs.mjs`：从权威 `docs/` 树替换前端已提交课程副本；`--check`
  拒绝镜像副本中缺失、改变或多出的文件，纳入每种质量门禁模式。
- `debug-system.sh`：为本地故障排查收集环境／运行时诊断。
  - 输出工具链版本、kernel capability 状态、Compose backend 状态和端口检查
  - 用于 `./start.sh diagnose`
- `runner-agent.sh`：准备和管理可选的无特权 Compose compiler Agent。
  - `start`、`stop`、`status`、`logs` 和只处理秘密的 `prepare` 操作
  - 不输出 token 地创建缺失 bootstrap token，并挂载 mode-0600 Docker Secret
- `runner-agent-smoke.sh`：加载运行时配置，优先用原生 Rust 验收客户端进行 TOTP 认证、Agent
  发现、真实 compile check、结果验证和失败清理。CLI 不可用时，shell/Python 实现仍作兼容回退。
- `test-runner-agent-tools.sh`：不启动 Docker，验证秘密生成、mode 0600、稳定复用、不披露、
  安全 Agent ID 和 shell syntax。
- `live-kernel-smoke.sh`：在可丢弃的特权 Linux 栈上，优先用原生 Rust 客户端认证，要求初始
  attachment 集合为空，运行内置 Aya `sched_switch` ring-buffer 模板，要求唯一绑定的 kernel
  event，detach 精确 pin 并拒绝残留。shell/Python 实现仍作兼容回退。
  `CYANREX_KERNEL_SMOKE_REPORT` 可选地写入绑定打包 release metadata 和运行环境的原子证据。
- `../engine/src/bin/cyanrex-release/`：原生 Rust 发布 CLI，处理完整 Tag 候选、live-kernel
  evidence、验证后的解包、解包后 package/image 验证，以及 health、live-kernel 和 Runner
  Agent 验收。Evidence 接受 legacy v1 report，新 report 使用自包含 v2 event binding。
- `live-kernel-evidence.py`：源码 checkout 中为 parity regression 保留的兼容实现；
  新离线包只交付原生 release CLI。
- `release-tool-image.sh`：从已构建 Engine image 安全导出 `cyanrex-release` 到离线包，并清理
  自己的临时 container。
- `test-live-kernel-smoke.sh`：模拟成功、stale-event 和 missing-event 路径，验证生成的及 legacy
  evidence，运行 native-wrapper 选择，拒绝 tampering/metadata mismatch/duplicate key，
  并要求完成清理，不向本地内核加载程序。
- `release-candidate.py`：四文件 Tag artifact 验证的兼容实现，为旧环境及 parity regression 保留。
- `release-package.py`：验证和安全解包 archive/checksum pair 的兼容实现，供旧环境迁移至
  `cyanrex-release package extract` 期间使用。
- `test-release-candidate.sh`：构建最小有效候选，拒绝 checksum tampering、evidence substitution、
  package member 修改、traversal、symbolic link、含糊 archive 和 output replacement，
  同时运行 candidate 与 package-only extraction 路径。

- `bench-event-stream.mjs <new-output-directory>`：串行 release-mode loopback WebSocket 压力检查，
  包括按节奏向 32 个 subscriber 投递、burst 时显式 lag closure，以及真实不读取 TCP client
  的 deadline cleanup。使用私人 ignored test harness，不涵盖 history、database、auth 和
  browser。记录源码 hash，每个场景运行三次；拒绝已有 output path。
- `bench-event-history-reads.mjs <new-output-directory>`：串行、无数据库的 filtered-read 矩阵。
  构建 release library test executable，只调用其显式 ignored manual benchmark；不开放新
  public service API。运行 latest-200、sparse filtering、no-match、full-export 及合成 mixed
  read/write 场景，记录源码 hash、每次测量及三次运行的 median/range。真实前端页面不按
  mixed 场景的 10 ms 间隔轮询 history。要求 Cargo offline dependency cache、Node 和 Linux
  GNU `time`；拒绝已有 output path。
- `bench-mainline.mjs <new-output-directory> [repeats=3]`：串行 release-mode、无数据库的 service
  benchmark 矩阵。运行 `mainline_bench` Rust example，包含预填 event history、subscriber
  filtering/JSON、snapshot reader、compiler cache/burst 场景及隔离的本地 learning 数据。
  写入每次 measurement、process resource counter、median/range 及 environment/source metadata。
  记录 Engine source hash 和 worktree status，使未提交实现仍可区分。绝不覆盖已有 output
  directory。要求 Linux GNU `time`、Cargo、Node 和 Clang。不是 HTTP、PostgreSQL、kernel/eBPF
  或 remote-Agent 端到端 benchmark。
  用 `cargo test --manifest-path engine/Cargo.toml --release --locked --example mainline_bench` 验证 measurement helper。
- `bench-event-bus.sh`：运行 legacy local event-bus throughput benchmark（debug build、空初始
  history、无 subscriber）。发布指标不证明持久数据库 throughput；`--verify` 可能观察到
  in-memory fallback，所以不证明持久化成功。
  - 可通过环境变量配置：
    - `CYANREX_BENCH_EVENTS`
    - `CYANREX_BENCH_USERS`
    - `CYANREX_BENCH_CONCURRENCY`
    - `CYANREX_BENCH_PAYLOAD_SIZE`
    - `CYANREX_BENCH_MAX_RECORDS`
    - `CYANREX_BENCH_POLICY=drop_oldest|drop_new`
    - `CYANREX_BENCH_BROADCAST_BUFFER`
    - `CYANREX_BENCH_VERIFY`（`1` 启用）
    - `CYANREX_BENCH_VERIFY_TIMEOUT`
    - `CYANREX_BENCH_DATABASE_URL`
    - `CYANREX_BENCH_JSON`（`1` 加入 JSON 输出）
    - `CYANREX_BENCH_JSON_FILE`（向文件追加 JSON 结果行）
    - `CYANREX_BENCH_LABEL`（可选 benchmark label）
  - JSON 结果包含：
    - `throughput`
    - `avg_publish_latency_ms`
    - `p50_publish_latency_ms`
    - `p95_publish_latency_ms`
    - `p99_publish_latency_ms`
    - `max_publish_latency_ms`

- `bench-event-bus-compare.sh`：比较两个命名 benchmark 场景（`A` 和 `B`），输出差异摘要。
  - 用 `CYANREX_BENCH_<KEY>_A` 和 `CYANREX_BENCH_<KEY>_B` 配置各场景
    （公共 `CYANREX_BENCH_<KEY>` 作为回退）
  - 支持的 key：`EVENTS`、`USERS`、`CONCURRENCY`、`PAYLOAD_SIZE`、
    `MAX_RECORDS`、`POLICY`、`BROADCAST_BUFFER`、`VERIFY_TIMEOUT`、`DATABASE_URL`
  - 默认输出：`scripts/bench-event-bus-compare-<timestamp>.jsonl`
- `perf-regression-check.sh`：以 JSON 模式运行 benchmark，向 JSONL history 追加结果，
  并可选地通过阈值检查与 baseline JSON 比较。
  - 输出 throughput/p50/p95/p99 latency 差异
  - 通过环境变量支持适合 CI 的 fail-fast 阈值：
    - `CYANREX_BENCH_MIN_THROUGHPUT_DELTA_PCT`（默认 `-5`，最低 throughput 差异）
    - `CYANREX_BENCH_MAX_P50_INCREASE_PCT`（默认 `15`）
    - `CYANREX_BENCH_MAX_P95_INCREASE_PCT`（默认 `20`）
    - `CYANREX_BENCH_MAX_P99_INCREASE_PCT`（默认 `30`）
    - `CYANREX_BENCH_BASELINE_JSON`（可选路径，单行 JSON baseline）
- `perf-baseline-capture.sh`：一条命令生成／更新单行 baseline JSON。
  - 默认输出：`scripts/perf-baseline/event-bus-baseline.json`
  - 命令：
    - `./scripts/perf-baseline-capture.sh`
  - 在系统达到稳定基线条件后运行，并提交输出文件。
- `perf-baseline/perf-thresholds.env`：本地回归运行的 baseline 路径和阈值预设。

常见流程：

```bash
# 1) generate baseline
./scripts/perf-baseline-capture.sh

# 2) run regression (with defaults)
set -a; source scripts/perf-baseline/perf-thresholds.env; set +a
./scripts/perf-regression-check.sh

# 3) run with stricter thresholds
CYANREX_BENCH_MIN_THROUGHPUT_DELTA_PCT=-10 \
CYANREX_BENCH_MAX_P99_INCREASE_PCT=40 \
./scripts/perf-regression-check.sh
```

CI 的 `Performance Regression` workflow 也可以通过 `baseline_json` 参数使用基线。
- `package-distribution.sh`：构建并打包用于隔离网络部署的分发产物。
  - 构建或拉取并导出 PostgreSQL、Engine 和 frontend Docker image
  - 生成 `deploy.sh`、`run.sh`、`stop.sh` 及可丢弃环境的 `install-smoke.sh`
  - 输出 checksum-bound `release-metadata.json`，不包含 build-host absolute path
  - 默认输出 `dist/cyanrex-lab-<version>-<timestamp>.tar.gz`
- `distribution-install-smoke.sh`：打包为 `install-smoke.sh`；验证新解包的 release，包括 checksum、
  打包 image content ID、service health、frontend CSP、login 和 remote Agent compilation。
  Package/metadata validation、image inspection 及 health JSON check 使用 Rust，因此打包路径
  不要求宿主 Python。清理保留既有 runtime configuration 和 token；继承的宿主 image override
  不能替换 candidate image。只在可丢弃特权 Linux host 上设置 `CYANREX_SMOKE_RUN_LIVE_KERNEL=1`，
  加入 live kernel attach/event/detach 验收。
- `test-distribution-tools.sh`：packaging helper 和 Compose runtime environment forwarding 的
  静态回归，以及不发布的 Tag candidate workflow 不变量；纳入每种质量门禁模式。

快速检查课堂启动准备时，可显式运行：

```bash
./scripts/check-instance-conflicts.sh \
  --instance-id room-a \
  --engine-port 18080 \
  --frontend-port 13000 \
  --postgres-port 15433
```

commit/PR 前运行质量检查：

```bash
./scripts/quality-gate.sh --format-only   # lightweight format + file-length check
./scripts/quality-gate.sh --backend-only   # backend + file-length check
./scripts/quality-gate.sh --frontend-only --no-npm-install   # frontend + file-length check
./scripts/quality-gate.sh --sdk-only --no-npm-install  # SDK build/tests + file-length check
./scripts/quality-gate.sh --security-only  # security audit only
./scripts/quality-gate.sh --permissions-only  # permission regression checks only
./scripts/quality-gate.sh --security       # adds security audit to full checks
./scripts/quality-gate.sh                  # full checks
```

直接运行安全审计：

```bash
./scripts/check-security-audit.sh
```

构建分发包：

```bash
./scripts/package-distribution.sh --version 0.3.5
./scripts/package-distribution.sh --version 0.3.5 --compose-template docker/docker-compose.yml   # custom compose if needed
```

已有本地 image（例如 CI 或私人 registry 预加载）时，可不重建而直接打包：

```bash
./scripts/package-distribution.sh --skip-build --engine-image myrepo/cyanrex-engine:0.3.5 --frontend-image myrepo/cyanrex-frontend:0.3.5
```

打包还遵循 `ENGINE_RUST_IMAGE`、`ENGINE_DEBIAN_IMAGE`、`ENGINE_APT_MIRROR`、
`ENGINE_CARGO_REGISTRY_INDEX`、`FRONTEND_NODE_IMAGE` 和 `FRONTEND_NPM_REGISTRY`，
与源码 Compose 在受限或镜像网络下的构建控制一致。

分发包入口：

```bash
./deploy.sh up     # start services
./deploy.sh status # check status
./deploy.sh logs   # tail logs
./deploy.sh down   # stop services
./runner-agent.sh start  # optional unprivileged compiler Agent
./runner-agent-smoke.sh  # authenticated remote compile smoke; native CLI is preferred
./live-kernel-smoke.sh   # privileged live attach/event/detach acceptance
./cyanrex-release smoke live-kernel --report /safe/output/report.json
./cyanrex-release smoke runner-agent --agent-id <id>
./cyanrex-release smoke health --engine-url http://127.0.0.1:8080
./cyanrex-release evidence verify /path/to/report.json --release-metadata ./release-metadata.json
./install-smoke.sh       # destructive disposable-host installation acceptance
# `run.sh` and `stop.sh` remain compatibility shortcuts.
```

Runner Agent 和 live-kernel 脚本优先使用 `cyanrex-release`；shell/Python 实现仍为旧环境或
手动组合环境提供迁移回退。安装冒烟要求原生 CLI。创建任何 runtime 文件前，用
`./cyanrex-release package verify .` 验证新解包的软件包；加载 image 后用
`./cyanrex-release package verify-loaded-images .` 对照 checksum-bound metadata 检查 content ID。

解包前验证完整下载的 Tag candidate（目录应只包含 archive、其 checksum、live-kernel report
及其 checksum）：

```bash
release_revision="$(git rev-list -n 1 v0.3.5)"
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  candidate verify /path/to/downloaded-candidate \
  --expect-version 0.3.5 --expect-revision "$release_revision" --expect-tag v0.3.5 \
  --extract-to /path/to/new-output-directory
```

安全解包没有 live-kernel evidence 的双文件软件包：

```bash
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  package extract /path/to/archive-and-checksum \
  --output /path/to/new-output-directory
```
