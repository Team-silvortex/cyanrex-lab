# 教学运行时与部署参考

[English](../en/runtime-reference.md) · [文档入口](README.md) · [项目首页](../../README.zh-CN.md)

本参考用于现有教学应用，集中说明源码启动、离线安装、教师账户、Engine API 和发布验证。
除明确进入解压包目录的章节外，命令均从仓库根目录运行。

独立低权限平台宿主另有[启动指南](platform-loopback-host.md)。启动教学应用不会挂载所有平台
适配器、迁移协作存储或连接浏览器任务保存。当前能力边界见[项目状态](project-status.md)。

## 快速开始

完整文档入口：[Cyanrex 项目文档](README.md)。现有教学流程中，教师建议先阅读
[教师快速开始](teacher-guide.md)，学生从
[学生快速开始](student-guide.md)进入课程。

### 1) 启动服务

```bash
./start.sh
```

常用命令：

```bash
./start.sh start --mode auto   # WSL2-native inside WSL; Docker elsewhere
./start.sh start --mode wsl    # Engine targets the WSL2 kernel
./start.sh start --mode docker # Engine targets the Docker host/VM kernel
./start.sh start --mode native # Engine targets the native Linux kernel
./start.sh start --local
./start.sh start              # fast-start (default, no forced rebuild)
./start.sh start --debug      # safe debug notice; commands are not traced
./start.sh start --rebuild    # force rebuild when deps/Dockerfile changed
./start.sh start --pull       # pull base images before start
./start.sh start --skip-conflict-check  # skip preflight port/project conflict check
./start.sh start --instance-id room-a    # same instance starts are auto-locked by instance ID
./start.sh start --skip-start-lock       # bypass start-lock when debugging only
./start.sh diagnose          # run system-level diagnostic checks
./start.sh status
./start.sh logs
./start.sh stop
```

eBPF Engine 始终运行在 Linux 内核上。在 Windows 中，可使用 WSL2-native 模式直接访问 WSL 内核，或使用 Docker 模式构建可丢弃的教学沙箱。Docker 模式有意使用特权运行，不能暴露给不可信用户。

### 2) 打开 UI

- 前端：`http://localhost:3000`
- Engine 健康检查：`http://localhost:8080/health`
- Postgres：`127.0.0.1:15432`（不暴露到局域网）

现有教学流程使用 `/ebpf` 和 `/learn`。在当前开发树中，`/tasks/new` 打开本地 Task 载荷编辑器（`/editor` 是兼容入口）。依赖持久化前请阅读[编辑器指南](editor.md)：它不会保存到服务端。启动应用不会初始化或迁移独立的协作存储。

### 2.5) 构建分发包

用于课堂部署或离线分发时，可创建包含预构建 Docker 镜像的打包产物：

```bash
package_version="$(awk -F'"' '/^version = / { print $2; exit }' engine/Cargo.toml)"
./scripts/package-distribution.sh --version "$package_version"
```

该命令产生：

- `dist/cyanrex-lab-<version>-<timestamp>.tar.gz`
- `dist/cyanrex-lab-<version>-<timestamp>.tar.gz.sha256`

归档包含 PostgreSQL、Engine 和前端镜像。在可丢弃的 Docker 宿主上，先进入刚解出的包目录，再用 `./install-smoke.sh` 进行端到端验证。它检查包清单与 `release-metadata.json`，在禁用镜像仓库拉取的情况下加载离线镜像，生成临时密钥，启动所有必需服务和可选编译 Agent，执行已认证探测，然后删除临时栈和卷。发布元数据记录源码修订及 clean/dirty 状态、存在时匹配的带注释版本 Tag、镜像引用/内容 ID/构建模式以及镜像归档 SHA-256，不暴露构建宿主路径。安装验收固定包内镜像引用，并在探测服务前验证全部三个已加载 Docker 内容 ID。元数据绑定校验和，但未签名。冒烟测试拒绝覆盖已有 `.env`。安装验收使用随包提供的 Rust CLI 验证包、检查已加载镜像身份并检查 Engine 健康状态；默认打包路径不再要求 Docker 宿主安装 Python。
仅在可丢弃的特权 Linux 宿主上设置 `CYANREX_SMOKE_RUN_LIVE_KERNEL=1`，以额外运行包内的 `live-kernel-smoke.sh`：它附着内置 Aya `sched_switch` 环形缓冲程序，要求一个唯一标识的真实内核事件，分离精确 pin，并拒绝残留附着。新包通过相邻原生 `cyanrex-release smoke live-kernel` 客户端运行该流程；脚本保留 shell/Python 路径，以兼容旧包或手工组装的包。
设置 `CYANREX_KERNEL_SMOKE_REPORT=/safe/output/report.json` 可原子写入机器可读证据；脚本拒绝覆盖已有报告。

之后在同一个解包目录中，针对该包的精确元数据验证报告：

```bash
./cyanrex-release evidence verify /safe/output/report.json \
  --release-metadata ./release-metadata.json
```

新的离线包携带此原生 Rust 验证器，不携带独立 Python 发布辅助工具。Python 实现在源码 checkout 中保留，作为兼容和跨实现参考。

对于从 Tag 工作流下载的产物，将其四个文件放入专用目录，并在解包前，从匹配源码 Tag 的可信 checkout 验证完整候选包：

```bash
release_tag="v<version>" # 选择准备安装的准确注释 Tag
release_version="${release_tag#v}"
release_revision="$(git rev-list -n 1 "$release_tag")"
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  candidate verify /path/to/downloaded-candidate \
  --expect-version "$release_version" --expect-revision "$release_revision" --expect-tag "$release_tag" \
  --extract-to /path/to/new-output-directory
```

该过程以流方式读取归档而不解包，拒绝链接、路径穿越、重复成员和未纳入校验和的载荷，再将精确归档元数据绑定到独立带校验和的实时内核证据。只有整个候选包通过后才开始解包；只向私有临时目录写入普通文件，保留脚本执行位，并拒绝已有输出路径。在配置发布签名之前，这些检查证明的是包的一致性，而非发布者真实性。

对于没有 Tag 验收报告、在本地构建的双文件包，可从可信源码 checkout 使用相同的已验证解包路径，但省略证据步骤：

```bash
cargo run --quiet --manifest-path engine/Cargo.toml --locked --bin cyanrex-release -- \
  package extract /path/to/archive-and-checksum \
  --output /path/to/new-output-directory
```

原 `scripts/release-package.py` 入口暂时保留以提供兼容。

目标机器上的用法：

```bash
cd /path/to/new-output-directory/cyanrex-lab-*
cp .env.example .env   # update credentials/bind settings as needed
./run.sh               # start
./stop.sh              # stop
```

包现在包含功能更丰富的 `deploy.sh` 辅助工具：

```bash
./deploy.sh up     # start (default behavior)
./deploy.sh status # show service status
./deploy.sh logs   # follow logs
./deploy.sh down   # stop services
```

`run.sh` 和 `stop.sh` 保留为兼容包装器。

如果 `.env` 缺失或仍含占位值，启动会失败，并明确提示先初始化密钥。

### 3) 私有部署教师账户

- 用户名：`admin`
- 角色：`teacher` — 个人使用时你就是教师；教学与部署共用同一身份
- 密码和 TOTP 密钥：首次启动时在 `docker/.env` 中生成
- `docker/.env` 模式为 `0600`，且被 Git 忽略；请勿发布它

首次启动前可通过环境变量覆盖：

- `CYANREX_ADMIN_USERNAME`（直接启动 Engine；受支持的启动器保留用户名 `admin`）
- `CYANREX_ADMIN_PASSWORD`
- `CYANREX_ADMIN_TOTP_SECRET`
- `CYANREX_ADMIN_USERNAMES`（旧式可选教师权限允许列表，用逗号/空格分隔）
- `CYANREX_TEACHER_USERNAMES`（为已验证现有账户授予完整教学/部署权限的可选配置）
- `CYANREX_ALLOW_MISSING_ORIGIN`（可选，默认禁用）— 允许受 CSRF 保护的状态变更路由没有 `Origin`/`Referer`
- 事件持久化调优（可选）：
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED`（默认 `true`）
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT`（默认 `80`）
  - `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT`（默认 `40`）
  - `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS`（默认 `10000`）

注册和仅凭密码的 TOTP 初始化默认禁用。仅在有监督的实验环境中设置 `CYANREX_ALLOW_REGISTRATION=true`。核心 eBPF 和用户脚本供已认证用户使用。教师管理模块、编译器设置、Runner Agent、教学与 Terminal；学生不能使用这些管理路由。现有管理员账户/配置保持兼容，但登录和 Session 角色现在报告为 `teacher`。

单人使用只需要预置教师账户，不需要公开注册或管理员开关。公开注册拒绝保留的教师/旧管理员名称，且不能选择角色。部署教师不能删除自身。升级前请审阅两个允许列表：原教师现在获得完整部署权限。请一起升级前端和 Engine；任命、所有权和隔离限制见[教师指南](teacher-guide.md)。

模块目录通过 v1 `module.json` 清单加入发现。Engine 启动时拒绝格式错误的清单，生命周期请求拒绝未知名称。启动/停止只更新单进程控制状态；Cyanrex 从不执行模块目录中的文件。在仓库或打包镜像之外运行时，可使用 `CYANREX_MODULES_DIR` 覆盖目录根。

## 认证 API（已实现）

- `POST /auth/register`
- `POST /auth/login`
  - 响应包含 `role`（`teacher` / `student`；`admin` 保留为旧 schema 值）
- `POST /auth/totp/bootstrap`
- `GET /auth/me`
  - 响应包含当前已认证 `role`
- `POST /auth/logout`
- `POST /auth/password/change`（需要认证 Session + OTP）
- `POST /auth/delete`（需要认证 Session + OTP）

## eBPF API（已实现）

- `POST /ebpf/check`
  - 仅编译的 clang 语法检查；从不向内核加载代码
  - 返回结构化行/列诊断
- `POST /ebpf/complete`
  - 在从一开始计数的光标位置执行 clang 语义补全
  - 返回头文件符号、宏、类型、函数与结构体字段
- `POST /ebpf/run`
  - 接受可选 `program_name`、`template_id` 和 `lab_id`
  - 接受可选 `runtime_backend`（`bpftool` | `aya`）
  - 支持用 `sampling_per_sec` 控制内核事件采样率
  - 支持用 `stream_seconds` 控制流持续时间
  - 支持 `enable_kernel_stream` 开关
  - 内核流优先使用 `ringbuf event_pipe`，回退到 `tracelog`
  - `aya` 后端当前面向 tracepoint 附着路径（对 sched switch 采样器提供一等支持）
- `GET /ebpf/attachments`
- `GET /ebpf/attachments/details`
- `POST /ebpf/detach`
- `GET /ebpf/templates`
  - 包括典型模板：`xdp`、`tracepoint`、`ringbuf skeleton`、`ringbuf high-freq sampler`
- `GET /events`
  - 事件快照
- `GET /ws/events`
  - 实时事件流
- `GET /settings/events`
  - 获取当前用户事件保留设置
- `POST /settings/events`
  - 更新用户事件保留设置（`max_records`、`overflow_policy`）
- `GET /helper/environment`
  - 运行时检查包括 `bpftool_autoattach`、`bpftool_link_show`、`btf_dump`、`bpffs_mount_type`、`runtime_context`

## Runner API（已实现）

- `GET /runner/status` — 已认证容量与显式隔离级别
- `GET /runner/overview` — 仅教师可见的活跃租约所有者与期限
- `POST /runner/agent/register` — 可选的 token 认证远程节点注册
- `POST /runner/agent/heartbeat` — 已注册节点的健康与容量心跳
- `GET /runner/agents` — 仅教师可见的远程节点清单
- `POST /runner/jobs/probe`、`/runner/jobs/compile-check`、`/runner/jobs/cancel` — 仅教师可用的远程任务生命周期
- `POST /runner/agent/jobs/claim`、`/sync`、`/result` — 已签名的 Agent 任务协议
- `GET /runner/jobs` — 仅教师可见的任务清单
- `GET /ebpf/check/backends` — 已认证、已脱敏的本地及合格 Agent 编译器清单
- `POST`、`GET /ebpf/check/remote` 和 `POST /ebpf/check/remote/cancel` — 用户范围内的异步远程诊断
- `/ebpf/run` 通过可替换的 `RunnerDriver` 边界执行
- 本地 Runner 默认最多两个全局任务、每用户一个任务，执行超时为 45 秒
- 显式报告 `shared_kernel`；配额不能代替每学生 VM 隔离
- Agent 任务使用逐节点 HMAC 凭据、防重放、租约、能力匹配、取消和有界结果
- 远程诊断需要编辑器显式选择，从不静默回退；本地检查仍为默认
- 可选编译任务使用受限 Clang，从不加载或返回 eBPF 对象；`/ebpf/run` 远程派发仍禁用
- `cyanrex-runner-agent` 将可信 Linux、WSL2 和非特权容器节点连接到该协议
- `scripts/runner-agent.sh start` 启用可选加固 Compose Agent；对应冒烟脚本验证已认证编辑器路径
- 教师可在 **部署与设置 → Runner Agent 运维** 中查看 Agent 健康/容量，并探测或取消近期任务

## 脚本 API（已实现）

- `GET /scripts`（用户范围列表）
- `POST /scripts/save`（用户范围创建）
- `POST /scripts/delete`（用户范围删除）

## 学习 API（已实现）

- `GET /learning/labs` — 当前用户的五个实验目录与进度
- `GET /learning/attempts` — 当前用户由后端记录的运行尝试
- `GET /learning/teacher/overview` — 教师/管理员课堂进度汇总
- `GET /learning/teacher/attempts?username=...&limit=...` — 仅教职人员可用的单个学生有界尝试、反馈和已提交源码审阅
- `POST /learning/teacher/feedback` — 教师/管理员在现有尝试上保存学生可见评论；需要 CSRF Origin 和 `{ username, attempt_id, comment, expected_revision }`
- 反馈为纯文本（去除首尾空白后 1–2000 个字符），评审者与时间由服务端指定。首次评论使用修订 `0`，之后使用最后读取的修订；过期编辑返回 `409`。反馈随尝试持久化，绝不改变自动完成结果或已提交源码。
- 学生可在 **学习 → 我的实验历史** 查看自身提交与教师反馈。
- 仅提供已知 `lab_id` 时，`/ebpf/run` 才记录尝试；完成结果依据真实运行、必需模板/源码模式以及附着验证计算。

## API 契约

- `GET /openapi.json` 无需认证即可提供版本化 OpenAPI 3.1 文档。
- [`engine/openapi/openapi.json`](../../engine/openapi/openapi.json) 从已注册 Axum 路由和维护的请求/响应 schema 生成。
- 质量门禁拒绝 Engine 与 OpenAPI 文档之间的路由或访问级别漂移，以及意外的 JavaScript SDK 覆盖或生成模型漂移。
- 打包后，SDK 消费者可从 `@cyanrex/sdk-js/openapi` 导入生成 schema，并从 `@cyanrex/sdk-js/operations` 导入操作元数据/类型。
- 冻结的 `0.3.0` 兼容基线拒绝删除操作、访问变更、收窄请求和削弱成功响应保证，但允许增量演进。

变更路由或线模型后，请重新生成并验证：

```bash
node scripts/generate-openapi.mjs
node scripts/openapi-contract.mjs
node scripts/api-compatibility.mjs
node scripts/generate-sdk-types.mjs
```

## 认证持久化

这些表属于现有运行时，而非显式持久协作 Source。其命名空间安装、账户世代与当前 Session 事务见[持久认证](collaboration-auth-source.md)；不要让准备层安装器指向现有部署，也不要假设这些存储会自动同步。

- 表：`users`、`sessions`
- 迁移模板：`engine/migrations/0001_auth_users_sessions.sql`

## 数据持久化

- 事件中心使用 `event_records`（`engine/migrations/0002_event_records.sql`）
- 脚本存储使用 `user_scripts`（`engine/migrations/0003_user_scripts.sql`）
- 源码快照、自动反馈和实验进度使用 `learning_attempts`（`engine/migrations/0004_learning_attempts.sql`）

## 工程规则

- 本仓库采用 TDD 优先。
- 流程：`Red -> Green -> Refactor`。
- 后端路由变更先更新 `engine/tests/` 中的测试。
- API 变更需重新生成 OpenAPI 文档，并保持其组件 schema 最新。
- 只有在明确评审的兼容性重置中，才可替换兼容基线。
- 维护的源文件限制为 600 行；文档文件限制为 2000 行。
- 团队约定见 `TDD.md`。

提交变更前，运行与 CI 相同的检查：

```bash
./scripts/quality-gate.sh
```

`--format-only` 运行公共预检查和格式化，而不运行应用测试。默认、可丢弃数据库、浏览器、SDK 和特权验收边界见[测试指南](testing-guide.md)。[历史网络](functional-network.md)及其 [JSON 快照](../functional-network.json)是保留的证据，而非当前完整清单。

### CI 门禁（`ci-gate`）

- 工作流 `CI` 当前并行运行检查，再使用最终 `ci-gate` 任务汇总结果：
  - `security-audit`
  - `file-lengths`
  - `engine`
  - `frontend`
  - `sdk`
  - `permissions`
  - `distribution`
- 将分支保护设置为仅要求 `ci-gate`（而非每个矩阵式任务），这样任意任务失败都会导致必需门禁失败。
- 推荐的 GitHub 分支规则（GitHub UI）：
  - Settings → Branches → Branch protection rules
  - 为默认分支（例如 `main`）添加规则
  - 启用 **Require status checks to pass before merging**
  - 添加必需状态检查：`CI gate`
  - （可选）各子任务不单独勾选以减少噪音；`ci-gate` 已汇总失败。

### Tag 候选验收

推送新的带注释 `v<major>.<minor>.<patch>` Tag 会运行 `Release Candidate Validation`。验证不可变版本树后，它从精确提交构建离线归档，要求 `release-metadata.json` 报告相同 Tag/修订、干净源码和本地构建镜像，针对解出的归档运行 `install-smoke.sh`（包括已加载镜像内容 ID 检查），并在将通过验收的归档与外层校验和上传为保留 30 天的工作流产物前，启用特权实时内核附着/事件/分离验收。产物还保留独立带校验和的实时内核报告，绑定候选元数据、内核环境、唯一事件与精确清理；在 CI 和 Tag 验收中，已验证解包取代直接展开归档。统一候选验证器在上传前以流方式检查完整四文件产物，并可在下载后安全解包。这不会创建 GitHub Release 或签名包。
