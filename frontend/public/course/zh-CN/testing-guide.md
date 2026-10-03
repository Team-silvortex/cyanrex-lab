# 当前项目测试指南

本文按 **2026-10-03 的源码版本 0.5.0** 核对，包含任务 payload、Session 命令与独立内容 HTTP 适配。
它说明各模块和边界应测什么、每层通过能证明什么，以及如何避免使用线上数据。本文是测试方案和
源码清单，不是一次新的测试结果。先用[平台链路地图](platform-network.md) 区分已连接与未连接路径。
[功能张量](capability-maturity.md)将实现切片关联到测试源码与分日期证据；公共脚本检查其结构、引用
及中英文评分表一致性，不执行被引用的产品测试，也不建立部署成熟度。

## 测试分层

| 层次 | 维护入口 | 通过能够证明 | 不能证明 |
|---|---|---|---|
| 公共源码和工具检查 | [`quality-gate.sh`](../../scripts/quality-gate.sh)、[`scripts/tests`](../../scripts/tests) | 长度、版本、课程同步、公共 API/SDK 契约及脚本/合成工具夹具回归 | 实际部署流程、真实 SSH/Docker/内核验收 |
| 默认 Rust 测试 | `cargo test --manifest-path engine/Cargo.toml --locked` | 模型、服务、进程内路由、默认集成夹具与编译期契约 | 显式忽略的 PostgreSQL、真实 Agent 传输和性能用例 |
| 显式 PostgreSQL 测试 | CI 的精确用例与下方存储 runner | 一次性数据中的真实 SQL 事务、命名空间/schema、锁、竞争、取消和故障注入 | 浏览器集成、部署数据迁移或线上授权切换 |
| 前端构建与单元测试 | 前端质量门禁和 [`frontend/package.json`](../../frontend/package.json) | 生产构建、TypeScript、状态/请求/权限/编辑器回归 | 真实浏览器渲染或真实 Engine 连接 |
| 浏览器回归 | 显式 `test:*-browser` 脚本 | 真实 Next 页面、交互、Monaco 模型/worker 与夹具定义的请求行为 | Engine 被模拟时的真实服务授权、数据库/内核端到端链路 |
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
| 事件和设置 | `module_boundaries_tdd`、EventBus、显式事件 SQL、事件/设置单元与浏览器套件 | 发布顺序 → 持久化历史 → 重新同步、导出/删除筛选安全、确认设置写入和读取失败 |
| 通用身份与权威 | `collaboration_*_tdd`、`legacy_workspace_projection_tdd` | 审计绑定/成员/授权修订、缺席键竞争、最后管理者、角色与部署权限分离 |
| 持久化认证源与生命周期 | `durable_auth_source_tdd`、`durable_collaboration_tdd`、删除/改密/初始化/核对测试目标 | 账户世代 → 精确当前 Session → 同事务身份/策略、命名空间替换、写后事实 |
| 运维初始化 | `provision_cli_tdd`、`provision_postgres_tdd` | 只读计划、目标绑定 apply、私密配置交付、确认丢失与取消；不收编现有配置 |
| 通用 Task、Artifact 与 Review | 下方显式存储 runner 及对应默认 Rust 测试 | 精确修订/摘要、Task/outbox 原子性、不可变文件边界、私人所有权、Review 历史、当前 Session 检查 |
| 本地任务 payload 与编辑器 | `test:editor-languages`、`test:task-payload-browser`、`test:multi-language-editor-browser` | 可选 payload、已接受内容/修订、导入导出、过期编辑、模型释放与禁止网络写入 |
| 内部任务内容元数据 | Rust `task_content_contract_tdd`、`task_content_binding_tdd` | 严格对象结构、字段/数量上限、有序准确引用、给定所有者/字节/摘要一致性；默认纯测试，不验证存储或 Session 授权 |
| 独立内容存储 | Rust `task_content_store_tdd`、`scripts/test-task-content-storage.sh` | Schema 2/3 隔离、完整元数据/outbox 原子性、修订冲突与存储故障；不是 Session 内容或浏览器适配 |
| 会话任务内容 | Rust `session_task_content_tdd`、`scripts/test-session-task-content.sh` | 同一事务内当前授权、旧新精确文本、元数据编辑、移除、命名空间身份及写后复验；不是公共/浏览器验收 |
| UI 导航与安全 | `test:ui-permissions`、布局/动作/账户/运行浏览器套件 | 绑定目标的确认、防重复操作、路由切换、键盘操作、未保存草稿保护 |
| API、SDK、打包与部署 | 公共契约测试、SDK 门禁、发布/SSH Rust 目标与工具夹具 | Engine 路由 → OpenAPI → 生成 SDK；归档来源 → 精确已审阅 SSH 目标；夹具与实际安装分离 |

`authSession` 等简写指 `frontend/tests` 中的对应文件；精确 npm 脚本名以 `frontend/package.json`
为准。Rust 精确筛选前先执行 `--list`，避免名称拼错后“零条测试通过”。存储 runner 自带这一步检查。

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
| `scripts/test-session-catalog-tasks.sh` | 16 | 精确准入定义元数据和 0–32 个输入 |
| `scripts/test-session-task-revisions.sh` | 17 | Draft 换版、旧/新输入并集、schema 2 兼容性与并发保存 |
| `scripts/test-session-task-content.sh` | 25 | Schema 3 当前 Session 内容、精确文本、旧新复验、命名空间替换、生命周期与并发故障 |
| `scripts/test-platform-task-content-http.sh` | 11 | 独立 HTTP → C2-M、私有归属、等待锁时 Session 过期、精确文本、提交/outbox 故障与并发编辑 |

十二个 runner 共选择 **212 条用例**（此前资源 155 条、内容存储 21 条、会话内容 25 条、独立 HTTP 11 条），不是仓库全部 PostgreSQL 覆盖。旧认证/事件/脚本/学习与通用
身份/认证源/生命周期/初始化用例在 CI 中另有精确清单。
[`postgresCi.test.mjs`](../../scripts/tests/postgresCi.test.mjs) 检查 runner 清单完整性。

例如，在一次性 URL 已配置后运行：

```bash
bash scripts/test-session-task-revisions.sh
```

记录实际执行的每个套件，不只写总数。结束后停止这次创建的准确 PostgreSQL 实例，仅清理核实过的
夹具目录。失败或取消不证明事务回滚，应检查测试断言和保留证据。同样，Artifact 发布或 Task 换版
失败不构成自动删除残留内容的许可。

## 浏览器与传输套件

使用**生产前端构建**，构建时令 `NEXT_PUBLIC_ENGINE_URL` 指向纯夹具地址；测试时设置匹配的
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
