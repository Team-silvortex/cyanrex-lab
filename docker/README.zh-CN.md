# Docker

使用 Docker Compose 运行完整 Cyanrex 栈。

## 服务

- `postgres` 默认 `127.0.0.1:15432`（`CYANREX_POSTGRES_PORT`）
- `engine` 默认 `127.0.0.1:8080`（`CYANREX_ENGINE_PORT`）
- `frontend` 默认 `127.0.0.1:3000`（`CYANREX_FRONTEND_PORT`）
- 可用不同实例 ID 和端口同时运行多个实例：
  - `--instance-id room-a` 和 `--instance-id room-b`
  - `--engine-port`、`--frontend-port`、`--postgres-port`
- 避免冲突的分配示例：
  - room-a：`--instance-id room-a --engine-port 18080 --frontend-port 13000 --postgres-port 25433`
  - room-b：`--instance-id room-b --engine-port 18081 --frontend-port 13001 --postgres-port 25434`

需要时在 `docker/.env` 调整事件持久化：

- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_ENABLED`（默认 `true`）
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_RATIO_PCT`（默认 `80`）
- `CYANREX_EVENT_PERSIST_QUEUE_CLEAR_RATIO_PCT`（默认 `40`）
- `CYANREX_EVENT_PERSIST_QUEUE_WARNING_INTERVAL_MS`（默认 `10000`）

同一文件中可配置本地 Runner 容量：

- `CYANREX_RUNNER_MODE`（当前支持 `local_process`）
- `CYANREX_RUNNER_MAX_CONCURRENT`（默认 `2`，范围 `1`–`32`）
- `CYANREX_RUNNER_MAX_PER_USER`（默认 `1`，不高于全局上限）
- `CYANREX_RUNNER_TIMEOUT_SECS`（默认 `45`，范围 `5`–`300`）
- `CYANREX_RUNNER_AGENT_TOKEN`（可选；空／缺失时禁用 Agent 注册，至少 32 字符）
- `CYANREX_RUNNER_AGENT_TTL_SECS`（默认 `30`，节点超过该间隔标记离线）
- `CYANREX_RUNNER_AGENT_RETENTION_SECS`（默认 `300`，过期内存记录保留时间）
- `CYANREX_RUNNER_AGENT_SIGNATURE_WINDOW_SECS`（默认 `60`，范围 `15`–`300`）

Engine 镜像在 `/app/modules` 内置版本化模块目录，并设置 `CYANREX_MODULES_DIR`。
目录启停只改变 Engine 状态，不执行模块目录中的文件。

这些是公平性和资源限制。Docker Engine 仍报告 `shared_kernel`，不是多租户隔离边界。
不支持的模式会使 Engine 启动失败，不静默回退到本地执行。

Agent 注册时签发独立凭据，后续请求要求带时间戳和一次性 nonce 的 HMAC-SHA256 签名。
队列承载内置探测及明确提交的仅编译检查，不分派 `/ebpf/run`、不加载 eBPF、不返回目标文件。
注册和签发凭据须保密，使用 TLS 或私有管理网络；节点丢失或被攻陷后重新注册以轮换凭据。

### 托管编译 Agent

可选 Compose Agent 默认禁用。主栈创建 `docker/.env` 后，可启用并验证完整认证编辑器路径：

```bash
./scripts/runner-agent.sh start
./scripts/runner-agent-smoke.sh
```

管理器创建缺失的 bootstrap token，不打印它，并写入匹配的 0600 Docker Secret；仅在控制面
此前禁用时重建 Engine。Agent profile 不发布端口，以拥有 0600 token 的 UID／GID 运行，
文件系统只读。它移除全部 Linux capability，启用 `no-new-privileges`，设置 PID／CPU／内存
限额，使用仅能访问 Engine、不能访问默认数据库／前端网络或外部网络的内部网络。管理命令：

```bash
./scripts/runner-agent.sh status
./scripts/runner-agent.sh logs
./scripts/runner-agent.sh stop
```

分发包根目录提供相同的两个脚本。

### 独立 Runner Agent

Engine 镜像包含 `cyanrex-runner-agent`。复制 `runner-agent.env.example`，只读挂载 bootstrap
token，并覆盖镜像入口：

```bash
cp docker/runner-agent.env.example runner-agent.env
chmod 600 runner-agent.env agent-token
docker run --rm --name cyanrex-runner-agent \
  --user "$(id -u):$(id -g)" \
  --read-only --security-opt no-new-privileges \
  --pids-limit 64 --memory 1536m --cpus 1 \
  --tmpfs /tmp:rw,noexec,nosuid,nodev,size=128m \
  --entrypoint cyanrex-runner-agent \
  --env-file runner-agent.env \
  --mount type=bind,src="$PWD/agent-token",dst=/run/secrets/cyanrex-agent-bootstrap-token,ro \
  cyanrex/cyanrex-engine:0.3.5
```

编译检查默认关闭，且 `shared_kernel` 下禁止使用。即使启用，Agent 也不得使用 `--privileged`、
宿主机 PID、内核挂载或额外 capability。Linux 和 WSL2 操作见学习中心的 Runner Agent 指南。

## eBPF 注意事项

- `engine` 是有意采用特权并可见宿主机 PID 的教学沙箱。
- Compose 中 `memlock` 为 unlimited，以满足 libbpf。
- 挂载宿主机 `/sys/fs/bpf` 和 `/lib/modules` 以进行内核／BPF 集成。
- Linux 上加载程序操作 Docker 宿主机内核；Windows／macOS 上操作 Docker Desktop VM 内核。
- 不向不可信网络暴露 Engine 端口，也不允许不可信用户提交程序。

## 启动

```bash
./start.sh start
# optional pre-flight check before classroom launch:
./scripts/check-instance-conflicts.sh --instance-id room-c --engine-port 18100 --frontend-port 13100 --postgres-port 25435
# explicit Docker backend:
./start.sh start --mode docker
# force rebuild when Dockerfile/dependencies changed:
./start.sh start --rebuild
./start.sh start --instance-id room-a --engine-port 18080 --frontend-port 13000 --postgres-port 15433
./start.sh status --instance-id room-a
./start.sh stop --instance-id room-a
```

首次运行时启动器创建 `docker/.env`，数据库、管理员和 TOTP 密钥随机生成。全部发布端口默认
绑定回环。远程使用优先选择 SSH 隧道或 TLS 反向代理；没有防火墙和可信用户访问控制时，
不要设置 `CYANREX_BIND_ADDRESS=0.0.0.0`。

使用 `scripts/package-distribution.sh` 生成的部署包时，运行包内辅助脚本：

```bash
cd /path/to/cyanrex-lab-<version>-<build-id>
cp .env.example .env   # fill secure credentials first
./run.sh               # same as ./deploy.sh up
./deploy.sh status     # show running services
./deploy.sh logs       # follow logs
./deploy.sh down       # stop services
```

包内部署辅助脚本默认验证密钥、Docker daemon 就绪和 Engine 启动健康。

包内 `runner-agent-smoke.sh`、`live-kernel-smoke.sh` 优先使用相邻原生 Rust release CLI。
旧 shell／Python 路径保留为旧包的兼容回退。

在一次性 Docker 宿主机验收发布时，解压后立即运行 `./install-smoke.sh`。它验证全部包校验和，
加载捆绑镜像，启动完整栈，检查前端和 Engine，运行经过认证的 Runner Agent 编译探测，最后
删除自行生成的配置和 Docker 卷。已有 `.env` 时拒绝运行；仅为诊断保留失败栈时设置
`CYANREX_SMOKE_KEEP=1`。包、镜像身份和 Engine 健康验证使用捆绑 Rust CLI，默认安装验收
不需要宿主机 Python。

已有安装仍用旧开发凭据时，在 `docker/.env` 设置 `CYANREX_ROTATE_ADMIN_CREDENTIALS=true`，
启动 Engine 一次后立即改回 `false`；这会使已有管理员会话失效。

主注册表不可用时，启动器用回退镜像、Debian 和 Cargo 镜像源重试。也可明确覆盖
`ENGINE_RUST_IMAGE`、`ENGINE_DEBIAN_IMAGE`、`ENGINE_APT_MIRROR`、
`ENGINE_CARGO_REGISTRY_INDEX`；前端 npm 注册表可用 `FRONTEND_NPM_REGISTRY` 覆盖。

## WSL2 后端

Windows 教学推荐把仓库克隆到 WSL Linux 文件系统（例如 `~/cyanrex-lab`）后运行：

```bash
./start.sh start --mode wsl
```

此模式使 Engine 操作 WSL2 内核，PostgreSQL 由 Docker 启动。启动器以当前用户构建 Engine，
仅为 Engine 进程请求 `sudo`，因为加载 eBPF 需要较高 Linux 权限。

实验前在环境助手确认 BTF、bpffs、bpftool、跟踪和 memlock。内核能力缺失时先更新 WSL；
高级实验可能需要自定义 WSL 内核。

## 检查

```bash
docker compose -f docker/docker-compose.yml ps   # or: docker-compose -f docker/docker-compose.yml ps
curl http://localhost:8080/health
```

## 停止

```bash
docker compose -f docker/docker-compose.yml down   # or: docker-compose -f docker/docker-compose.yml down
```
