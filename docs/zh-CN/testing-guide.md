# 当前项目测试指南

源码 **0.6.3** 已收录显式回环／TLS 宿主、受限 Session 签发及私人 Task 创建／列举／重开／清单换版。
不接通浏览器／SDK 保存、局域网或存储迁移；早期实施阶段标签保留原范围与证据。

Unreleased [原绑定文本发布](platform-text-artifact-connection.md)新增
`bash scripts/test-platform-text-publication-host.sh` 六条真实 SQL／工作区／明文／TLS 选择库存，
覆盖原七路由／六通道构造器和显式八路由／七通道 CLI 组合、原绑定、当前权限、独立响应通道、
最小新增 INSERT、私人遗留文件及发布／关联／重开。库存不是通过结果，也不证明浏览器／SDK 验收；
全局 16 连接、四个真实文本 worker 和既有登录／Task／读取权限不变。

未发布的[手动 Draft 清单换版](platform-task-replace-connection.md)增加十条 HTTP 单测及
`bash scripts/test-platform-task-replace-connection.sh` 十条准确 SQL／HTTP／真实宿主选择，覆盖
预期修订、空／仅标题修改与已有文本引用、严格 PUT、原绑定和旧／新字节、当前授权、延期故障
及第六响应通道；不代表 Artifact 发布、状态写入或浏览器／SDK 集成。

未发布的[空任务创建](platform-task-create-connection.md)增加十条 HTTP 单测及
`bash scripts/test-platform-task-create-connection.sh` 八条准确 SQL／HTTP／真实宿主选择，覆盖
私人手动 Draft、严格版本／标识／标题输入、原绑定、受限写入、最终会话检查、冲突／未知结果
及独立第五响应通道；不代表载荷发布、浏览器保存、SDK/OpenAPI 或自动重试。

未发布的[原绑定内容读取](platform-task-content-connection.md)新增八条只读 HTTP 单测、原根／worker
单测及 `bash scripts/test-platform-task-content-connection.sh` 八条准确 SQL／真实宿主选择，覆盖空任务、
按序准确文本、所有者／账号、原描述符、损坏、窄权限、实际过期等待与取消；旧 Task／Artifact 写入及
索引／宿主仍按独立回归组验证。

未发布的[原绑定任务索引](platform-task-index-connection.md)增加四条工作区路由单测，以及
`bash scripts/test-platform-task-index-connection.sh` 八条准确 SQL／HTTP／真实宿主用例，覆盖
空 payload、双所有者隔离、原对象漂移、真实等待／最终过期、严格预读、受限权限及响应存活期；
浏览器保存仍独立。

可选[服务状态通知](platform-host-notifications.md)增加真实 Unix datagram、清空能力 CLI 默认测试，
及下列 TLS 宿主 runner 的三条准确 SQL 选择，覆盖 Ready／Stopping、启动拒绝与发送失败后真实
排空；不证明已安装服务托管。

未发布的[原生 HTTPS 回环宿主](platform-loopback-tls-host.md)，增加默认私有材料／真实 TLS
传输和 CLI 测试，以及 `bash scripts/test-platform-loopback-tls-host.sh` 的原五条 SQL；
加上通知三条，runner 现共八条准确选择。
需要 OpenSSL 生成一次性测试身份，产品宿主不调用它。客户端 CA／IP、SNI／ALPN、握手准入及
Secure Cookie 不证明浏览器或已部署局域网验收。

未发布的[回环宿主](platform-loopback-host.md)新增真实 CLI 拒绝、私有 TCP 传输测试及
`bash scripts/test-platform-loopback-host.sh` 的八条准确 SQL／子进程用例，覆盖原启动绑定、
真实 HTTP 登录／查询／退出、权限及可观测信号排空。合成 TCP 路由不绕过生产启动，也不证明
浏览器／局域网／TLS 验收。

未发布的[共享 Session 路由](platform-session-router.md)新增默认组合传输测试及
`bash scripts/test-platform-session-router.sh` 七条准确 SQL 用例，覆盖同一真实绑定、独立通道、
原对象漂移及确认后交付取消。默认关闭 Source 夹具只证明后端失败，真实查询／退出须由 SQL 验证。

未发布的[声明式存储打开](platform-storage-open.md)新增默认本地探针测试及
`bash scripts/test-platform-storage-open.sh` 的七条准确一次性 PostgreSQL 选择，验证声明到保留
绑定、只读权限、身份漂移及真实锁等待取消，不证明已挂载宿主或浏览器／局域网验收。

独立未发布的[登录 HTTP 适配](platform-login-http.md)新增私有默认传输测试及
`bash scripts/test-platform-login-http.sh` 的七条准确受限角色 SQL 用例，覆盖有界准入、
敏感 Cookie 投影及确认后未交付响应；不证明浏览器送达、丢失 COMMIT 确认、TLS 或已挂载宿主。

未发布的[保留绑定登录连接器](platform-login-connection.md)新增 API／默认测试及
`bash scripts/test-platform-login-connection.sh`：十二条受限角色 SQL 用例，加三条观察真实排队
密码工作的准确库测试，核对连接／锁释放、取消后名额保留、原绑定漂移和最初账号缺失。
只使用一次性 `CYANREX_TEST_DATABASE_URL`；这是内部签发验证，不是公共登录或局域网验收。

[格式 3 标记声明](platform-marker-expectation.md)新增三条纯值、九条严格输入测试，以及
`bash scripts/test-platform-marker-expectation.sh` 的五条显式 SQL 选择，验证原身份声明／当前
诊断兼容，不是已部署宿主或局域网安全验收。

**2026-10-16 未发布**的[绑定会话连接](platform-session-connection.md)新增
`platform_session_backend_tdd` 传输回归及显式数据库／角色／锁／触发器脚本
`bash scripts/test-platform-session-connection.sh`。只使用一次性 `CYANREX_TEST_DATABASE_URL`，
测试不挂载宿主或公开签发 Session。

未发布的[本地文本文件导入](local-text-file-import.md)有 12 条纯测试、6 条浏览器用例及
5 条结构守卫，入口为 `test:editor-languages` 与 `test:task-text-import-browser`；
受控浏览器身份不证明服务端保存或部署。

职责索引见[当前架构图](architecture-map.md)。`scripts/tests/architectureMap.test.mjs`检查原44个
服务声明、可见性／Unix条件、未挂载组合及双语导航；这是结构守卫，编译后的文件错误矩阵和
显式Source／HTTPS runner分别验证自己的实际行为。

源码版本 **0.5.8**（**2026-10-13**）已收录分日期的跟进及其测试选择。未发布标记保留当时
实施阶段，库存和此前 0.5.4 执行记录不是新鲜的 0.5.8 SQL 运行或部署验收；实际发布门禁结果
及未覆盖范围记录于[项目状态](project-status.md)。

[顺序下载](large-file-download-client.md)、[清单 HTTP](large-file-manifest-http.md)及
[准确清单客户端](large-file-manifest-client.md)、所选保存、观察／取消和配对分发路由现收录于
0.5.8；分日期的 0.5.7 证据仍保持原范围。

本文按 **2026-10-08 的 0.5.4 源码** 核对，包含 C2-T–W 意图、派发与检查边界、Next.js 15.5.27
下限，以及此前 AI 适配、草稿准备层、清理器加固、认证防护与独立内容 HTTP 适配。
它说明各模块和边界应测什么、每层通过能证明什么，以及如何避免使用线上数据。本文是测试方案和
源码清单，不是一次新的测试结果。先用[平台链路地图](platform-network.md) 区分已连接与未连接路径。
[功能张量](capability-maturity.md)将实现切片关联到测试源码与分日期证据；公共脚本检查其结构、引用
及中英文评分表一致性，不执行被引用的产品测试，也不建立部署成熟度。

仍基于源码 0.5.4 的 **2026-10-11 未发布多包跟进**扩展元数据组合测试及下方两个精确会话 runner。
这只是库存更新；分日期通过结果仍记于项目进度，原评审和证据日期保持不变。

独立的 **2026-10-11 未发布评估绑定跟进**另增成对可空定义契约用例，并给这两个 runner 各增加
一条用例。两个字段必须显式存在，有绑定/无绑定混合元数据仍保留目录身份，类型化评估仍只接纳
有绑定定义。下方当前数量只是库存，不是执行或旧二进制兼容证据。

独立的 **2026-10-11 未发布 C2-Y 跟进**另增显式配置的目录任务内容 HTTP 路由。下方将其默认、
精确 SQL 和公共守卫清单单列；该库存和旧手动路由结果都不构成本轮执行或部署验收。

独立的 **2026-10-11 未发布 C2-Z 跟进**新增强类型目录草稿发布尝试，与手动 C2-P–W 流程
分开。下方准备、受控确认及精确 SQL 选择属于源码库存，不继承旧手动发布验收。

独立的 **2026-10-11 未发布 C2-AA 跟进**只读观察一个未知目录尝试目标。其当前读取比较及
类型化失败与 C2-Z 发布、旧手动 C2-Q 观察分开，早期两组结果都不证明本轮执行。

源码 0.5.5 上独立的 **2026-10-11 未发布[任务内容索引](task-content-index.md)**新增
`task_content_index_tdd` 与 `session_task_content_index_tdd`。独立精确 runner
`scripts/test-task-content-index.sh` 选择**四条可信存储 + 八条会话 SQL 用例**，共 **12** 条。
这是源码库存，不是执行结果，也不继承旧发布或内容读取验收。选择范围为分页/游标上限、
手动/中立目录准入、完整记录及前瞻校验、私人所有权、Artifact 独立性、当前生命周期和
观察到锁等待后的取消/到期。

源码 0.5.5 上同日独立的**未发布[索引 HTTP 适配](task-content-index-http.md)**新增
`task_content_index_http_tdd` 和**八条 SQL 源码选择**，与十二条服务层索引用例分开。
检查严格原始查询、私有投影、旧路由不变、当前 Session 故障/等待及响应正文容量；
该库存不证明执行、公共挂载、浏览器接线或交叉平台编译。精确 runner 为
`scripts/test-task-content-index-http.sh`，源码库存/接线守卫为
`scripts/tests/taskContentIndexHttp.test.mjs`，不替代数据库执行。

## 测试分层

| 层次 | 维护入口 | 通过能够证明 | 不能证明 |
|---|---|---|---|
| 公共源码和工具检查 | [`quality-gate.sh`](../../scripts/quality-gate.sh)、[`scripts/tests`](../../scripts/tests) | 长度、版本、课程同步、公共 API/SDK 契约及脚本/合成工具夹具回归 | 实际部署流程、真实 SSH/Docker/内核验收 |
| 默认 Rust 测试 | `cargo test --manifest-path engine/Cargo.toml --locked` | 模型、服务、进程内路由、默认集成夹具与编译期契约 | 显式忽略的 PostgreSQL、真实 Agent 传输和性能用例 |
| 显式 PostgreSQL 测试 | CI 的精确用例与下方存储 runner | 一次性数据中的真实 SQL 事务、命名空间/schema、锁、竞争、取消和故障注入 | 浏览器集成、部署数据迁移或线上授权切换 |
| 前端构建与单元测试 | 前端质量门禁和 [`frontend/package.json`](../../frontend/package.json) | 生产构建、TypeScript、状态/请求/权限/编辑器回归，以及生成资源异常时的拒绝检查 | 真实浏览器渲染或真实 Engine 连接 |
| 浏览器回归 | 显式 `test:*-browser` 脚本 | 真实 Next 页面或本地生成资源夹具、交互、Monaco 模型/worker 与夹具定义的请求行为 | Engine 被模拟时的真实服务授权、数据库/内核端到端链路 |
| SDK 与契约 | `quality-gate.sh --sdk-only`、[`sdk-js/package.json`](../../sdk-js/package.json) | 生成模型/操作、兼容性、运行请求、类型检查和打包消费者导入 | 公共契约之外的能力，包括通用准备层命令 |
| 工具与发布夹具 | `test-runner-agent-tools.sh`、`test-distribution-tools.sh`、`test-live-kernel-smoke.sh`、`test-release-candidate.sh` | 模拟环境中的密钥处理、目标/元数据/归档校验、清理和失败报告 | 真实安装、远程 SSH 改动、Agent 部署或内核执行 |
| 显式真实验收 | `runner-agent-smoke.sh`、`live-kernel-smoke.sh`、`distribution-install-smoke.sh`、原生 `cyanrex-release` | 该轮记录的实际环境、候选包和操作 | 其它内核、机器、版本或隔离保证 |
| 性能测量 | `bench-mainline.mjs`、`bench-event-stream.mjs`、`bench-event-history-reads.mjs` | 被测负载、构建配置和源码快照的测量结果 | 正确性验收或未经测量的整机吞吐结论 |

所有质量门禁模式都会先跑公共预检与工具夹具；默认模式再跑后端、前端和 SDK。`--security` 加入
Rust 公告审计，前端/SDK 检查也包含生产依赖的 npm 审计。因此 `--format-only` 并非只检查格式。
门禁**不会**自动跑所有 ignored 用例、浏览器套件或真实特权冒烟。
[CI 配置](../../.github/workflows/ci.yml) 额外执行具名 PostgreSQL 用例与真实回环 Agent 编译；
应核对其中的显式清单，不能认为 `cargo test` 已覆盖它们。

## 按模块选择边界测试

| 改动范围 | 主要测试 | 应一并覆盖的相邻边界 |
|---|---|---|
| 旧认证与角色 | `routes_tdd`、`auth_service` 单元/显式 SQL；前端 `authSession`、`sidebarPermissions` | 账户生命周期、CSRF、教师权威、课堂注册取消、UI 退出失败 |
| 课堂发现与邀请 | `classroom_tdd`、`classroomConnection` 工具/单元/浏览器套件 | 版本/协议/能力拒绝、已确认来源、一次性邀请 → 旧账户创建 |
| 脚本与学习持久化 | `module_boundaries_tdd`、`learning_store` | 所有者隔离、损坏/失败 JSON 写入、真实脚本 SQL、反馈修订竞争、历史恢复不重跑 |
| 模块、头文件和终端 | `routes_tdd` 模块/命令用例、头文件/分发器服务测试、前端 `terminalCommand` | 教师专有修改、校验和错误下载、选用元数据 → 编译器、不得分发任意 shell |
| eBPF 编辑与执行 | 编译器/加载器/Runner；`compilerCheck`、`semanticCompletion`、运行/断点浏览器套件 | 检查与运行分离、诊断源码新鲜度、取消/租约所有权、精确卸载；真实内核验收另跑 |
| Runner Agent | 认证器、注册表、作业队列/执行器/客户端；`runner_agent_client_tdd`、清单/管理浏览器测试 | 签名/防重放 → 租约/结果、超时/取消、所有者远程检查、不得回退执行 |
| 教学包与通用目录 | `task_catalog_tdd`、`teaching_task_adapter_tdd`、`collaboration_contract_tdd` | 类型化证据、精确定义/策略身份；目录准入不执行规则、不验收 Task |
| 纯元数据目录组合 | `task_definition_catalog/composition.rs`、`taskCatalogComposition` 公共守卫、会话目录/内容组合 SQL 用例 | 显式包/版本、顺序、完整引用冲突及嵌套 256 定义总上限；已有 Schema 2/3 命令保留私人所有权和完整定义检查，不依赖提供器或执行规则 |
| 明确评估绑定选择 | `task_definition_catalog/assessment_binding.rs`、`taskAssessmentBinding` 公共守卫、会话目录/内容绑定 SQL 用例 | 必需的可空 JSON 字段、成对无绑定、旧已绑定编码、混合元数据、派发前拒绝无绑定的类型化提供器及两个方向的绑定漂移；不升级手工模式、不自动创建 Review，也不保证旧二进制降级 |
| 事件和设置 | `module_boundaries_tdd`、EventBus、显式事件 SQL、事件/设置单元与浏览器套件 | 发布顺序 → 持久化历史 → 重新同步、导出/删除筛选安全、确认设置写入和读取失败 |
| 通用身份与权威 | `collaboration_*_tdd`、`legacy_workspace_projection_tdd` | 审计绑定/成员/授权修订、缺席键竞争、最后管理者、角色与部署权限分离 |
| 注册表时间 | `test-registry-timestamps.sh` 从四个既有目标准确选择十条用例 | 可空退役有效性、必填身份/策略审计时间、有限边界、旧历史/重放、追加读回、对账错误及 Session 命令拒绝；不新增年代/排序规则 |
| 持久化认证源与生命周期 | `durable_auth_source_tdd`、`durable_session_boundary_tdd`、`durable_collaboration_tdd`、删除/改密/初始化/核对测试目标 | 来源命名空间/表身份 → 账户世代 → 精确当前 Session → 同事务身份/策略、命名空间替换、写后事实 |
| 准备层密码工作 | 默认 `auth_service::durable_source::password_work` 单测及源码接线守卫 | 派发/排队容量、取消/超时生命周期、panic 回收与真实哈希；各入口另重跑来源、改密和初始化 SQL |
| 缺失账号密码工作 | `test-durable-login-password.sh`，固定 PHC 单测及源码/CI 守卫 | 观察共享门控真实派发到被占用的执行器、SQL 资源释放、取消、同名并发注册及占位密码匹配仍拒绝；不是计时基准或完整抗枚举证明 |
| 显式过期会话清理 | `test-durable-session-cleanup.sh`：九条准确库内 SQL、默认关闭连接池测试及源码/CI 守卫 | 128 条分批、有效会话保留、独立写者、锁后截止时间、文本/时间预检、孤儿/坏候选及命中证明的删除/提交/取消故障；不自动清理或操作线上数据 |
| 准备层密码参数 | `test-durable-password-profile.sh`，默认 `password_profile`/`password_work` 单测与源码/CI 守卫 | 六条准确 SQL 覆盖坏记录、写入兼容、已有 Session 和回滚；极端数字仅做纯预检测试，不执行巨量计算 |
| 准备层 OTP 时效 | `test-durable-otp-freshness.sh` 准确选择七条 `--lib` SQL；默认 `otp::tests` 及源码/CI 守卫 | 观察 SQL 等待，或执行器占用时读事务释放后，再推进私有时钟；不证明准确入队时刻。覆盖回滚、成功与时效/组合消费区别，不改机器时间或提供公开时钟覆盖 |
| OTP 消费策略 | 默认 `otp_consumption::tests` 与 `durableOtpConsumption` 源码守卫 | 纯计数器选择、真实碰撞、凭据绑定、准确末次计数器和输入/时间边界；纯单测不证明持久化/原子性。策略现用于准备层登录/改密 |
| OTP 原子消费 | `test-durable-otp-consumption.sh`：11 条准确库内 SQL | 独立来源、重开/退出、登录改密竞争、账号代次、版本/格式拒绝、抑制/篡改/延迟失败/取消及真实碰撞 |
| OTP 来源格式 | `test-durable-otp-schema.sh`：四条准确 SQL | 真实 Schema 1 原样拒绝、Schema 2 列/默认值/约束损坏拒绝、注册/初始化水位 -1 及命中证明的初始化故障；不测试或宣称存量迁移 |
| 运维初始化 | `provision_cli_tdd`、`provision_postgres_tdd` | 只读计划、目标绑定 apply、私密配置交付、确认丢失与取消；不收编现有配置 |
| 通用 Task、Artifact 与 Review | 下方显式存储 runner 及对应默认 Rust 测试 | 精确修订/摘要、Task/outbox 原子性、不可变文件边界、私人所有权、Review 历史、当前 Session 检查 |
| 本地任务 payload 与编辑器 | `test:editor-languages`、`test:task-payload-browser`、`test:multi-language-editor-browser` | 可选 payload、已接受内容/修订、导入导出、过期编辑、模型释放与禁止网络写入 |
| DOMPurify 与生成的 Monaco 资源 | `frontendDependencySecurity` 公共守卫、默认 `test:tooling`、显式 `test:dompurify-browser` | 锁定副本和 override → 精确替换第三方代码 → 新 chunk 引用 → Monaco 实际使用的清理器；依赖审计与部署行为另行验证 |
| 内部任务内容元数据 | Rust `task_content_contract_tdd`、`task_content_binding_tdd` | 严格对象结构、字段/数量上限、有序准确引用、给定所有者/字节/摘要一致性；默认纯测试，不验证存储或 Session 授权 |
| 大文件清单及流式核心 | 默认 `file_transfer_tdd`、私有 Unix 来源检查单元及 `fileTransfer` 公共库存守卫；[边界](large-file-core.md) | 严格有界元数据、真实分块/整文件摘要、精确流结束、固定缓冲、部分输出/IO 故障及预开普通文件检查；不验收私有暂存、持久进度、配额、认证传输、基准或桌面互通 |
| Unix 私有分块暂存 | 默认 `file_chunk_staging_tdd` 及私有排队工作单元；[边界](large-file-staging.md) | 准确清单绑定、独占私有文件、失败保留、可观察根/文件项漂移及实际取消/排队生命周期；不是上传回执、预留配额、真实 fsync/磁盘满故障验收、恢复、网络或基准 |
| 受信任上传意图与逻辑额度 | 默认 `file_upload_intent_tdd` 准入用例、显式 `scripts/test-file-upload-intent.sh`、`fileUploadIntent` 源码守卫；[边界](large-file-upload-intent.md) | 全新 Schema、准确所有者/账号代次、跨代次字节/条数竞争、有界损坏账本拒绝与真实触发器/取消正控；仅元数据，不证明分块回执、物理分配、当前 Session、续传或传输 |
| 上传绑定本地暂存 | 默认 `file_upload_staging_tdd` 准入用例、显式 `scripts/test-file-upload-staging.sh`、`fileUploadStaging` 源码守卫；[边界](large-file-upload-staging.md) | 真实跨块字节、锁等待后的原来源/记录复核、原 inode 回执、共享准入/取消及失败保留；不验收持久分块日志、Session 权限、续传、网络、真实磁盘满/fsync 故障或基准 |
| 分块写前日志 | 默认 `file_upload_chunk_journal_tdd`、显式 `scripts/test-file-upload-chunk-journal.sh`、`fileUploadChunkJournal` 源码守卫；[边界](large-file-chunk-journal.md) | 任何文件派发前提交 Unknown、准确日志坐标、真实 SQL 等待/取消、文件保留及仅元数据观察；不是恢复执行、状态证明的当前完整性、整文件最终核验、Session 权限或传输 |
| 整上传当前字节核验 | 默认 `file_upload_whole_tdd`、显式 `scripts/test-file-upload-whole.sh`、私有工作单元及 `fileUploadWhole` 公共守卫；[边界](large-file-whole-verification.md) | 有序绑定块/整文件摘要、原来源/文件快照漂移、只读结果及共享工作准入；不是历史回执、日志收编、持久发布、恢复、Session 权限或传输 |
| 持久文件发布元数据 | 默认 `file_upload_publication_tdd`、显式 `scripts/test-file-upload-publication.sh`、`fileUploadPublication` 公共守卫；[边界](large-file-publication.md) | 准确完整已确认日志与真实整字节、不可变上传/版本唯一、有界全局容量、原身份与提交确认后失败；不是恢复回执、永久物理封存、当前 Session、授权或传输 |
| Session 私有文件元数据 | 默认 `session_file_metadata_tdd`、显式 `scripts/test-session-file-metadata.sh`、`sessionFileMetadata` 公共守卫；[边界](large-file-session-metadata.md) | 单一认证源事务推导 owner／账号代次，精确读取本人上传／Published、新鲜策略及到期检查、配额、真实等待／故障及原认证源／目标身份；不是真正 Session 发布、字节健康、授权、恢复或传输 |
| 纯文件发布／接收授权 | 默认 `file_access_tdd` 及 `fileAccess` 公共组合／文档守卫；[边界](large-file-access-contract.md) | 准确账号／版本选择器、独立撤权、半开时间区间及严格有界 JSON；不是签发来源、审计当前授权、Session／SQL 授权、字节访问或传输 |
| 未发布的持久发布权限元数据 | `file_publisher_grant_tdd`、`session_file_publisher_grant_tdd`、显式 `scripts/test-file-publisher-grants.sh` 和 `scripts/test-session-file-publisher-grants.sh`、`filePublisherGrant` 组合守卫；[边界](large-file-publisher-grants.md) | 可信签发／撤销归属、有界全账本写入、CAS 撤权、当前 Session 本人读取和历史写屏障；不是管理员认证、发布权限消费、接收授权、字节或在线挂载 |
| 未发布的宿主配置 Session 授权管理 | `session_file_grant_management_tdd`、显式 `scripts/test-session-file-grant-management.sh`、`fileGrantManagement` 守卫；[边界](large-file-grant-management.md) | 准确配置管理者／账号、推导操作者、目标生命周期、延迟工作／原固定／新鲜 Session 复核；不是继承角色、持久在线配置撤权、发布消费、接收者、字节或在线挂载 |
| 未发布的 Session 授权文件发布 | `session_file_publication_tdd`、显式 `scripts/test-session-file-publication.sh`、`fileSessionPublication` 守卫；[边界](large-file-session-publication.md) | 当前准确操作者／授权、真实字节、跨工作间隙原固定、屏障内插入／延迟／新鲜有效期及原确认后复核；不是接收者、传输、在线挂载、恢复或丢失确认回滚保证 |
| 未发布准确文件接收账本及本人读取 | `file_recipient_grant_tdd`、`session_file_recipient_grant_tdd`、显式 `scripts/test-file-recipient-grants.sh` 和 `scripts/test-session-file-recipient-grants.sh`、`fileRecipientGrant` 守卫；[边界](large-file-recipient-grants.md) | 独立准确文件／账号绑定、CAS／历史／额度及认证源当前接收者读取；不是接收授权管理、下载权限、字节或传输 |
| 未发布发布者本人分享 | `session_file_sharing_tdd`、显式 `scripts/test-session-file-sharing.sh`、`fileSharing` 守卫；[边界](large-file-sharing.md) | 本人准确 Published、活跃发布权限／当前接收者、延迟原固定／增量／最终新鲜时间及独立原所有者撤销；不是全局管理、下载、字节 IO 或传输 |
| 未发布接收者下载清单 | `session_file_download_tdd`、显式 `scripts/test-session-file-download.sh`、`fileDownloadManifest` 守卫；[边界](large-file-download-manifest.md) | 本人 Active 接收授权、准确原发布者／Published 版本、原记录及最终同钟授权／Session 有效期；只有元数据，不是字节健康、可复用许可或传输 |
| 未发布授权分块快照 | `session_file_chunk_read_tdd`、显式 `scripts/test-session-file-chunk-read.sh`、私有读取单元与 `fileChunkRead` 守卫；[边界](large-file-chunk-read.md) | 真实有界单块字节、无锁间隙前后原根／记录、最终接收授权／Session 到期及保留预算；不是整文件重哈希、HTTP／客户端送达或恢复 |
| 未发布未挂载二进制分块 HTTP | `file_chunk_read_http_tdd`、显式 `scripts/test-file-chunk-http.sh`、私有 query／正文单元及 `fileChunkHttp` 守卫；[边界](large-file-chunk-http.md) | 严格空 GET、原当前接收者命令、固定二进制帧、Bytes 所有者跨两种预算及首帧期限；不是已挂载局域网送达、客户端保存／续传或恢复 |
| 未发布已提供响应 Unix 接收器 | `file_chunk_receiver_tdd`、显式 `scripts/test-file-chunk-receiver.sh`、私有任务存活单元及 `fileChunkReceiver` 守卫；[边界](large-file-chunk-receiver.md) | 准确单例头、私有有界帧／真正 EOF／摘要及 idle／total 后独占暂存；不是主动获取、远端回执、整文件拼装／保存、续传或在线挂载 |
| 未发布显式单块 HTTP 客户端 | `file_chunk_client_tdd`、显式 `scripts/test-file-chunk-client.sh`、私有请求／工作单元及 `fileChunkClient` 守卫；[边界](large-file-chunk-client.md) | 真实环回请求／文件、准确输入、独立头／正文期限及任务持有准入，另有 ignored 当前 Source SQL 和 OpenSSL CA／SAN 对照；不是正常应用挂载或整文件／续传验收 |
| 未发布本地整文件拼装 | `file_chunk_assembly_tdd`、暂存私有任务／变动单元及 `fileChunkAssembly` 守卫；[边界](large-file-chunk-assembly.md) | 真实旧式输入、独占实际整文件输出／EOF／摘要／读回、冲突及取消所有权；不是用户选路径保存、网络调度、恢复进度或部署验收 |
| 0.5.8 顺序私有下载 | `file_download_client_tdd`、私有流程／工作单元、显式 `scripts/test-file-download-client.sh` 及 `fileDownloadClient` 守卫；[边界](large-file-download-client.md) | 实际有序 HTTP 到整文件字节、首失败保留及排队／运行取消所有权；独立当前 Source SQL 桥接，不是界面保存／续传或已挂载部署 |
| 0.5.8 接收者清单 HTTP | `file_download_manifest_http_tdd`、私有契约单元、显式 `scripts/test-file-download-manifest-http.sh` 及 `fileManifestHttp` 守卫；[边界](large-file-manifest-http.md) | 严格六项选择器／当前接收者及有界私人 JSON／正文准入，不是客户端获取、字节、已挂载送达或续传 |
| 0.5.8 准确清单客户端 | `file_manifest_client_tdd`、私有帧 Body／工作单元、显式 `scripts/test-file-manifest-client.sh` 及 `fileManifestClient` 守卫；[边界](large-file-manifest-client.md) | 真实元数据 GET／严格完整版本及顺序私有整文件字节，含空文件 GET 与实际 Source 撤权；不是界面保存／续传或已挂载部署 |
| 0.5.8 宿主选择私有保存 | `file_save_tdd`、私有源／输出漂移和工作单元、`fileSave` 守卫及扩展的显式清单客户端 SQL 桥接；[边界](large-file-save.md) | 原描述符／完整清单连续性，独占实际复制／读回、保留不确定输出及真实 worker 生命周期；不是普通目录／界面保存、续传或在线挂载 |
| 未发布授权下载及私有保存 | `file_delivery_tdd`、私有整流程／任务生命周期、`fileDelivery` 守卫及 `test-file-delivery.sh`；[边界](large-file-delivery-client.md) | 一次准入跨真实清单／块／拼装／保存，独立输出读回及首失败／取消留存；不是界面、续传或在线挂载 |
| 未发布显式本人普通目录保存 | `file_user_save_tdd`、私有目录／任务／阶段单元、`fileUserSave` 守卫及扩展的既有交付 SQL；[边界](large-file-user-save.md) | 固定原父目录 UID／GID／模式、拒绝危险路径／权限、实际副本与取消；私有暂存仍 0700，不验收共享可写目录或界面 |
| 未发布实时交付进度观察 | `file_delivery_progress_tdd`、私有实际任务生命周期、`fileDeliveryProgress` 守卫及扩展既有交付SQL；[边界](large-file-delivery-progress.md) | 单最新快照、真实ACK前缀、失败／取消／慢读取与实际输出；不是持久回执、恢复、续传、界面或部署验收 |
| 未发布显式交付等待取消 | `file_delivery_cancel_tdd`、私有实际任务／通道选择单元、`fileDeliveryCancel` 守卫及扩展既有Source；[边界](large-file-delivery-cancellation.md) | 明确请求／句柄丢弃区别、输入／取消／容量优先级、真实Cancelled后任务资源／输出留存；不是即时kill、回滚、恢复或界面 |
| 未发布完整HTTPS交付验证 | 三条显式原生 `file_delivery_tls_tdd`、`test-file-delivery-tls.sh` 及 `fileDeliveryTls` 库存守卫；[范围](large-file-delivery-https.md) | 实际一次性TLS、完整保存字节、凭据前身份拒绝及显式等待取消；与SQL权威和已部署局域网验收分开 |
| 未发布配对分发路由 | `file_distribution_http_tdd`、`fileDistributionHttp` 及 `test-file-delivery.sh` 六条显式Source（含三条原生HTTPS联验）；[边界](large-file-distribution-router.md) | 一套缓存配置、两个独立HTTP门、实际binary owner／元数据／撤权，不是Session签发或隔离服务 |
| 未发布真实Source与HTTPS联验 | `file_delivery_tdd` 三条 `source::source_https`、同一 `test-file-delivery.sh` 及 `fileSourceHttps` 守卫；[范围](large-file-source-https.md) | 当前真实grant、原Published字节、单router、原生HTTPS、所选保存、派发前撤权及正文取消；不是局域网部署、签发或隔离 |
| 草稿导入与发布计划 | Rust `task_draft_publication_tdd`、`taskDraftPublication` 公共守卫、前端 `taskDraft` 共享夹具 | 严格有界 JSON、空白标题拒绝、本地身份剥离、给定准确内容映射及标准导出兼容；不发布内容、不检查当前 Session、不保存浏览器草稿 |
| 会话草稿发布步骤 | Rust `session_task_draft_publication_tdd`、`scripts/test-session-task-draft-publication.sh`、`sessionTaskDraftPublication` 公共守卫 | 固定分配与 Session 绑定、每次推进一个写入、空/多项成功、确认前缀与未知步骤、取消、撤权及最终真实字节检查；无持久恢复或浏览器保存 |
| 独立目录草稿发布 | `session_catalog_draft_publication_tdd`、内部 `catalog_draft_publication::tests`、`scripts/test-session-catalog-draft-publication.sh`、`sessionCatalogDraftPublication` 守卫 | 无 I/O 的准确完整定义冻结、独立 token 域、每次推进一个已有 Session 写入、完整 `Some` 确认、保留前缀与未知目标及手动/日志类型隔离；C2-AA 比较另列，不提供恢复、重试或 HTTP/浏览器保存 |
| 目录未知目标观察 | `session_catalog_draft_observation_tdd`、内部 `catalog_draft_publication::observation_tests`、`scripts/test-session-catalog-draft-observation.sh`、`sessionCatalogDraftObservation` 守卫 | 一个已有当前 Session 读取、冻结完整 `Some` 与有序字节、有效差异与类型化损坏、缺失/他人目标范围、取消和尝试不变；不收编确认、不推导历史提交/回滚、不恢复或重试 |
| 未知草稿目标观察 | Rust `session_task_draft_observation_tdd`、`scripts/test-session-task-draft-observation.sh` | 原 Session/状态准入、精确只读比较、类型化错误、尝试不变、当前 Task 字节与历史前缀区别及 Task 缺席范围；无回滚证明或恢复推进 |
| 草稿元数据检查点 | 默认 Rust `draft_publication::tests::checkpoint`、`session_task_draft_checkpoint_tdd`、`sessionTaskDraftCheckpoint` 公共守卫 | 严格有界元数据 codec、有序分配/长度、状态/计数关系及往返；导入进度是数据，不是授权、来源证明、持久化或尝试恢复 |
| 检查点目标显式检查 | 默认 `draft_publication::tests::inspection`、`session_task_draft_inspection_tdd`、`scripts/test-session-task-draft-inspection.sh`、`sessionTaskDraftInspection` 公共守卫 | 显式范围/当前 Session、一个已有读取器、精确元数据/文本检查、类型化故障、缺席/空 Task 边界及数据不变；不证明原身份/正文来源，不提供日志或恢复 |
| 不可变会话草稿意图 | `session_task_draft_intent_tdd`、`scripts/test-session-task-draft-intent.sh`、`sessionTaskDraftIntent` 公共守卫 | 真实 Ready/零进度准入、有界规范元数据、重复冲突、精确所有者/账号代次、当前认证源/登记/日志身份及失败结果；不派发资源、不复活尝试、不实现写前执行 |
| 日志化会话草稿派发 | `session_task_draft_dispatch_tdd`、`scripts/test-session-task-draft-dispatch.sh`、`sessionTaskDraftDispatch` 公共守卫 | 显式 Schema 2、消耗初始尝试、确认 Unknown 预留、准确所有者/账号/nonce 守卫、资源/标记原子性及完整前缀/身份复验；无重启/重试或公共/浏览器验收 |
| 会话草稿日志检查 | `session_task_draft_journal_inspection_tdd`、`scripts/test-session-task-draft-journal-inspection.sh`、`sessionTaskDraftJournalInspection` 公共守卫 | 仅 Schema 2、当前精确所有者/账号、完整已提交前缀加可选 Unknown、最终 Task 计数、身份/新鲜权限复验及类型化错误；不读取资源/文件，不恢复尝试 |
| 日志步骤资源观察 | `session_task_draft_journal_observation_tdd`、`scripts/test-session-task-draft-journal-observation.sh`、`sessionTaskDraftJournalObservation` 公共守卫 | 序号准入、同事务精确资源读取、读取后原完整日志/nonce/首次身份复验、新鲜权限及不含正文结果；不写业务、不提供回执或恢复 |
| 独立内容存储 | Rust `task_content_store_tdd`、`scripts/test-task-content-storage.sh` | Schema 2/3 隔离、完整元数据/outbox 原子性、修订冲突与存储故障；不是 Session 内容或浏览器适配 |
| 会话任务内容 | Rust `session_task_content_tdd`、`scripts/test-session-task-content.sh` | 同一事务内当前授权、旧新精确文本、元数据编辑、移除、命名空间身份及写后复验；不是公共/浏览器验收 |
| 私人任务内容元数据索引 | `task_content_index_tdd`、`session_task_content_index_tdd`、`scripts/test-task-content-index.sh` | 有界 UUID 页面、仅位置游标、严格任务头/前瞻、显式中立目录准入及无 Artifact 访问的当前会话；不验收完整清单、内容可用性、HTTP/SDK/UI 或恢复 |
| 独立元数据索引 HTTP | `task_content_index_http_tdd`、`scripts/test-task-content-index-http.sh`、`taskContentIndexHttp` 守卫；[契约](task-content-index-http.md) | 精确 GET 传输、严格原始游标/上限、私有七字段投影、当前会话委托、损坏/空页缺列拒绝、Artifact 独立性与响应存活期容量；不验收公共/浏览器/SDK 接线、交叉平台或部署 |
| 独立目录任务内容 HTTP | `catalog_task_content_http_tdd`、`scripts/test-platform-catalog-task-content-http.sh`、`catalogTaskContentHttp` 公共守卫 | 严格精确引用对象 → 可信完整定义、有绑定/无绑定目录身份、手动/所有者隔离、当前 Session/完整元数据/文本检查及共用传输/准入结果；不评估，也未接公共/浏览器路径 |
| UI 导航与安全 | `test:ui-permissions`、布局/动作/账户/运行浏览器套件 | 绑定目标的确认、防重复操作、路由切换、键盘操作、未保存草稿保护 |
| API、SDK、打包与部署 | 公共契约测试、SDK 门禁、发布/SSH Rust 目标与工具夹具 | Engine 路由 → OpenAPI → 生成 SDK；归档来源 → 精确已审阅 SSH 目标；夹具与实际安装分离 |

`authSession` 等简写指 `frontend/tests` 中的对应文件；精确 npm 脚本名以 `frontend/package.json`
为准。Rust 精确筛选前先执行 `--list`，避免名称拼错后“零条测试通过”。存储 runner 自带这一步检查。

AI Agent 配置另选 `ai_agent_settings_tdd`、私有存储取消单元、`test:ai-agents`、显式
`test:ai-agents-browser` 及 SDK Agent 运行/类型/包测试。边界为教师/CSRF → 修订配置 → 准确 UI 确认，
以及有限工具、逐次批准、恶意调用与禁止自动重试；合成协议格式不证明真实模型服务验收。

## 安全的默认流程

从用于测试的检出目录开始。启动前清除继承的部署配置，尤其是 `DATABASE_URL`、`CYANREX_*`、
PostgreSQL 连接变量和 dotenv 预加载设置，再只添加所选夹具需要的变量。仅清除
`CYANREX_TEST_DATABASE_URL` 不能保护仍向普通服务继承了 `DATABASE_URL` 的进程。
不要复用部署数据目录、私有 Artifact 根目录、凭据或 Agent token。

准备好隔离环境后，选择最小相关门禁及其边界测试：

```bash
./scripts/quality-gate.sh --backend-only
./scripts/quality-gate.sh --frontend-only --no-npm-install
./scripts/quality-gate.sh --sdk-only --no-npm-install
```

只有依赖已匹配锁文件时才用 `--no-npm-install`。常规前端构建会从 `docs/` 同步课程副本；不要把
`frontend/public/course` 当成文档源。仅改文档时检查长度、版本同步、课程同步和链接，不把这些检查
描述成新一轮运行时回归。

前端门禁还运行 `test:private-request`，检查共享传输生命周期。各功能测试保留自身的响应/回执
策略；显式组件浏览器夹具分别检查 Settings、Metrics、Runner、Events 与 Runtime 状态。
浏览器等待被取消不代表服务端回滚。

## 显式数据库套件

使用新建的一次性 PostgreSQL 实例、专用测试库和私有临时文件。可用私有 Unix socket 或受限回环
监听，绝不能指向部署数据库。夹具会主动创建/删除 schema、替换记录、注入触发器并破坏合成内容，
部分用例需要普通应用角色没有的 schema/对象权限。[验收说明](acceptance.md) 的隔离流程与当前 CI
夹具可作为例子，但不是复用旧本地数据库凭据和目录的授权。

将 `CYANREX_TEST_DATABASE_URL` 显式设为该一次性数据库，保持 `DATABASE_URL` 未设置。部分旧 SQL
套件还要求 `CYANREX_DB_FALLBACK=true`，这只是夹具设置，不能把回退当成持久化成功。
初始化 CLI 的 SQL 夹具还使用 `CYANREX_TEST_DATABASE_PASSWORD`。以各套件夹具和 CI 环境为准。

| Runner | 当前源码快照的用例数 | 主要边界 |
|---|---:|---|
| `scripts/test-task-storage.sh` | 15 | 受信任 Task 存储、生命周期修订与 outbox |
| `scripts/test-task-content-storage.sh` | 21 | 独立 Schema 3 手工 Task/清单持久化、原子编辑、旧格式拒绝与故障/并发检查 |
| `scripts/test-artifact-storage.sh` | 17 | 精确不可变内容、私有文件故障与固定输入 |
| `scripts/test-review-storage.sh` | 17 | Review 历史、人工/规则分离、非教学集成 |
| `scripts/test-session-task-storage.sh` | 17 | 当前 Session → 私人手工 Task |
| `scripts/test-session-artifact-storage.sh` | 18 | 当前 Session → 私有发布与精确读取 |
| `scripts/test-session-task-inputs.sh` | 20 | Task → 精确本人 Artifact 输入与写后验证 |
| `scripts/test-session-review-storage.sh` | 18 | Session → 带精确目标/证据的私人人工 Review |
| `scripts/test-session-catalog-tasks.sh` | 20 | 精确准入有绑定/无绑定定义元数据、无提供器的单包/组合目录和 0–32 个输入 |
| `scripts/test-session-task-revisions.sh` | 17 | Draft 换版、旧/新输入并集、schema 2 兼容性与并发保存 |
| `scripts/test-session-task-content.sh` | 33 | Schema 3 手动/目录内容、有绑定/无绑定组合元数据、精确定义/文本、旧新复验、命名空间替换、生命周期与并发故障 |
| `scripts/test-platform-task-content-http.sh` | 13 | 独立 HTTP → C2-M、私有归属、等待锁时 Session 过期、精确文本、提交/outbox 故障、并发编辑及覆盖响应正文完成的共享准入 |

十二个 runner 共选择 **226 条用例**（此前资源 159 条、内容存储 21 条、会话内容 33 条、独立 HTTP 13 条），
不是仓库全部 PostgreSQL 覆盖。两条目录新增用例覆盖无评估器的元数据构造与精确准入；这是当前
源码库存，不改写旧执行数量。六条目录内容新增用例沿用 Schema 3，覆盖显式模式隔离、完整定义
一致性、文本/修订守卫，以及故障和等待前后的当前授权。旧认证/事件/脚本/学习与通用
身份/认证源/生命周期/初始化用例在 CI 中另有精确清单。
[`postgresCi.test.mjs`](../../scripts/tests/postgresCi.test.mjs) 检查 runner 清单完整性。
多包跟进在 `session_catalog_task/composition.rs` 和 `session_task_content/catalog_composition.rs`
各增加一条用例，通过维护的精确 runner 选择。纯组合测试或源码守卫不能代替数据库执行。
独立评估绑定跟进另增 `session_catalog_task/assessment_binding.rs` 和
`session_task_content/catalog_assessment_binding.rs`，检查有绑定/无绑定定义经过原 Schema 2/3
路径、显式 null 持久化、私人内容不变，以及同编号下增减绑定的拒绝。两条用例已经计入上方
库存，不在其精确 runner 之外额外加计。

**2026-10-08 未发布的 C2-N 准入跟进**需检查状态克隆及分别构造路由共享容量、正文轮询/SQL
之前的拒绝，以及成功和错误响应正文持续持有名额。覆盖收集/取消、最终帧/结束/丢弃时释放与
私有错误结果不变。显式 SQL 回归应包含保留成功内容正文、等待中的变更、超额请求不产生业务
副作用、取消及原 Session 检查。当前 HTTP 清单在原十一条基础上新增两条精确 SQL 用例，
库存不代表已完成运行。这些是适配器边界，不是全局工作线程配额或回滚证明；
原 0.5.0 结果不作为本次跟进的执行证据。

独立的 **2026-10-08 未发布 C2-X** 目标 `text_artifact_http_tdd` 和精确 runner
`scripts/test-platform-text-artifact-http.sh` 用十条精确 SQL 覆盖文本发布与精确修订投影，
不计入上述十二个 runner；十二条默认用例和三项新增公共守卫另行选择。
检查独立的 2 MiB 传输 / 256 KiB 解码文本限制、私有错误与当前授权、历史修订、响应正文名额
保持，以及文本/Task 独立预算。通用读取器可在投影前物化 1 MiB；C2-N 的 64 KiB 请求限制不变。
将返回引用用于另一次 Task 请求只是调用者控制的进程内夹具序列，不是原子批次、路由合并或
浏览器测试。源码守卫与默认传输测试不能替代显式一次性数据库执行；实际分日期结果记录于
[项目状态](project-status.md)。

独立的 **2026-10-11 未发布 C2-Y** 目标 `catalog_task_content_http_tdd` 包含 **28 条默认用例**，
`scripts/test-platform-catalog-task-content-http.sh` 精确选择 **八条 SQL 用例**，不计入上述十二个
runner。默认用例由一条漏引用、三条精确引用、八条目录正文，以及在真实目录路由上复用的
六条传输、十条准入用例构成。三项 `catalogTaskContentHttp` 公共守卫核对源码边界和精确清单；
这些是选择库存，不是通过记录。

SQL 覆盖组合包/版本与评估绑定元数据经过四个命令、双向手动/目录/所有者隐私、完整定义漂移和
未知坐标、旧新字节、当前 Session/账号生命周期、写后/提交故障、已观察锁等待，以及响应正文/
取消容量。拒绝快照比较**七张 Task/Artifact 资源表**的全部行和私有目录全部文件名/字节，
并非认证/身份/审计所有表。路由保留 C2-N 的 64 KiB / 两秒正文边界，但使用独立的强类型状态族；
分别配置的手动/目录/文本预算相加，不是全局配额。
未知定义在已调用 Session 命令后返回通用 `400 invalid_request / unconfirmed`，不提供 HTTP
回滚保证。源码/默认检查和旧手动 SQL 结果都不能替代该适配器的一次性数据库执行。它仍未挂载，
不自动合并路由、不发现目录、不让客户端选择模式、不调用评估提供器、不接日志 P–W，也不保存
浏览器草稿。契约见 [C2-Y 文档](catalog-task-content-http.md)，分日期结果见[项目状态](project-status.md)。

0.5.3 C2-P 另加 `scripts/test-session-task-draft-publication.sh`，精确选择 12 条用例，不计入上述十二个 runner 总数。
它在一次性存储上准确选择 `session_task_draft_publication_tdd` 的仅创建流程用例；默认准备/状态
测试不能替代这些 SQL 运行。分日期执行结果仍记录于[项目状态](project-status.md)。

独立的 **2026-10-11 未发布 C2-Z** 目标 `session_catalog_draft_publication_tdd` 选择
**四条默认准备用例**；内部 `catalog_draft_publication/tests.rs` 选择**八条受控回执/取消用例**。
六条 `sessionCatalogDraftPublication` 守卫检查不透明类型、两个共享纯 helper、源码接线及精确
库存。这些选择都不是新增通过记录，也不能替代数据库/文件执行。

`scripts/test-session-catalog-draft-publication.sh` 选择**八条不同的精确 SQL 用例**：behavior 两条、
faults 三条、lifecycle 一条、waits 两条，不计入上方十二个 runner 总数。它们覆盖组合包/版本和
有绑定/无绑定定义、有序/空计划、原 Session 替换、有效故障计数器、完整定义/manifest 确认、
已确认字节损坏、预分配 Artifact/Task 冲突但不接管，以及每步当前退出/过期/成员状态。
wait 夹具必须先证明 PostgreSQL 阻塞，再在 Artifact 与最终 Task 两类步骤测试取消或数据库
时钟 Session 过期。

快照比较覆盖七张 Task/Artifact 资源表全行与私有目录文件名/字节，不涵盖所有 auth/identity 表。
需区分拒绝时不变快照与第二次 Artifact 失败夹具中准确新增的孤立 blob：之前已提交前缀保留，
SQL 不接管失败目标，未知 blob 不删除也不重试。冻结定义和 `Some` 确认不认证准备、不评估
provider，也不让旧手动检查点/日志/观察 API 接收此尝试。参见[C2-Z 契约](session-catalog-task-draft-publication.md)；
实际结果和未覆盖范围仍在[项目状态](project-status.md)中分日期记录。

独立的 **2026-10-11 未发布 C2-AA** 目标 `session_catalog_draft_observation_tdd` 选择
**四条默认用例**，检查 Ready/原 token 准入、准确 Artifact 和空 Task 读取错误、尚未 poll 的
观察及进度不变。内部 `catalog_draft_publication/observation_tests.rs` 选择**六条受控比较/准入
用例**；其中给定值损坏检查不是实际读取器验收。六条源码守卫检查当前读取器委托、共用不含正文的
DTO、不收编状态及精确库存，不执行数据库/文件测试。

`scripts/test-session-catalog-draft-observation.sh` 选择**八条不同的精确 SQL 用例**：behavior
三条、faults 两条、lifecycle 一条、concurrency 两条，不计入上方十二个 runner 总数。
它们区分合法注册定义/状态/内容变化与有效注入的同编号元数据、手动 `None` 及双向绑定损坏；
类型化 `InvalidRecord` 不得改成报告。还覆盖组合版本、有绑定/无绑定及有序/空内容、当前
Session/成员、字节损坏或丢失、缺失/他人目标及 Task 缺席时不检查 Artifact 命名空间的路径。
等待用例必须先证明真实 PostgreSQL 阻塞，再检查取消或数据库时钟过期。

观察不变快照比较选定来源、Artifact、Task Schema 中每张表的排序行 MD5，以及私有文件名、
长度、mode、SHA-256 和根目录 device/inode/mode。这是限定范围的摘要，不是原始行/字节逐字
相等或全部部署 Schema 审计。故障夹具先证明计数器/存储改写确实生效，再观察；Artifact SQL
回滚仍可能留下准确不可变 blob，资源数量另行测量，`NotVisible` 从不证明回滚。进行中写者
用例通过观察到的 outbox 屏障运行另一条显式命令，不伪造响应丢失；之后可见也不确认尝试的
先前写入。契约见[C2-AA 说明](session-catalog-task-draft-observation.md)，实际结果及限制见
分日期的[项目状态](project-status.md)。

0.5.3 C2-Q 使用独立精确 runner `scripts/test-session-task-draft-observation.sh` 与
`session_task_draft_observation_tdd` 目标。12 条只读用例与 C2-P 的 12 条创建用例分开：观察须保持
尝试状态，区分当前可见与过去提交确认。Task 缺席不要求检查 Artifact 命名空间，读取错误不能
转成缺席；这些用例也不计入上方十二个 runner 总数。

0.5.3 C2-R 检查点测试是默认纯契约/库内检查，不新增 PostgreSQL runner。须拒绝畸形/超限数据及
不一致的目标或进度，并使解析数据与活跃尝试保持分离。往返通过不证明崩溃耐久性，也不允许
重启后调用 C2-Q；存储与恢复须由后续独立测试验证。

0.5.3 C2-S 使用 `scripts/test-session-task-draft-inspection.sh` 验证独立检查入口，提供当前
Session 权限而非重建 C2-Q 原尝试。其精确数据库用例与 C2-R codec、C2-Q 观察套件分别选择；
元数据相符不能计作历史确认、原正文比较或安全重试依据。

0.5.4 C2-T 使用 `scripts/test-session-task-draft-intent.sh` 从 `session_task_draft_intent_tdd`
选择登记/读取用例，面向显式安装的一次性日志。全新 Schema/安装 ID、精确账号代次、重复冲突
和事务故障检查须与 C2-P 写入分开；认证源/登记/日志核验不验证原 Task/Artifact 代次。
记录确认不等于资源已派发，也不证明派发前持久化；数据库 COMMIT 设置不能代替宿主恢复或备份
回放测试。分日期执行结果记于[项目状态](project-status.md)，不合入上方原有 runner 总数。

0.5.4 C2-U 使用 `scripts/test-session-task-draft-dispatch.sh` 与 `session_task_draft_dispatch_tdd`。
检查不升级 Schema 1 的显式 Schema 2 准入、先确认 A 再派发 B 的顺序、授权变化、精确 nonce/
前缀、业务校验前的标记触发器及末尾只读日志复验。资源 SQL 与标记必须共享 B 的提交；回执未知
不能成为回滚证明或重试权限。原 C2-T 登记/读取用例独立保留，不能证明这些派发行为。

0.5.4 C2-V 使用 `scripts/test-session-task-draft-journal-inspection.sh` 从
`session_task_draft_journal_inspection_tdd` 精确选择十条 SQL 用例，另有两项默认测试和三项
公共守卫；分日期执行结果见[项目状态](project-status.md)。检查合法前缀、空 payload 仍需末尾 Task 计数、精确
所有者/账号可见性、Schema/身份故障、等待过期及取消，并保持资源和文件不读。记录缺失不证明
回滚，已记录进度不能恢复包装器或授权重试；C2-T/U 证据保持独立。

0.5.4 C2-W 通过 `scripts/test-session-task-draft-journal-observation.sh` 与
`session_task_draft_journal_observation_tdd` 精确选择十二条 SQL，另有两项默认用例及三项公共
守卫；分日期执行见[项目状态](project-status.md)。检查序号纯拒绝，区分缺失意图、未记录步骤和不可见
资源，覆盖原精确 Artifact 修订、编辑后的当前 Task 内容、类型化 blob 错误及资源读取后原完整
日志/身份复验。不返回正文仍会读取文件；等待/取消不能授予回执、写入或重试。C2-V 保持不读
资源，其既有执行不证明此独立路径。
外部写者阻塞不验证同一读取事务内篡改；首次快照复用由源码守卫约束。

独立的 `scripts/test-prepared-resource-sql.sh` 另选择两条库内用例，检查 Task/Artifact/Review 共用
机制的普通表拒绝及解码顺序，以及单一池连接上的读写事务设置和回滚/drop 后局部超时恢复。
`architectureBoundaries.test.mjs` 检查精确清单与 CI 接线；这两条不包含在上述十二个 runner 总数内，
也不替代各存储及 Session 回归。

截至 2026-10-07 到期时间解码改动，`scripts/test-durable-session-boundary.sh` 精确选择
**17 条来源层 Session 用例**（原 14 条加三条时间用例），覆盖
歧义/临时命名空间、过滤或不兼容关系、写后路径/关系替换、元数据变化、登录两段事务身份以及
退出/过期锁顺序，以及会话验证、登录写后读回和改密守卫中的坏值/有限边界到期时间，不需要
协作注册表。`durableSessionBoundaryCi.test.mjs` 检查其清单；它与上述
212 条资源用例、原 `durable_auth_source_tdd` 的 14 条 SQL 分开计数。入口数量不证明全部已重跑，
实际执行见带日期的项目进度。

`scripts/test-durable-reconciliation.sh` 现精确选择 **17 条用例**：原十二条生命周期检查、
两条 Session 到期用例，以及 2026-10-07 随后增加的三条注册表时间用例。Session 检查只取 SQL
过期布尔值；注册表用例覆盖准确错误阶段、有限审计时间和 Session 授权绑定被拒绝。runner 执行前
核对准确名称；这些是源码选择数量，不是新的通过结果，原 C1-K 历史结果不改写。

2026-10-07 注册表后续新增 **10 条独立 SQL 用例**：身份存储两条、身份审计两条、策略审计三条、
对账三条；对应完整套件库存分别为 **17、18、19、17 条**。`scripts/test-registry-timestamps.sh`
只是这十条的精确专题入口，不能在完整套件之上重复累计；CI 通过维护的全套选择清单收录它们。
范围防护只针对三个存储注册表列，不代表任意时间字段安全或新增墙上时钟排序规则。

例如，在一次性 URL 已配置后运行：

```bash
bash scripts/test-session-task-revisions.sh
```

记录实际执行的每个套件，不只写总数。结束后停止这次创建的准确 PostgreSQL 实例，仅清理核实过的
夹具目录。失败或取消不证明事务回滚，应检查测试断言和保留证据。同样，Artifact 发布或 Task 换版
失败不构成自动删除残留内容的许可。

## 浏览器与传输套件

### 可选的独立宿主 Session 网页

原生宿主尾部的 `--session-ui` 参数在 `/platform/` 提供七个固定资源，含 `tasks.js`、
`task-edit.js` 与 `task-contract.js`，默认关闭。五语言私人 Task 页只读正文，并确认手动 Draft
标题／显示标签保存，不改引用／顺序／字节／条数。不接原文修改、新 Artifact、创建、Run 写入、
正常 Next 本地草稿／SDK、认证 API／权限或教学认证回退。以下显式测试均需绝对路径
`CYANREX_PLAYWRIGHT_MODULE` 与 `CYANREX_CHROMIUM_PATH`：

```bash
bash scripts/test-platform-session-ui-browser.sh
bash scripts/test-platform-task-reader-browser.sh
bash scripts/test-platform-task-metadata-browser.sh
# 另需显式一次性 PostgreSQL URL、OpenSSL 和 certutil：
bash scripts/test-platform-browser-session.sh
# 只读 Task 路径使用真实明文宿主，不证明 TLS 浏览器验收：
bash scripts/test-platform-browser-tasks.sh
bash scripts/test-platform-browser-task-metadata.sh
```

Session UI runner 使用真实 Chrome、生产资源与 CSP，但 **Session API 为 mock**，检查确认锁、
有界响应、未知结果不重放写入、五语言和 360px 触控布局。恢复事件由合成的 persisted
`pagehide/pageshow` 触发，不代表真实 BFCache 导航验收；空帧及迟到响应用例明确使用 fetch 替身。
Session SQL runner 精确选择两条 ignored 用例：真实明文／原生 TLS 浏览器登录、Cookie 回读、取消与
确认退出，独立核对 Session／OTP／账号以及 Task／Artifact 资源不变。明文用例另注入真实的
退出删除权限失败，检查 Cookie／Session 保留；手动检查状态后仍须再次显式确认退出。

Task-reader 夹具列举 12 条真实 Chrome、**mock API** 用例：显式每页 20 项、惰性纯文本／空
payload／未知语言、有序引用与摘要、畸形／超限响应、撤权形状错误、Session 变化、合成页面
生命周期及迟到代次拒绝。这些 UI 断言不证明 SQL 权限。公共套件中的
`platformTaskReaderContract.test.mjs` 另有 20 条纯 Node 用例，核解析前重复／转义键、深度／token、
安全数值、准确作用域／修订／清单／内容、UTF-8 上限和本地 SHA-256。读取限制为索引报文
128 KiB、内容报文 50 MiB、16,384 帧及解码后 32 × 256 KiB；十秒等待不抢占同步解析或服务端
工作。Task 401／403、Session 检查、确认退出开始派发及 pagehide 必须清旧数据，迟到响应不能恢复。

Task SQL／浏览器 runner 精确选择一条 ignored 明文宿主用例，经真实当前权限验证私人分页／
重开与撤权，同时核 Task／Artifact 无写入。库存不等于选择已通过，也不证明 TLS 浏览器、
局域网、编辑或保存验收。

Unreleased 显示信息夹具列举 12 条 mock API 浏览器用例：默认取消的预览确认、单次 PUT、严格
HTTP 200 回执、空 Draft／非 Draft 边界、本地校验、dirty／未知导航守卫、冲突不重放、畸形／
超限／丢失响应，以及失权／pagehide 隔离且不跨 Session 恢复。预览／私人 DOM 清空，受控迟到
ACK 不能恢复尝试。`platformTaskMetadataContract.test.mjs` 另列举 20 条纯 Node 用例，核仅改
显示信息的描述和严格三键回执：安全修订加一、同 Task／owner／创建时间、不改有序引用，
不重新 hash 或宣称来源证明。请求／回执为 64 KiB／128 KiB，回执最多 4,096 帧，浏览器等待
十秒，不是解析／服务端硬期限。元数据保存后正文仍是先前未变副本，不证明新鲜读取。

显示信息 SQL／浏览器 runner 精确选择一条 ignored 明文宿主用例，核确认元数据、竞争修订冲突
和撤权，并比较保留 Artifact 文件及不变的引用／顺序／条数。它与 mock UI、原文保存和 TLS
浏览器验收分开，库存不构成通过结果；UI 不增加数据库权限或 HTTP API。

TLS 浏览器验收需 Chrome 146+，且 **不能已有旧 `~/.pki/nssdb`**：Chromium 会优先使用该库，
而非夹具的私有 XDG NSS 目录。遇到此环境必须拒绝，不得删除或修改用户信任库、覆盖 HOME，
或跳过证书校验。合适的一次性工作机先验证不信任测试 CA，再只将该 CA 导入自己临时 NSS 库，
重新启动浏览器并正常校验证书。本机旧 NSS 阻塞该浏览器 TLS 用例，原生 TLS socket 回归不能
替代。CI 通过 `scripts/test-platform-browser-ci.sh` 安装临时固定版本浏览器工具后运行三个 mock
及三个 SQL／浏览器 runner；接入不等于远端 CI 已通过。默认 Rust／源码检查、mock UI、真实 SQL／浏览器和真实
TLS 结果须在[项目进度](project-status.md)中分开记录。

### 2026-10-26 本地显示信息保存验证

未提交的 0.6.3 工作树通过 `quality-gate.sh --backend-only`：636 项公共检查、1451 项默认
Rust 测试通过，1030 项忽略，无警告。三个 mock API Chrome 套件各通过 12 项；另行通过
13 条精确一次性 SQL 选择：新增显示信息浏览器用例、原明文 Session 和只读 Task 浏览器各一条，
以及十条 Task 换版用例。独立存储核对确认仅推进一次修订／事件、Artifact 文件不变，旧修订
不能覆盖新版本，撤销后的 Session 不能写入。显示信息契约 20 项已含在公共数量中；另一个仍未
挂载的内容保存帮助层通过 36 项单测，不代表浏览器原文保存验收。保持拒绝正文未结束的响应头
用例在修复清屏前实际失败、修复后通过；合成生命周期和受控迟到 fetch 仍保留各自限制。
已检查桌面／360px 截图；浏览器 TLS 仍受上述环境限制，CI 接入不是远端通过结果。
仅 socket 的 PostgreSQL 18 夹具已停止，56 MiB 生成数据已删除，日志及截图留在仓库外。
本切片未改版本、提交或推送。

依赖 Next 页面的套件使用**生产前端构建**，构建时令 `NEXT_PUBLIC_ENGINE_URL` 指向纯夹具地址；测试时设置匹配的
`CYANREX_UI_ENGINE_URL` 和临时前端的 `CYANREX_UI_BASE_URL`。默认无法找到运行时或浏览器时，
再指定 `CYANREX_PLAYWRIGHT_MODULE`、`CYANREX_CHROMIUM_PATH`。执行前阅读该套件 helper：这些
变量和夹具不意味着连接真实后端。部分 WebSocket 夹具会干扰开发 HMR，因此应使用生产模式。

本地 payload/编辑器边界可执行：

```bash
npm --prefix frontend run test:task-payload-browser
npm --prefix frontend run test:multi-language-editor-browser
npm --prefix frontend run test:editor-intelligence-browser
```

payload helper 使用真实页面和 Monaco，仅允许模拟的身份/未读读取，拒绝 payload 网络写入及
WebSocket。这证明本地行为边界，不是服务端保存测试。其它浏览器套件会模拟认证、课堂、学习、事件、
设置、Runner 管理和安全交互的 Engine 路由。当前不存在能证明尚未实现的通用 Task HTTP 保存闭环的
浏览器套件。

### 0.5.3 的生成 Monaco 清理器夹具

DOMPurify 后续改动为默认 `test:tooling` 增加资源生成边界单测，并将公共离线依赖守卫扩展到
DOMPurify 3.4.16 下限，与 sharp、source-map-js 一并检查。单测核对受支持版本、原始 chunk 摘要
和语法结构、官方 ESM 的隔离包装、第三方代码块以外字节不变、确定性新 chunk URL 及引用改写。
不支持的输入必须直接失败，测试不授权修改 `node_modules` 或悄悄升级 Monaco；联网生产 npm
审计仍独立执行，沿用原阈值。
工具单测另注入准备及补丁写入失败，检查 `public` 之外的私有暂存和发布前原资源保留；不声称
最终发布具备原子性。
公共 Docker 输入守卫另要求从依赖阶段取得生成资源，并从构建上下文排除宿主机 Monaco 资源目录；
此源码检查不是实际 Docker 镜像构建。

安装依赖与锁文件一致且 Playwright/Chromium 可用时，从仓库根目录先生成本地资源，再显式选择
浏览器夹具：

```bash
node frontend/scripts/sync-monaco-assets.mjs
npm --prefix frontend run test:dompurify-browser
```

原 0.5.3 夹具包含十项用例，其中九项通过测试插桩取得实际生成 AMD chunk 中 Monaco 使用的同一
DOMPurify 实例，不只检查根 npm 包。覆盖修复版本、安全格式保留及可执行标记移除、元素处理前后和属性处理后
的移除钩子（含保留的自定义元素）、rawtext 根节点拒绝、健康根节点身份，以及真实 Monaco hover
渲染。测试插桩不写入生成的生产资源。

第十项加载未修改的 AMD loader 与编辑器入口、新摘要 chunk 和语言注册图。发往合成来源
`monaco-fixture.invalid` 的请求全部被拦截，只由本地生成文件提供响应；拒绝意外的来源或路径。
前九项拒绝所有网络请求；全部用例均不产生外网访问、不启动 Next 或 Engine。可使用上方相同的
Playwright/Chromium 路径配置，但不要求生产前端构建或 Engine URL。这是组件/浏览器回归，
不是应用可利用性、已部署清理器版本或端到端验收的证明。浏览器执行为 opt-in；默认前端门禁运行
纯工具单测，不包含这十项浏览器用例。用例库存不等于通过结果，分日期执行结果仍见
[项目进度](project-status.md)。

### Unreleased 净化器回归后续验证

当前契约要求 DOMPurify 3.4.17。相同的 `test:dompurify-browser` 入口保留原用例，并增加不同 DOM
输出模式下 attached shadow root 的模板重组、钩子移除父节点或尚未遍历的兄弟节点、XHTML SMIL
目标属性。安全文本／动画对照和 shadow root 存在性断言防止空结果误通过。这些测试使用实际生成的
Monaco 实例，阻止外网请求，不求值模板表达式，也不验证 Next.js 漏洞。须先按上文重新生成匹配资源；
用例仍需显式执行，不代表已部署环境验收。

真实传输另行验证。ignored 的 `runner_agent_client_tdd` 用例启动回环 HTTP 服务并使用支持 BPF
目标的 Linux Clang，只编译、不挂载程序。事件流压力夹具检查真实节奏发送、过载和阻塞连接。
两者都不证明真实局域网浏览器/教师/学生流程、TLS 部署或 VM 隔离。

## 真实与发布验收

`test-live-kernel-smoke.sh` 使用模拟；**`live-kernel-smoke.sh` 是另一回事，会进行真实特权操作**。
只有在明确指定的一次性 Linux 环境中，审阅目标、凭据、空挂载要求和清理流程后才能运行后者。
它把观测到的内核事件绑定到本次程序、卸载精确 pin 并拒绝残留。候选验收证据必须匹配所声明的包
元数据、修订和镜像内容。

同样，`test-distribution-tools.sh` 不是安装，而 `distribution-install-smoke.sh` 可能启动真实服务栈。
SSH 夹具通过也不授权远程 apply。执行这些操作前阅读[离线部署与发布验收](../../docker/README-DEPLOY.md)、[验收说明](acceptance.md)
和[课堂连接](classroom-connection.md)。真实特权运行、模拟工具检查、依赖公告审计和 benchmark
必须分别记录，不能混为一种证据。

## 证据与维护

未发布的 **C2-L 存储验证**通过 436 项默认 Rust（忽略 416 项）、111 项公共脚本，另通过 176 项
精确 PostgreSQL 用例：新增 21 项及此前 155 项资源回归。数据库为全新私有 socket PostgreSQL 16
实例，不是部署服务；没有执行内容 Session/浏览器适配或迁移。清理与限定范围见项目进度对应日期记录。

未发布的 **C2-K 纯契约验证**通过 428 项默认 Rust（忽略 395 项）、110 项公共脚本，含 26 项新增
元数据/快照回归；没有重跑 PostgreSQL 或浏览器测试。

此前的 **C2-J 后端开发验收**通过 402 条默认 Rust、92 条公共脚本和原有 155 条资源数据库
用例；此前的**任务 payload/编辑器验收**通过 164 条默认前端及 35 条浏览器用例。它们是
2026-10-03 分别进行的限定范围验证，不能相加后宣称新的一次端到端验收。后端该轮未重跑全部
认证源/生命周期数据库套件，浏览器 mock 也不能证明真实服务端保存。完整限定和此前各阶段见
[项目进度](project-status.md)。

每次新执行都应保留日期、源码修订/工作树状态、选中测试、精确环境、通过/失败/忽略数、夹具边界和
清理状态；重复执行同一用例不增加覆盖。保留[历史测试网络](testing-network.md) 及其报告，不改写
旧失败、数量或源码指纹。runner、边界或测试环境变化时更新本指南，并同步[英文版](../en/testing-guide.md)。
