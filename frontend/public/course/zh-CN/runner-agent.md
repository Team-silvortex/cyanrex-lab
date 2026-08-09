# Runner Agent 使用指南

独立的 `cyanrex-runner-agent` 用于把可信 Linux、WSL2 或容器节点接入 Engine 控制面。0.2.0 版本
执行内置 `control_probe`，并可选择开启只编译的 `ebpf_compile_check`。编译检查默认关闭；两种模式
都不接受 Shell 命令或任意可执行载荷，也不需要 root 和 Linux Capability。编译作业不会加载 eBPF，
也不会返回目标文件。

## 准备 Engine

生成 32～512 字符的随机 Bootstrap Token，写入 Engine 的 `docker/.env` 后重启：

```bash
openssl rand -hex 32
# 将输出保存为 CYANREX_RUNNER_AGENT_TOKEN。
./start.sh restart
```

不要把 Token 放进命令行、代码仓库、截图或前端配置。Agent 只在注册时使用它；注册返回的单节点
凭据仅保存在进程内存。Agent 重启后会重新注册并轮换该凭据。

Engine 和 Agent 必须同步时钟，因为签名默认只有 60 秒时效。跨主机连接应使用 TLS；非回环地址的
明文 HTTP 默认拒绝，只有受信、带防火墙的实验网络才应打开显式例外。

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
管理员观察，不会凭空创建隔离。

## 容器

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
  cyanrex/cyanrex-engine:0.2.0
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
4. 同步 Job Lease，检查取消请求；
5. 执行内置探针，或在显式开启时用固定参数调用 Clang 做编译检查；
6. 编译检查限制资源与输出，只返回目标摘要并删除工作区，不加载或返回目标文件；
7. Engine 丢失内存注册状态后自动重新注册；
8. 收到 Ctrl-C 时尽力发送 `draining` 心跳。

管理员通过 `POST /runner/jobs/compile-check` 显式提交只编译作业，并用 `GET /runner/agents` 和
`GET /runner/jobs` 查看状态。已登录的编辑器用户通过 `GET /ebpf/check/backends` 获取脱敏后的编译
后端清单；显式选择 Agent 后，编辑器用 `POST /ebpf/check/remote` 提交、用带 `job_id` 的同名 GET
接口轮询，并通过 `POST /ebpf/check/remote/cancel` 取消过期请求。作业绑定当前用户，每个用户最多
同时保留两个未终结的远程检查。

编辑器默认使用本地检查；所选 Agent 不可用时会明确失败，不会静默回退。`/ebpf/run` 仍在本地执行，
远程加载仍未启用。作业清单只记录源码大小，不记录源码正文。协议只允许字面量安全系统头文件；
引号、宏生成、父目录相对路径、`include_next`、`embed` 和头文件探测写法都会被拒绝。

## 故障排查

- `401`：Bootstrap Token 不一致、节点凭据被轮换、Nonce 重放或时钟偏差；
- Engine 重启后出现 `404`：属于正常情况，Agent 会自动重新注册；
- `503`：Agent 控制面未启用，或有上限的注册表/队列已满；
- 非回环 HTTP 被拒绝：应配置 HTTPS；只有受信且有防火墙的实验网才打开明文例外；
- 签名持续失败：先同步系统时间，再考虑轮换凭据。
- 编译作业一直排队：在隔离 Agent 上开启编译检查，并确认清单包含 `clang_check`；
- 编译配置被拒绝：使用 `container`、`virtual_machine` 或 `dedicated_host`，提供存在的 Clang 绝对
  路径，并把工作目录放在私有、可丢弃的存储中。
