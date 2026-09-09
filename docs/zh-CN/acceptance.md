# 验收清单

快照：**2026-09-09**，包含 Unreleased 变更的 **0.3.5** 版本线工作区。
这些增量现收录于 **0.3.6**；测量、输入清单和二进制哈希保留升版前的快照，不重写为 0.3.6
发行制品验收证据。
这是本地集成验收检查点，不是发行制品验收，也不表示运行中的部署已包含这些源码。
更完整的测试基线见[项目进度状态](project-status.md)。

## 第一轮：已通过

| 检查 | 实际结果 | 范围 |
|---|---|---|
| 真实 PostgreSQL | PostgreSQL 16.14 上 1 项通过 | 服务层 SQL 调用，不是浏览器/HTTP 验收 |
| 真实 Runner Agent | 签名回环 HTTP 与 Clang 18.1.3 编译，1 项通过 | 探针与只编译作业，不加载 eBPF |
| CI 配置 | 本地配置回归和 YAML 结构检查通过 | 新 PostgreSQL 步骤尚未在 GitHub Actions 运行 |

数据库测试从旧学习记录表开始，迁移评语字段，以同一版本号竞争两次写入，重载成功更新，并保留
原始源码和自动完成状态。还检查所有者准确读取历史提交、排除他人/不存在的记录、重载新提交，
以及连接池关闭后读取和评语写入报错且不创建本地快照。这不验证网络分区、断电、数据库容量或
完整学生/教师会话。

Agent 测试在 `127.0.0.1:0` 自建 HTTP 服务，使用合成引导凭据注册客户端并完成探针/编译作业，
检查作业成功状态及非空目标文件大小/哈希，随后清理工作目录。它实际上在非特权宿主进程中运行；
所声明的 `Container` 能力只是元数据，**不能证明容器或 VM 隔离成立**。

数据库按 content ID 使用本机已有 PostgreSQL 16 镜像，采用随机密码、临时回环端口、UID/GID 999、
只读根文件系统、移除 capabilities、`no-new-privileges`、资源限制及 tmpfs 数据。没有挂载宿主数据
或持久化数据卷，测试后仅删除本轮新建的容器。未改变部署凭据、已有数据库、Compose 栈、VM 或
内核挂载；未启动特权 Engine，也未执行离线安装冒烟。

机器可读结果、部分源码哈希和测试输出摘录保存在
`reports/acceptance/2026-09-09-real-integrations/`。这些哈希只覆盖选定文件，不是完整源码清单；
所记录的 HEAD 也不能单独标识这个有未提交变更的工作区。它们不能替代发行要求的候选产物校验和
及绑定候选产物的真实内核证据。

## 第二轮：一次性 VM 内核验收已通过

获准后，使用 2026-09-05 发布的 Ubuntu 24.04 minimal 镜像创建独立 QEMU/KVM Guest。
先用已安装的 Ubuntu cloud-image keyring 验证 SHA-256 清单签名，再核对镜像校验和。
更新 Guest 软件包后，实际验收环境为 Ubuntu 24.04.5、内核 `6.8.0-139-generic`、Clang 18.1.3
和 bpftool 7.4.0。

- 资源：2 vCPU、3072 MiB Guest 内存、8 GiB 按需分配磁盘。非特权 QEMU 进程还受 200% CPU、
  4 GiB 内存、禁止 swap、64 个任务和两小时运行时限约束；启用 seccomp，Guest 没有宿主文件
  系统共享、设备直通或 Docker socket。
- SSH 使用新建私钥，主机密钥由 Guest 串口核对并固定，禁用密码认证与 Agent 转发。宿主只在
  回环端口 32222 转发 Guest SSH。安装阶段临时允许下载软件包；加载 eBPF 前先关机，再以 QEMU
  `restrict=on`、禁用 IPv6 的模式启动。专用宿主回环探针在安装模式可达、隔离模式不可达，
  同时宿主自身仍可访问探针。这是专项网络检查，不是完整局域网或逃逸测试。
- 从当前工作区锁定依赖构建 **dev profile** 原生 Engine 与 Rust 验收程序，传入 Guest 后哈希一致。
  Engine 仅在 Guest 内运行并绑定自身回环，管理员/TOTP 凭据随机生成、只保留在进程中，
  没有使用部署 `.env`、数据库或前端。
- 真实 Aya `sched/sched_switch` 挂载、唯一匹配的 24 字节 ring-buffer 事件、准确 detach 与空挂载
  清单检查通过。独立 bpffs 检查没有残留 pin；Engine 尚未退出时，前后内核对象清单仍是相同的
  12 个程序 ID 和 1 个 link ID。Rust 证据验证器在 Guest 与宿主两侧均通过。
- 验收 Engine、探针和 VM 均已停止，VM 磁盘通过 `qemu-img check`。私有 VM 文件留在仓库外供
  后续使用，没有重启已有 VM 或 Compose 服务。

证据及全部 145 项 Engine 源码输入哈希在 `reports/acceptance/2026-09-09-kernel-vm/`。
证据明确记录 `candidate: null`：这是**单一 Guest 内核上的源码级验收**，不是优化后的发行构建、
离线安装包、Tag 候选或完整教学/局域网验收。报告中的 `native-linux` 指 Guest 内核，不是物理
宿主内核；手工管理的测试 VM 也不代表新增了可选 Runner 模式或学生 VM 生命周期功能。

## 复现不加载内核的集成测试

先创建一个使用 UTF-8 locale 的**全新、可丢弃数据库**，测试角色需能创建/删除 schema。
通过本地环境设置 `CYANREX_TEST_DATABASE_URL`，绝不能指向已有课堂数据库。不要复制部署 `.env`，
也不要把凭据写进报告。测试成功时只删除自己生成的 schema；失败可能留下 schema，因此无论
成功或失败都应销毁测试数据库。CI 为此独立创建 PostgreSQL 服务。

在仓库根目录运行；要求 Rust 和支持 BPF target 的 Linux `/usr/bin/clang`：

```bash
set -euo pipefail
test -n "${CYANREX_TEST_DATABASE_URL:-}"
task_acceptance_data=$(mktemp -d /tmp/cyanrex-integration-data.XXXXXX)
task_pg_case=services::learning_store::feedback::tests::postgres_feedback_migrates_legacy_rows_and_serializes_updates
cargo test --manifest-path engine/Cargo.toml --locked --lib "$task_pg_case" \
  -- --ignored --list | grep -Fx "$task_pg_case: test"
env -u DATABASE_URL -u CYANREX_BENCH_DATABASE_URL \
  CYANREX_DB_FALLBACK=true CYANREX_DATA_DIR="$task_acceptance_data" \
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$task_pg_case" \
  -- --ignored --exact --nocapture
env -u DATABASE_URL -u CYANREX_TEST_DATABASE_URL -u CYANREX_BENCH_DATABASE_URL \
  CYANREX_DATA_DIR="$task_acceptance_data" \
  cargo test --manifest-path engine/Cargo.toml --locked --test runner_agent_client_tdd \
  -- --ignored --nocapture
```

两项测试在可移植的默认门禁中仍被忽略，由 CI 分别显式调用。数据库 URL 和启用标志仅作用于
该步骤；服务使用临时回环端口，不占用部署的固定端口。准确名称/list 检查避免测试改名后零测试
也被当成通过。完成后只检查、清理本轮记录的临时数据目录。

## 后续门槛，按顺序推进

1. **一次性 Linux 目标——已准备：** 上述独立、受限 Guest 已通过源码级内核验收并关机。
   候选安装应使用全新 Guest 状态，不得替换为生产宿主或无关的已有 VM；临时 Guest 不是备份。
2. **候选安装与内核证据：** 构建干净、已版本化的候选，核对源码和镜像/归档校验和，再在目标机
   运行解包后的安装冒烟。内核检查必须观察真实 Aya 挂载、唯一绑定的 ring-buffer 事件、准确
   detach 与无残留。安装/内核脚本有特权和破坏性操作，执行前先阅读前置条件。
3. **局域网教学验收：** 从预期客户端验证选定的 TLS/来源策略和可达性、登录/CSRF/角色边界、
   真实学生提交 → 教师评语 → 确认后恢复、事件重连及重启持久化。只用合成账号与数据，不需要公网入口。
4. **隔离边界：** 若要让学生安全运行不可信程序，必须先实现并验证 VM 生命周期、持久化所有权、
   执行/事件转发、故障恢复与跨所有者拒绝。本地运行仍共享宿主内核，只编译 Agent 不提供该边界。
   详见[课堂隔离目标](classroom-isolation.md)。
5. **发布决策：** 汇总证据、完成版本/Changelog 和干净源码检查后，另行确认 commit、push/tag
   与部署操作。保留未通过项，不能将缺口标成通过。

本轮只读检查发现运行中的前端仍包含 Next.js 15.5.22，而工作区已锁定 15.5.24 和 sharp 0.35.4。
本轮没有重新部署；获准更新部署时按[安全指南](security.md)执行重建。
