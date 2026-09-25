# Runner Agent 使用指南

独立的 `cyanrex-runner-agent` 用于把可信 Linux、WSL2 或容器节点接入 Engine 控制面。0.4.3 版本
执行内置 `control_probe`，并可选择开启只编译的 `ebpf_compile_check`。编译检查默认关闭；两种模式
都不接受 Shell 命令或任意可执行载荷，也不需要 root 和 Linux Capability。编译作业不会加载 eBPF，
也不会返回目标文件。

## 准备 Engine

外部 Agent 需要先生成 32～512 字符的随机 Bootstrap Token，写入 Engine 的 `docker/.env` 后重启
Docker 栈：

```bash
openssl rand -hex 32
# 将输出保存为 CYANREX_RUNNER_AGENT_TOKEN。
./start.sh stop && ./start.sh start --mode docker
```

不要把 Token 放进命令行、代码仓库、截图或前端配置。Agent 只在注册时使用它；注册返回的单节点
凭据仅保存在进程内存。Agent 重启后会重新注册并轮换该凭据。

Engine 和 Agent 必须同步时钟，因为签名默认只有 60 秒时效。跨主机连接应使用 TLS；非回环地址的
明文 HTTP 默认拒绝，只有受信、带防火墙的实验网络才应打开显式例外。

## 托管 Docker Agent

源码树与离线发布包都包含可选的 `runner-agent` Compose Profile，常规启动不会自动启用它。主系统
完成配置后执行：

```bash
./scripts/runner-agent.sh start
./scripts/runner-agent-smoke.sh
```

发布包中直接运行根目录的 `./runner-agent.sh` 和 `./runner-agent-smoke.sh`。管理脚本只在控制面尚未
配置时生成 Token，将其写入私有运行环境和权限为 0600 的 Docker Secret，并重建一次 Engine 使配置
生效；Token 不会输出到终端。可用 `status`、`logs`、`stop` 管理生命周期。

托管 Agent 使用只读、无特权容器，丢弃全部 Linux Capability，不发布端口，并限制 PID、CPU 和内存；
可写 `/tmp` 是较小的 `noexec` tmpfs。仅内部可见的控制网络让它只能连接 Engine，不能访问默认的
PostgreSQL/前端网络或外部网络。它如实报告 `container` 隔离并开启只编译诊断，`/ebpf/run` 仍在本地
执行。
CI 还会让同一个 Agent 客户端连接真实回环 Engine 和 Linux Clang，并解析源码版与发布版两个
Compose Profile。

## Linux 或 WSL2

使用普通用户构建：

```bash
cargo build --release --locked \
  --manifest-path engine/Cargo.toml \
  --bin cyanrex-runner-agent
```

创建仅 Agent 账户可读的 Token 文件：

```bash
install -m 600 /dev/null ~/.cyanrex-agent-token
# 文件中只粘贴 Bootstrap Token。
```

启动：

```bash
CYANREX_AGENT_ENGINE_URL=https://engine.lab.example \
CYANREX_AGENT_BOOTSTRAP_TOKEN_FILE="$HOME/.cyanrex-agent-token" \
CYANREX_AGENT_ID=lab-vm-01 \
CYANREX_AGENT_ISOLATION=virtual_machine \
./engine/target/release/cyanrex-runner-agent
```

若要在隔离节点开启只编译检查，请安装支持 BPF Target 的 Clang，并增加：

```bash
CYANREX_AGENT_ENABLE_COMPILE_CHECK=true \
CYANREX_AGENT_CLANG_PATH=/usr/bin/clang \
CYANREX_AGENT_ISOLATION=virtual_machine \
./engine/target/release/cyanrex-runner-agent
```

`shared_kernel`、`container`、`virtual_machine`、`dedicated_host` 必须如实描述节点边界。该字段只用于
教师观察，不会凭空创建隔离。

## 外部容器

Engine 镜像也包含 `/usr/local/bin/cyanrex-runner-agent` 和 Clang。运行 Agent 时不要添加
`--privileged`、宿主 PID、内核目录挂载或额外 Capability：

```bash
docker run --rm --name cyanrex-runner-agent \
  --user "$(id -u):$(id -g)" \
  --read-only --security-opt no-new-privileges \
  --pids-limit 64 --memory 1536m --cpus 1 \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=128m \
  --entrypoint cyanrex-runner-agent \
  --env-file ./runner-agent.env \
  --mount type=bind,src="$PWD/agent-token",dst=/run/secrets/cyanrex-agent-token,ro \
  cyanrex/cyanrex-engine:0.4.3
```

配置可以从 [`docker/runner-agent.env.example`](../../docker/runner-agent.env.example) 开始。源码更新后
需要重建 Engine 镜像。虽然同一镜像也能运行特权 Engine 服务，但 Agent 本身保持无特权。容器是
实际边界时应设置 `CYANREX_AGENT_ISOLATION=container`；报告 `shared_kernel` 的 Agent 会被拒绝开启编译。

## 配置

| 变量 | 默认值 | 说明 |
|---|---:|---|
| `CYANREX_AGENT_ENGINE_URL` | `http://127.0.0.1:8080` | 不带路径的 Engine 基础地址 |
| `CYANREX_AGENT_BOOTSTRAP_TOKEN` | 无 | 直接提供密钥；优先使用文件变量 |
| `CYANREX_AGENT_BOOTSTRAP_TOKEN_FILE` | 无 | 只包含 Bootstrap Token 的文件 |
| `CYANREX_AGENT_ID` | `$HOSTNAME` | 稳定的 3～64 字符节点 ID |
| `CYANREX_AGENT_ISOLATION` | `shared_kernel` | 真实隔离描述 |
| `CYANREX_AGENT_MAX_CONCURRENT` | `1` | 上报容量，范围 1～32 |
| `CYANREX_AGENT_CAPABILITIES` | `control_probe` | 逗号分隔能力；必须支持探针 |
| `CYANREX_AGENT_ENABLE_COMPILE_CHECK` | `false` | 显式开启有上限的只编译作业，并加入 `clang_check` |
| `CYANREX_AGENT_CLANG_PATH` | `/usr/bin/clang` | 不经过 Shell 调用的 Clang 绝对路径 |
| `CYANREX_AGENT_COMPILE_WORK_DIR` | 系统临时目录下的 `cyanrex-runner-agent` | 私有、用后删除的编译根目录 |
| `CYANREX_AGENT_POLL_SECS` | `5` | 心跳和领取间隔，范围 1～30 秒 |
| `CYANREX_AGENT_REQUEST_TIMEOUT_SECS` | `10` | HTTP 超时，范围 2～60 秒 |
| `CYANREX_AGENT_ALLOW_INSECURE_HTTP` | `false` | 在明确受信实验网允许非回环 HTTP |
| `CYANREX_AGENT_ONCE` | `false` | 完成一次成功轮询后退出 |

不能同时设置两种 Token 变量。客户端会禁用重定向和环境 HTTP 代理，避免注册凭据被意外转发到
其他地址。

## 运行流程

1. 使用 Bootstrap Token 注册并取得单节点 HMAC 凭据；
2. 发送带签名的健康与容量心跳；
3. 领取数量不超过已上报空闲容量；
4. 同步 Job Lease，检查取消或丢失；租约已丢失的任务不得开始执行；
5. 执行内置探针，或在显式开启时用固定参数调用 Clang 做编译检查；
6. 编译检查限制资源与输出，只返回目标摘要并删除工作区，不加载或返回目标文件；
7. Engine 丢失内存注册状态后自动重新注册；
8. 收到 Ctrl-C 时尽力发送 `draining` 心跳。

领取容量使用心跳中的 `active_jobs + available_slots`，注册校验保证它不超过登记上限；队列把
已领取和等待取消确认的租约计入该总量一次。空位数已经扣除了上报的进行中任务，不能重复扣减；
也不能直接使用登记上限来重新开放 Agent 主动保留的容量。旧心跳不能绕过队列的未完成租约计数。
独立客户端仍逐个处理任务，本次修复没有增加客户端并行执行。

同步结果的 `lost_job_ids` 包含当前任务时，Agent 在执行或上报前丢弃它；即使 `cancel_job_ids`
也包含它，丢失状态仍优先。该任务按可重试冲突处理，轮询可以继续；有效取消仍不执行任务而直接确认。
这是执行前检查，不是执行中的持续租约监控，也不保证立刻中止 Clang。

读取 Engine 响应时逐块检查累计大小，包含没有 `Content-Length` 的分块响应和错误响应；保留的
正文将超过 640 KiB 时立即拒绝，不等发送方结束，恰好 640 KiB 仍可接受。原有请求超时继续生效。
这限制响应正文缓冲，不是整个传输层或进程的内存上限。

## 教师部署运维

教师可进入 **部署与设置 → Runner Agent 运维**。面板每 10 秒自动刷新，也支持手动刷新，并会明确区分
“控制面未启用”和“控制面已启用但暂无节点”两种状态。面板展示：

- 在线与保留 Agent 数、健康空闲容量、隔离类型、版本、能力、标签、内核版本和最后心跳；
- 最近 12 个远程作业的状态、目标或执行 Agent、所有者、创建时间与有上限的结果消息；
- 对健康 Agent 显式发送健康探针，以及取消排队中或已领取的作业。

面板不会渲染作业输出或源码。节点清单和管理接口要求教师权威，兼容旧管理员账号；学生仍只能
在编辑器中取得脱敏后的编译后端清单。教师无需另登管理员账号。

教师通过 `POST /runner/jobs/compile-check` 显式提交只编译作业，并用 `GET /runner/agents` 和
`GET /runner/jobs` 查看状态。已登录的编辑器用户通过 `GET /ebpf/check/backends` 获取脱敏后的编译
后端清单；显式选择 Agent 后，编辑器用 `POST /ebpf/check/remote` 提交、用带 `job_id` 的同名 GET
接口轮询，并通过 `POST /ebpf/check/remote/cancel` 取消过期请求。作业绑定当前用户，每个用户最多
同时保留两个未终结的远程检查。

编辑器远程检查从发出提交开始，整次请求最多等待 35 秒，包含读取响应及轮询；本地内联检查为
20 秒。修改源码、头文件上下文、Engine 或目标会取消旧请求并隐藏旧标记。取消/过期任务显示
`unavailable`，正常完成的编译拒绝仍显示 `issues`。请求携带会话、禁止缓存和重定向，不自动重试
或转本地。已知但未完成的作业会尽力取消，取消请求单独限时 10 秒，也包括切换后迟到的提交响应；
未知 ID 由下面的服务端排队/租约规则兜底。浏览器暂停或导航不证明服务端已停止，取消也不代表回滚。
8 秒/24 项缓存只属于当前编辑器挂载，不跨导航保留或跨编辑器共享请求，也不是跨标签页的会话撤销机制。

用户绑定的检查排队 35 秒仍未领取时，在下一次队列交互（提交、领取、查状态、取消、同步、结果或
清单查询）过期，释放源码及用户活跃名额；终态元数据沿用 15 分钟保留规则。没有后台定时器，
完全闲置的队列要等被访问才清理。这避免提交响应丢失或尽力取消失败永久占满用户的两个名额。
已领取任务的执行期限仍从领取时计算；教师管理接口提交的无用户归属探针/编译任务保持原有显式
排队/取消规则，不应用新等待时限。响应结构和所有者检查不变。

编辑器默认使用本地检查；所选 Agent 不可用时会明确失败，不会静默回退。`/ebpf/run` 仍在本地执行，
远程加载仍未启用。作业清单只记录源码大小，不记录源码正文。协议只允许字面量安全系统头文件；
引号、宏生成、父目录相对路径、`include_next`、`embed` 和头文件探测写法都会被拒绝。

## 故障排查

- `401`：Bootstrap Token 不一致、节点凭据被轮换、Nonce 重放或时钟偏差；
- Engine 重启后出现 `404`：属于正常情况，Agent 会自动重新注册；
- `503`：Agent 控制面未启用，或有上限的注册表/队列已满；
- 非回环 HTTP 被拒绝：应配置 HTTPS；只有受信且有防火墙的实验网才打开明文例外；
- 签名持续失败：先同步系统时间，再考虑轮换凭据。
- 用户检查在领取前过期：确认 Agent 健康、空位及 `clang_check` 后明确重试，不会自动转本地；
  教师管理任务仍可排队等待领取或显式取消；
- 编译配置被拒绝：使用 `container`、`virtual_machine` 或 `dedicated_host`，提供存在的 Clang 绝对
  路径，并把工作目录放在私有、可丢弃的存储中。

复现与验证范围见[沿链路抓虫 02](functional-network-bug-hunt-02.md)。
